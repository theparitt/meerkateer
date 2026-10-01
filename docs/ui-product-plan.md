# Meerkateer UI product plan: cute, simple, complete

Status: canonical UI direction. Last reviewed 2026-09-29.

## North-star goal

Meerkateer should feel like a friendly uptime guardian, not a wall of observability charts. A new
operator must understand what is healthy, what needs attention, why it happened, and what to do next
without learning monitoring jargon. The product remains complete enough for serious game-server and
SME-server operations by revealing advanced evidence and controls only when they are needed.

The design promise is:

> **Cute enough to feel approachable. Simple enough to act quickly. Complete enough to trust in an
> incident.**

Cute does not mean childish, and simple does not mean hiding risk. Outages, security boundaries,
data loss, destructive actions, and unsupported states always use direct language and unambiguous
controls.

This plan is subordinate to the release evidence in the [roadmap to 1.0](roadmap-to-1.0.md). A
screen is not complete because it looks polished; its real API, permissions, failure states,
accessibility, and end-to-end journey must pass.

## Who the UI serves

| Person | Primary question | Default complexity |
| --- | --- | --- |
| Owner of a small game/SME fleet | “Is everything okay?” | Summary and guided actions |
| On-call operator | “What broke, when, and what should I check?” | Incident evidence and next checks |
| Developer | “Is my process sending useful signals?” | SDK setup, credentials, payload diagnostics |
| Infrastructure administrator | “Are agents, storage, backups, and upgrades safe?” | Fleet and system administration |
| Auditor/security reviewer | “Who changed what and which tenant could access it?” | Roles, sessions, credentials, audit and exports |
| Meerkateer Cloud operator | “Is every tenant isolated and within SLO?” | Private provider operations, never shown in Community |

Community defaults to one company per installation. That company can have many workspaces, members,
machines, and services. Cloud adds company switching and hosted account operations without changing
the workspace mental model.

## Product principles

1. **Answer four questions in order.** Every operational surface starts with: Is it working? What
   changed? Why do we think that? What can I safely do next?
2. **Use progressive disclosure.** Summary first, evidence second, configuration third, dangerous
   controls last. Advanced detail is available but never competes with the next useful action.
3. **Show evidence, not confidence theatre.** “Unknown” and “no recent evidence” are different from
   “healthy.” The UI never guesses recovery from silence.
4. **One object, one home.** A machine, service, incident, alert delivery, credential, or member has
   one canonical detail page. Cards elsewhere link to it instead of duplicating partial editors.
5. **Make the safe path the short path.** Defaults are bounded; secrets are copy-once; destructive
   actions name the target and consequence; risky advanced settings are not in first-run forms.
6. **No decorative status.** Color is paired with an icon, label, time, and reason. Red is reserved
   for confirmed failure or destructive action; yellow for degraded/stale; gray for unknown; green
   only for fresh positive evidence.
7. **Mascot with purpose.** The meerkat guides setup, empty states, recovery, and learning. It does
   not celebrate an outage or obscure a critical warning.
8. **Real before visible.** Do not expose a button for a planned backend action. Planned capability
   belongs in documentation, not in an enabled production control.
9. **Fast on ordinary hardware.** Useful text and status render before charts. Lists paginate or
   virtualize. The Console remains usable on a small self-hosted installation and a slow network.
10. **Accessible by default.** Keyboard, screen reader, reduced motion, zoom, contrast, touch target,
    focus, and error association are component requirements, not a final polish phase.

## Information architecture

The authenticated Console uses one stable shell:

```text
Company (Cloud only when more than one)
└── Workspace
    ├── Overview
    ├── Machines
    ├── Services
    ├── Incidents
    ├── Alerts
    ├── Integrations
    └── Settings

Company administration
├── People & roles
├── Credentials & sessions
├── Audit log
└── Data & retention

Installation administration (Community owner)
├── System health
├── Backup & restore
├── Upgrade status
└── Diagnostics & support bundle

Hosted account (Cloud only)
├── Companies & regions
├── Usage & quotas
├── Service status & support
└── Billing later, after the free beta
```

On desktop this is a compact side navigation plus workspace header. On mobile it becomes a bottom
navigation for Overview, Machines, Services, and Incidents, with the remaining destinations in a
labelled menu. Switching workspace never silently changes company. The current company and
workspace remain visible in every operational view.

