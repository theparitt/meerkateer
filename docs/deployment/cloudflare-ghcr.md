# Cloudflare web and GHCR production packaging

This runbook publishes the web console to Cloudflare Workers and packages the API and
background worker as OCI images in GitHub Container Registry (GHCR). The production host
remains under the operator's control.

Meerkateer is still a developer preview. This runbook makes deployments repeatable; it does
not waive the open production-readiness gates in `docs/phase-status.md`. Use a private pilot,
keep backups, and do not promise a GA service from this build.

## Deployment shape

```text
browser
  -> app.example.com (Cloudflare Worker + static assets)
       -> /v1, /health, /ready ...
          -> api-origin.example.com (Cloudflare Access, recommended)
             -> Cloudflare Tunnel
                -> 127.0.0.1:6510 on 192.168.0.148
                     -> server + worker + dedicated PostgreSQL 18
```

The Worker is a same-origin API gateway. Browser cookies and CSRF protection therefore work
without enabling broad cross-origin access. The API port binds only to loopback on the
production host; do not publish port 6510 on the LAN or router.

## 1. Publish the web console

Requirements:

- Node.js 22.22.2 or newer and npm 10;
- a Cloudflare account with Workers enabled; and
- an HTTPS API-origin hostname routed through Cloudflare Tunnel. A private LAN address such as
  `192.168.0.148` cannot be used as `MEERKATEER_API_ORIGIN`.

Authenticate once and select the Cloudflare account when prompted:

```sh
cd meerkateer-web
npm ci
npx wrangler login
npx wrangler whoami
```

Store the API origin as an encrypted Worker secret. Enter only the origin: HTTPS scheme,
hostname and optional port, with no path.

```sh
npm run cloudflare:set-api-origin
# Example value entered at the prompt: https://api-origin.example.com
```

Now every web release is one command:

```sh
npm run deploy
```

That command runs the TypeScript/Vite production build first and then calls `wrangler deploy`.
Before a public API exists, the Worker can publish the landing page without
`MEERKATEER_API_ORIGIN`; API paths fail closed with HTTP 503 while static routes remain available.
After the secret is configured, the Worker proxies only Meerkateer API paths and continues serving
the Vite SPA from Workers Static Assets.

Before a release, an offline packaging check is available:

```sh
MEERKATEER_API_ORIGIN=https://api-origin.example.com npm run deploy:dry-run
```

Configure the public custom domain, for example `app.example.com`, on the
`meerkateer-web` Worker in the Cloudflare dashboard. Do not point the API-origin secret back
to that same hostname; the Worker rejects this loop.

### Protect the tunnel origin with Cloudflare Access

The preferred production setup places a Cloudflare Access application on
`api-origin.example.com` and permits a Service Auth policy. Create one Access service token,
then add both values as encrypted Worker secrets:

```sh
npx wrangler secret put MEERKATEER_ACCESS_CLIENT_ID
npx wrangler secret put MEERKATEER_ACCESS_CLIENT_SECRET
npm run deploy
```

The gateway removes any client-supplied Access headers and injects the trusted pair. If only
one of the two secrets is configured, API requests fail closed with HTTP 503. Keep the service
token out of `.dev.vars`, source control, shell history, and build arguments.

For local Worker testing, copy `.dev.vars.example` to `.dev.vars`; Wrangler ignores that file
through the repository `.gitignore`.

## 2. Build and publish API/worker images to GHCR

The repository's hardened runtime image contains both binaries. It is built once and tagged
as `meerkateer-server` and `meerkateer-worker`; Compose selects the worker binary with an
entrypoint override.

Authenticate to GHCR without putting the token in a command argument:

```sh
export CR_PAT='<GitHub token with write:packages>'
printf '%s' "$CR_PAT" | docker login ghcr.io -u theparitt --password-stdin
unset CR_PAT
```

Use an immutable SemVer or commit tag. Publishing builds locally and then pushes both tags:

```sh
make publish-images VERSION=0.1.0
```

To build without pushing:

```sh
make images VERSION=0.1.0
```

The default target platform is `linux/amd64`. Override it only when the production host has a
different architecture:

```sh
MEERKATEER_IMAGE_PLATFORM=linux/arm64 make publish-images VERSION=0.1.0
```

