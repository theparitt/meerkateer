# Security Policy

## Supported versions

Meerkateer is pre-alpha and has no supported production release yet. Security support
will begin with the first release candidate. The support table and end-of-support dates
must be published before `v1.0.0`.

## Reporting a vulnerability

Do not create a public issue, discussion, or pull request for a suspected vulnerability.
The public source repository and its private vulnerability reporting channel are not
available yet. Do not publish vulnerability details in a public issue or discussion.
The repository owner must enable private reporting and publish a monitored fallback
security address before the first public preview.

Include:

- affected version or commit;
- deployment mode and operating system;
- reproduction steps or a minimal proof of concept;
- expected and observed impact; and
- any known mitigations.

Do not access other users' data, persist access, disrupt a service, or include real
credentials or personal data in a report.

## Response targets

After the private channel is activated, the project targets acknowledgement within
three business days, an initial severity assessment within seven business days, and
coordinated remediation/disclosure based on impact. These are response targets, not a
warranty or paid support SLA.

## Security scope

High-priority areas include tenant isolation, authentication/authorization, agent
enrollment and update, telemetry privacy, parser safety, SSRF, billing entitlements,
secret handling, and supply-chain integrity.

## Community setup key

Free Community deployments use `MEERKATEER_BOOTSTRAP_TOKEN` to create the first company and
to recover the owner's password. Normal owner sign-in uses email and password. Treat the setup
key as a recovery credential: keep `.env` mode `0600`, never place the key in a URL, screenshot,
source file, or support message, and terminate TLS at a trusted reverse proxy before access from
another machine. Cloud mode does not expose this mechanism and will use its configured identity
provider.
