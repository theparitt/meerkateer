# Contributing to Meerkateer

Thank you for improving Meerkateer. The project values small, reviewable changes with
tests and clear operational behavior.

## Before starting

1. Review existing project documentation. The public issue tracker and pull request workflow
   will become available when the canonical repository is published.
2. Open a design discussion once the public forum is available before changing a public protocol, schema, security
   boundary, storage contract, billing behavior, or supported-platform promise.
3. Read `docs/standards/` and the accepted architecture decision records under
   `docs/adr/`.

## Development workflow

1. Create a focused branch in your source checkout. Fork the repository once it is public.
2. Add or update tests before changing behavior.
3. Run `make test` and `make lint` as documented in `README.md`.
4. Update documentation and `CHANGELOG.md` for user-visible changes.
5. Submit a pull request using the repository template once the public repository is available.

Pull requests must not include generated secrets, production data, player identity,
customer data, raw request bodies, or unredacted support bundles.

## Developer Certificate of Origin

The project uses the Developer Certificate of Origin 1.1 in [DCO](DCO). Every commit
must include a sign-off:

```text
Signed-off-by: Your Name <your.email@example.com>
```

Create it with `git commit -s`. The sign-off certifies that you have the right to submit
the contribution under the project's Apache-2.0 license; it is not merely a signature
formatting requirement.

## Review and merge

- At least one maintainer approval is required.
- Protocol, authentication, billing, data deletion, agent update, or cryptographic
  changes require two approvals when the project has two active maintainers.
- CI must pass and unresolved review threads must be addressed.
- Maintainers may request smaller changes, additional fixtures, a threat-model update,
  or an ADR.

## Reporting security issues

Do not open public issues for vulnerabilities. Follow [SECURITY.md](SECURITY.md).