## Complexity layers

| Layer | What the operator sees | Examples |
| --- | --- | --- |
| Glance | Current state and one next action | 12 healthy, 2 need attention, “Check stale agent” |
| Inspect | Evidence and change history | reason, observed time, deployment, failed check, recovery |
| Configure | Routine safe settings | interval, alert destination, maintenance window, member role |
| Advanced | Rare or risky controls | retention, credential rotation, probe policy, restore, deletion |

Opening a detail view preserves the current filters and scroll position. Deep links work after sign
in. Browser Back returns to the same operational context.

## Complete surface map

### Public and access

| Surface | Required behavior | Important states |
| --- | --- | --- |
| Landing | Explain product, Community/Cloud modes, current version, limits, roadmap | mobile, no-JS basic content, current release claim |
| Getting started | Choose self-hosting, agent, SDK, or future Cloud path | unsupported OS, missing prerequisite, old docs |
| Setup | Create the one Community company and owner | invalid/expired setup key, existing company, weak password, API unavailable |
| Sign in/recovery | Password session and setup-key recovery | rate limit, revoked/expired session, CSRF, offline API |
| Cloud preview | Clearly planned, free beta first, no fake signup | unavailable, roadmap link, Community alternative |

### Overview

The overview answers “What needs me now?” It contains:

- fresh healthy/degraded/down/unknown totals with the observation age;
- an attention queue ordered by severity, duration, and freshness;
- active incidents and recent recoveries;
- machines that stopped reporting;
- alert-delivery failures and maintenance currently suppressing alerts;
- recent deployments or configuration changes near failures; and
- a short installation-health warning if ingestion, worker, database, disk, or notification paths
  are unhealthy.

It does not begin with decorative charts or total event counts. Healthy installations receive a
calm “All clear” state and the time at which that statement was supported.

### Workspaces

- create, rename, and archive a workspace;
- switch without losing company context;
- show members, machines, services, open incidents, and alert policy summary;
- empty state offers exactly two paths: enroll a machine or connect an application;
- archive explains retained data and active monitoring consequences; and
- slugs, display names, duplicate names, Unicode, long names, and authorization errors are handled.

### Machines

- searchable/filterable fleet list with freshness, OS, agent version, workspace membership, and
  last successful upload;
- machine detail with host collectors, processes/services, recent telemetry gaps, credential state,
  desired configuration, agent logs limited to diagnostics, and update compatibility;
- guided enrollment for Linux/Windows, copy-once token, command, proxy/CA notes, and `doctor` result;
- assign/remove from workspaces without confusing removal with agent revocation;
- rotate/revoke credentials, quarantine clones, and show why an agent is rejected; and
- filters for online, stale, offline, incompatible, permission failure, spool pressure, and update
  required.

The UI must distinguish “machine is offline,” “agent cannot reach control plane,” “collector lacks
permission,” and “the service is down.”

### Services and checks

- service list grouped or filtered by environment, kind, machine, state, and tags;
- service detail summary, latest evidence, checks, dependencies, deployments, incidents, credentials,
  and integration health;
- create flows for application SDK, process/system service, HTTP/HTTPS, TCP, DNS, TLS expiry,
  Docker/container, Minecraft Java, and later supported adapters;
- check configuration previews the target, interval, timeout, data collected, probe location, and
  security restriction before saving;
- credential issue/rotate/revoke with one-time secret handling and SDK snippets; and
- safe validation for redirects, private addresses, DNS rebinding, malformed responses, large
  payloads, unsupported protocol versions, and permission denial.

### Incidents

An incident is a first-class object, not only a colored timeline row:

- title, current state, severity, start/duration, affected service and workspace;
- “what happened,” “evidence,” “what changed nearby,” and “suggested next checks” sections;
- finding history that distinguishes confirmed failure, degraded evidence, missing evidence,
  acknowledgement, maintenance, and fresh recovery;
- deduplicated updates, alert history, notes, acknowledgement owner, and immutable audit events;
- deep link and export suitable for a support report; and
- recovery closes only on fresh supported evidence. Acknowledgement never changes service health.

Filters include open/recovered, severity, service, machine, assignee, alert state, and date. Flapping
is represented as one explainable incident according to the published incident policy, not a flood
of cards.

### Alerts