Publishing `latest` is opt-in because production should pin an immutable tag:

```sh
MEERKATEER_PUBLISH_LATEST=true make publish-images VERSION=0.1.0
```

If the GHCR packages are private, log in on the production host with a separate read-only
token carrying `read:packages`. Never copy the publishing token to production.

## 3. Prepare `192.168.0.148`

Install Docker Engine and the Compose v2 plugin, then clone the exact source tag used for the
image. The source checkout is needed for the versioned SQL migrations mounted by Compose.
Generate production credentials on the production machine, not on a laptop:

```sh
git clone https://github.com/theparitt/meerkateer.git
cd meerkateer
git checkout 0.1.0
make production-env VERSION=0.1.0
make production-config
```

`make production-env` creates `deploy/.env.production` with mode `0600`, random independent
database roles, a bootstrap token, and a metrics token. It refuses to overwrite an existing
file. Back this file up in a password manager or encrypted secret store; losing it can make
recovery impossible. Do not copy it into Git.

The default production topology runs a dedicated PostgreSQL 18 container and named volume on
`.148`. This is safer than sharing the database instance on `.147` with unrelated systems. If
an external PostgreSQL server is later required, allocate a dedicated database and dedicated
owner/app/worker roles and test backup/restore before changing the two database URLs.

For a private GHCR package, authenticate with the read-only token, then start the stack:

```sh
make production-pull
make production-up
curl --fail http://127.0.0.1:6510/live
curl --fail http://127.0.0.1:6510/ready
docker compose --env-file deploy/.env.production \
  -f deploy/compose.production.yaml ps
```

The API should show healthy, the worker should remain running, and PostgreSQL should be
healthy. Inspect logs without printing the environment file:

```sh
make production-logs
```

Create a Cloudflare Tunnel route from `api-origin.example.com` to
`http://127.0.0.1:6510` on this same machine. Keep the Compose binding on loopback. Verify
through the public web hostname:

```sh
curl --fail https://app.example.com/live
curl --fail https://app.example.com/ready
```

Finally open `https://app.example.com/setup` and use the bootstrap token from the private
production environment file to create the first company owner. Do not send that token over
chat or email.

Stop containers without deleting the database volume:

```sh
make production-down
```

Never add `-v` to that command unless the explicit intention is to destroy all PostgreSQL
data.

## 4. Backup, upgrades, and rollback

Take a logical backup before every image change. A timestamped example is:

```sh
mkdir -p backups
umask 077
docker compose --env-file deploy/.env.production \
  -f deploy/compose.production.yaml exec -T postgres \
  pg_dump -U meerkateer_owner -d meerkateer -Fc > "backups/meerkateer-$(date -u +%Y%m%dT%H%M%SZ).dump"
```

Copy backups off `.148` and regularly prove a restore into an isolated database. A file that
has never passed a restore exercise is not a verified backup.

For an upgrade:

1. read the release notes and migration instructions;
2. take and verify a backup;
3. update the source checkout and `MEERKATEER_IMAGE_TAG` to the same release;
4. run the release's migration procedure;
5. run `make production-config`, `make production-pull`, and `make production-up`; and
6. verify `/live`, `/ready`, sign-in, one SDK heartbeat, worker delivery, and the incident UI.

Do not assume that mounting a new SQL file upgrades an existing PostgreSQL volume: files in
`docker-entrypoint-initdb.d` run only when a volume is first initialized. Until a release has
an explicit tested migration path, stop and test the upgrade on a restored copy rather than
changing production.

Application rollback means restoring the prior image tag and matching source checkout.
Database rollback means restoring the pre-upgrade backup; SQL migrations are not assumed to
be reversible.

## Release checklist

- `npm test`, `npm run check`, `npm run build`, and `npm run deploy:dry-run` pass in
  `meerkateer-web`.
- `make test`, `make lint`, and the relevant integration/failure labs pass.
- both GHCR tags exist for the exact immutable version.
- the production Compose configuration validates without printing secret values.
- a current off-host backup has passed a restore exercise.
- Cloudflare Access protects the API-origin hostname and the tunnel targets loopback.
- `/live`, `/ready`, owner login, machine enrollment, SDK heartbeat, failure detection,
  recovery, webhook delivery, and the visual timeline are checked after deployment.
