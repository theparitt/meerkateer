# Governance

Meerkateer uses a maintainer-led, consensus-seeking governance model.

## Roles

- **Contributors** submit issues, documentation, tests, designs, or code.
- **Reviewers** are trusted contributors who regularly review changes.
- **Maintainers** merge changes, manage releases and security reports, and protect the
  project's technical and community standards.

The initial repository owner acts as the initial maintainer. Maintainer additions and
removals must be recorded in this file with a public rationale once identities and the
canonical repository are established.

## Decision making

Routine changes use pull-request review. Maintainers seek consensus for material design
decisions. Public protocol, storage, security-boundary, licensing, governance, or
backward-compatibility changes require an ADR and a documented review window.

If consensus cannot be reached, maintainers record the alternatives and rationale, then
make the smallest reversible decision. Security embargoes may be handled privately and
documented after coordinated disclosure.

## Releases

Releases follow Semantic Versioning. A release requires passing CI, an updated
changelog, reproducible artifacts, dependency/license review, and the gates defined in
the production roadmap. No release may be called Stable solely because a date arrived.

## Commercial service

Meerkateer Cloud operations, pricing, customer contracts, and service-level agreements
are commercial decisions. They do not reduce the rights granted to the Community code
under Apache-2.0 or grant control over independent forks.