- destinations and routing policies are separate from incident state;
- channel setup and test for webhook first, then supported email/chat integrations;
- delivery history shows queued, delivered, retriable, dead-lettered, suppressed, and replayed;
- deduplication, cooldown, maintenance windows, acknowledgement, escalation basics, and external
  watchdog visibility;
- secret fields are write-only and redacted after save; and
- receiver `429`, `5xx`, timeout, invalid TLS, DNS failure, disabled channel, and recovery are
  actionable states.

The UI never says “alerted” when only an outbox item was created.

### Integrations

- language tabs for Node.js, Go, Rust, Python, and PHP plus the host agent;
- snippets generated from the selected workspace/service but containing no real secret in docs;
- copy-once credentials, rotation overlap, revocation, last-used time, and environment scope;
- heartbeat/event/deployment examples and a validation view for the most recent accepted/rejected
  payload; and
- compatibility version, retry behavior, offline behavior, and links to complete guides.

### People, security, and audit

- owner/admin/operator/viewer roles with a plain-language capability preview before assignment;
- invitation, expiry, resend, accept, removal, role change, session list/revocation, and optional
  OIDC when implemented;
- prevent removal or downgrade of the last owner;
- credentials grouped by machine, service, and user with state and last use—but never the secret;
- audit filters by actor, action, object, result, IP/context when retained, and time; and
- every denied screen explains whether the object is absent or the role lacks permission without
  leaking another tenant's existence.

### Data and installation operations

- retention and quota summary with projected storage impact;
- backup state, last verified restore, destination, encryption state, measured RPO/RTO, and a guided
  restore drill—not an untested “backup enabled” badge;
- migration and upgrade readiness, compatibility warnings, progress, failure, rollback window, and
  release notes;
- installation health for API, database, workers, queues, disk, alert receiver, and external
  watchdog;
- diagnostics bundle preview and redaction before download; and
- explicit export, archive, deletion, and uninstall flows with target-name confirmation and retained
  data consequences.

### Cloud-only account experience

- company switcher, invitation and tenant lifecycle;
- region and probe-location visibility without exposing provider internals;
- usage and quota explanations, fair-limit errors, export and deletion status;
- published beta SLO, service status, support request context, and incident communication; and
- billing pages only after the free beta gate. Billing failure never changes historical evidence and
  Community never renders Cloud commerce controls.

## Shared state language

Every monitored object uses the same words and visual grammar:

| State | Meaning | Visual requirement |
| --- | --- | --- |
| Healthy | Fresh positive evidence within policy | green + check + observed time |
| Degraded | Fresh evidence reports a partial problem | yellow + warning + reason |
| Down | Fresh evidence confirms failure | red + failure icon + reason/duration |
| Stale | Expected evidence is late | yellow/gray + clock + last evidence |
| Unknown | Not enough valid evidence | gray + question + what is missing |
| Maintenance | Monitoring continues; alert behavior follows policy | blue + tool/calendar + end time |
| Recovering | Positive evidence exists but recovery policy is not complete | blue/green + progress explanation |

“Loading,” “empty,” “partial data,” “offline browser,” “permission denied,” “rate limited,” “API
unavailable,” and “unexpected error” are application states, not monitoring states. Components must
not confuse them.

## Visual system: cute without noise

- Keep the existing navy, sky blue, mint, coral, warm cream, and gold family. Semantic status tokens
  are centralized and meet WCAG contrast.
- Use rounded cards, clear outlines, small shadows, friendly illustrations, and short motion. Avoid
  glass effects that lower readability and avoid putting every section in a card.
- Mascot poses are limited to setup, monitoring, recovery, empty state, help, and Cloud identity.
  Reuse approved assets rather than generating a different character on each screen.
- Operational density is adjustable: comfortable by default, compact for large fleets. Density never
  changes semantics or hides status labels.
- Charts require a question they answer. Every chart has a text summary, units, time zone, range,
  missing-data treatment, and accessible table or list equivalent.
- Motion is under 200 ms for controls and optional for illustrations. `prefers-reduced-motion`
  removes nonessential animation.
- Critical dialogs are calm and plain. No mascot, confetti, euphemism, or ambiguous primary action.

## Responsive behavior

Test reference widths are 390, 768, 1024, and 1440 CSS pixels, plus 200% browser zoom.

