# Release verification

Meerkateer release evidence has two separate layers:

1. SHA-256 checksums detect a changed download.
2. GitHub artifact attestations bind an artifact or OCI digest to this repository, workflow, and
   source revision using GitHub's Sigstore-backed identity.

This does not remove Windows SmartScreen warnings. Windows Authenticode/MSIX trust is a separate
platform-signing decision and the current Controller preview remains explicitly unsigned.

## OCI images

A version tag or explicitly dispatched version builds one immutable multi-architecture image and
publishes it as both `meerkateer-server` and `meerkateer-worker`. BuildKit attaches an SBOM and
maximum provenance; GitHub publishes attestations for both image names.

```sh
gh attestation verify \
  oci://ghcr.io/theparitt/meerkateer-server:VERSION \
  --repo theparitt/meerkateer

docker pull ghcr.io/theparitt/meerkateer-server:VERSION
docker image inspect ghcr.io/theparitt/meerkateer-server:VERSION --format '{{index .RepoDigests 0}}'
```

Production Compose must pin a released version and operators should record the resulting digest.
Do not deploy `latest` as a production rollback target.

## Controller packages

Download the package and its adjacent `.sha256`, verify the checksum, then verify provenance:

```sh
sha256sum --check meerkateer-controller_0.2.0_amd64.deb.sha256
gh attestation verify meerkateer-controller_0.2.0_amd64.deb --repo theparitt/meerkateer
```

The attestation proves where the bytes were built; it does not claim that the current preview has
passed the Public Preview operator gate. That gate still requires supported-platform installation,
accessibility, upgrade/rollback, restore, and fresh-operator evidence.