- No horizontal page scroll at any supported width.
- Tables become labelled cards or horizontally contained regions; columns are not silently dropped.
- Touch targets are at least 44 × 44 CSS pixels where practical.
- Timeline and status retain time, reason, and state on mobile.
- Mobile actions remain near the affected object; destructive actions do not become swipe-only.
- The layout supports long English/Thai labels and user content without overlap. User-visible strings
  are centralized for future reviewed localization; critical warnings are not machine-translated at
  runtime.

## UI delivery phases

Implementation snapshot (2026-09-30): UI-0 is complete. The first UI-1/UI-2 slice is running in the
local Docker stack: the authenticated shell has deep-linked Overview, Machines, Services,
Incidents, Alerts, Maintenance, Admin, and Connect views; desktop and mobile navigation; workspace context; honest
loading/error/retry/permission/empty states; a guided company → workspace → machine/application →
fresh-signal journey; and the existing real enrollment, SDK credential, game probe, machine
assignment, and timeline actions. Incidents are durable open/resolved outage records with
role-aware acknowledgement, self-assignment, immutable notes, and operator activity; Alerts shows
audited policy and durable worker outcomes; Maintenance suppresses notifications without hiding
evidence; and Admin exposes tenant counts plus append-only audit activity to owner/admin roles.
Machine and service search/state/environment filters run against the currently loaded workspace data. Machine,
service, and service-evidence detail URLs survive refresh/sign-in and reject resources outside the
selected workspace without substituting another object. Pagination, canonical incident-detail
URLs, alert escalation/multi-destination routing, automatic first-signal refresh, and remaining backend-dependent surfaces remain
open work and are not represented as finished controls. A headless browser pass against the Docker
web container at 1440 × 1000 and 390 × 844 confirmed the Overview and first-signal flow have no
horizontal overflow; this is useful implementation evidence, not the later full visual-regression
matrix.

### UI-0 — Foundation and inventory (complete)

Deliver:

- this canonical plan and source-backed capability inventory;
- shared tokens for color, status, type, spacing, radius, shadow, focus, and motion;
- stable app-shell/navigation proposal and URL map; and
- component/state catalogue identifying current, partial, and planned behavior.

Tests and exit gate:

- existing landing, setup, dashboard, timeline, probe, and documentation tests remain green;
- no UI claims an unavailable action; and
- each planned screen maps to a roadmap capability and API dependency.

### UI-1 — First success (`0.2 Community Alpha`, active)

Deliver:

- guided company → workspace → machine or SDK → first signal journey;
- a simple “choose agent or application” fork;
- copy-once secret handling, progress steps, validation, and safe retry; and
- useful empty/error/recovery states.

Test cases:

- fresh install, setup-key error, duplicate bootstrap, weak password, zero workspaces, token expiry,
  clipboard denial, agent offline, SDK rejection, refresh mid-flow, Back/forward navigation, and API
  outage. A new operator must reach the first fresh signal without database or repository knowledge.

### UI-2 — Daily fleet operations (`0.2–0.3`)

Deliver Overview, Machines, Services, search/filter, canonical details, collector/check evidence, and
responsive navigation. Preserve current machine assignment, credential, SDK, game-probe, and
timeline capability while moving editors into their canonical homes.

Test cases include hundreds of objects, duplicate names, stale and unknown evidence, mixed agent
versions, unavailable collectors, pagination, deep links, slow API, partial responses, mobile, zoom,
keyboard, and screen reader. The glance view must identify every object needing attention without
opening each object.

### UI-3 — Incidents and alerts (`0.2–0.5`)

Deliver incident objects, evidence-led detail, alert destinations/policies/history, maintenance,
acknowledgement, retry/dead-letter/replay, and watchdog state.

Test cases include repeated/conflicting/out-of-order facts, false-recovery prevention, flapping,
receiver `429`/`5xx`/timeout, duplicate suppression, maintenance boundaries, acknowledgement versus
health, worker restart, and inaccessible destination. The UI state must agree with durable database
evidence after reload.

### UI-4 — Integrations and agent lifecycle (`0.3`)

Deliver OS-specific enrollment, `doctor`, proxy/CA guidance, agent config/update/rotation, supported
collectors/probes, five SDK consoles, payload validation, and compatibility explanations.

Test offline spool, reboot, sleep/resume, credential overlap/revocation, incompatible version,
permission denial, DNS rebinding rejection, oversized/malformed response, copy failure, and command
escaping on supported shells. No displayed command contains a committed or reusable example secret.

### UI-5 — Team and security (`0.4`)

Deliver members/roles, invitations, sessions, optional OIDC, credentials, audit, and permission-
specific navigation. Test every role, direct deep link, guessed cross-tenant ID, invite expiry/replay,
last-owner protection, session revocation, CSRF/rate limit, secret redaction, and audit injection.
Zero cross-tenant disclosure is the exit gate.

### UI-6 — Installation operations (`0.5–0.7`)

Deliver retention/quota, backup/verified restore, upgrade/rollback, system health, queues/workers,
diagnostics bundle, capacity envelope, compatibility, and release information.

Test corrupt/missing backup, wrong key, interrupted migration, insufficient disk, old/new schema,
database/worker/network failure, poison job, high load, partial diagnostics, and unsupported platform.
Operators must complete a restore and upgrade drill using only the UI and published runbook.

### UI-7 — Hosted Beta (`0.8`, parallel after `0.5`)

Deliver company switching, hosted onboarding, regions, quotas, export/delete, Cloud service status,
support context, and SLO visibility in the private Cloud repository. Reuse public operational
components and status semantics.

Test concurrent/partial provisioning, multi-company identity, tenant switching during open detail,
region failure/move, fair limits, support impersonation audit, large export, deletion/backup expiry,
and every cross-tenant path. Stripe stays absent.

### UI-8 — Release candidate and stable (`0.9–1.0`)

Deliver final consistency, accessible help, complete diagnostics, visual regression, performance
budgets, compatibility/support contracts, release notes, and known limitations. Freeze new UI scope
at `0.9` except release blockers.

Pass at 390/768/1024/1440, 200% zoom, keyboard-only, screen reader, reduced motion, high contrast,
slow/offline network, current supported browsers, and a 14-day representative operator soak. No
severity-one/two usability defect, inaccessible critical action, silent failure, or misleading state
may remain.

## Test pyramid and evidence

| Layer | Required evidence |
| --- | --- |
| Component | state matrix, keyboard behavior, accessible name/description, error association |
| Contract | generated API type, schema validation, permission/error mapping, no invented fields |
| Integration | real API/database for create/change/reload, tenant context, durable state |
| Journey E2E | first success, daily check, outage/recovery, alert failure, rotation, restore/upgrade |
| Visual | approved screenshots at four widths for critical screens and every semantic state |
| Accessibility | automated checks plus manual keyboard, screen reader, zoom, contrast, reduced motion |
| Resilience | slow/partial/offline API, retry, reload, concurrent change, stale cache, old agent |
| Security | direct URL and action matrix by role/tenant, secret redaction, CSRF, audit evidence |
| Performance | useful content, interaction, list scale, memory, bundle and request budgets |

Screenshots alone never close a phase. Each UI feature needs a real enabled backend path, loading,
empty, success, failure, permission, and recovery coverage appropriate to its risk.

## Definition of done for a screen

A screen is done only when:

- its primary operator question and next safe action are obvious;
- all data comes from a versioned real contract;
- URLs/deep links, refresh, Back, and concurrent changes behave predictably;
- loading, empty, partial, stale, permission, error, retry, and recovery states exist;
- role and tenant boundaries are tested at UI, API, worker, and database layers;
- keyboard, accessible names, focus, zoom, contrast, reduced motion, and mobile pass;
- timestamps, time zones, units, missing evidence, and destructive consequences are explicit;
- logs and analytics contain no secret or unnecessary personal/telemetry data;
- tests and screenshots are stored as release evidence; and
- documentation and known limitations match what the button actually does.

## Immediate implementation order

1. Extract the authenticated app shell and introduce real routes without changing backend behavior.
2. Turn the current all-in-one operations page into Overview, Machines, Services, and Setup flows.
3. Create shared status, freshness, evidence, empty/error, secret, confirmation, and responsive-list
   components.
4. Finish the `0.2` incident and alert backend contracts, then build their canonical screens.
5. Add visual/accessibility E2E infrastructure before expanding to team, operations, and Cloud UI.

This order keeps the UI cute and simple immediately while ensuring every later capability has a
stable place instead of growing the current dashboard into one large, confusing page.
