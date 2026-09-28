# Game server beta: Minecraft Java / Paper first

Decision: 2026-09-28. The first pilot is for owners and admins of roughly 1–10 game instances. No FiveM pilot is available yet, so Minecraft Java / Paper is the first adapter. Meerkateer remains a reliability layer beside existing hosting panels; Cloud hosts monitoring, not the game server. Community receives the same core monitoring features without a billing dependency.

## Promise and boundaries

> Know when the game is unreachable from a probe, when the game loop slows, and when it recovers. Alert admins once and give players a clear status page.

An instance is a service with its own address, credentials, signals, alert rules, and history. A machine can host multiple instances. The control-plane health card describes Meerkateer itself; it must never stand in for a player's view of game health. No public RCON, root password, remote shell, hosting panel replacement, or automated world restore is required for monitoring.

The three signals remain separate:

| Signal | Evidence | Safe display when absent or failing |
| --- | --- | --- |
| External reachability | Minecraft Java status response and probe location/time | “Unreachable from this probe” or “Could not test”; never “process crashed” from this alone |
| Host/process | Scoped agent or collector heartbeat | “Internal data missing”; never infer game reachability |
| Game performance | Paper collector TPS/MSPT and optional process CPU/memory | “Not connected”; never fill a missing metric with zero |

Player count is the count the server reports; zero players is healthy by itself. Probe latency is measured from the named probe, not every player's ping. A successful status query does not prove that login or gameplay works. Paper targets 20 ticks/s and 50 ms/tick in its standard loop, but thresholds must be configurable and sustained before an alert. Link to spark for profiling; Meerkateer will not claim a plugin caused an incident from timing alone.

## Pilot delivery order and acceptance cases

| Slice | Deliverable | Acceptance cases |
| --- | --- | --- |
| G0 — Scope and safe query | Persist Minecraft instance address separately from machine; bounded, on-demand Minecraft Java status test from the Community control plane; distinguish protocol response, rejection, timeout, and unsafe destination. | G0.1 one machine can represent several services/instances; G0.2 public status fixture reports version/player counts/latency without player names; G0.3 malformed/oversize/truncated protocol fails safely; G0.4 all resolved private/loopback/link-local/metadata addresses are rejected and DNS is not resolved again during connect; G0.5 one tenant cannot probe another tenant's instance; G0.6 probe cannot be called in Cloud until network isolation is enforced. |
| G1 — Automatic observations | Scheduled probes outside the game's failure domain, retained reachability facts, staleness, and a separate external status card. | G1.1 stop process, block only external path, collector-only loss, and intermittent timeout show different states; G1.2 probe restart/retry does not duplicate observations; G1.3 no-data intervals remain unknown; G1.4 probe location and last checked time are shown. |
| G2 — Paper insight | Optional outbound Paper collector or plugin sends bounded TPS/MSPT/player count and process signals through a scoped credential; no player identity. | G2.1 low TPS/high MSPT while query works shows performance degradation; G2.2 plugin stops while query works shows internal data missing; G2.3 absent metrics are not zero; G2.4 spike below configured duration sends no alert; G2.5 spark link is optional and validated. |
| G3 — Calm alerts | Discord webhook setup/test, durable issue/recovery notifications, confirmation window, dedupe, cooldown, maintenance suppression. | G3.1 sustained failure produces one issue and one recovery; G3.2 repeated failures do not spam; G3.3 transient failure is held; G3.4 webhook failure retries safely; G3.5 test alert and redaction; G3.6 maintenance expires and normal alerts resume. |
| G4 — Player status and story | Public, tenant-scoped status page with owner-selected address/player-count visibility, announcements, incident history, and scheduled maintenance; one operator timeline combines probe, collector, deploy, and admin notes. | G4.1 public page exposes no private address/secret by default; G4.2 status reflects game signals rather than control-plane health; G4.3 incident shows observed start, changes, recovery, and source; G4.4 no-data and maintenance are distinct from uptime; G4.5 keyboard/mobile review. |
| G5 — Real pilot | Run with 3–5 operators and record false positives, time-to-detect, setup friction, retention, and alert usefulness before a second adapter. | G5.1 each failure scenario above is reproduced in a test lab; G5.2 operators can explain one incident from the timeline; G5.3 support and restore drill; G5.4 compatibility matrix and release decision. |

G0 is the implementation slice now. G1–G5 remain open. Integration with Pterodactyl or txAdmin, backup-health ingestion, and carefully gated restart are later work driven by pilot feedback; they are not part of G0.

## Implementation rules

- Public Cloud probes require independent egress controls in addition to application validation. Until then the G0 test endpoint is Community-only and public-address-only.
- Resolve A and AAAA records, reject the request if any answer is unsafe, then connect to a validated numeric address. Bound DNS/connect/read time, packet size, and request frequency.
- Keep game responses limited to version and counts. Do not store or show player names, sample lists, chat, or IPs.
- The status page and alert messages must say “observed from this probe” and “reported reason” where appropriate; temporal proximity is context, not proof of cause.

References: [Paper scheduling and tick behavior](https://docs.papermc.io/paper/dev/scheduler/), [Paper spark guidance](https://docs.papermc.io/paper/profiling/), [Discord webhook documentation](https://discord.com/developers/docs/resources/webhook), and [OWASP SSRF prevention](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html). These links guide design; support is established by the acceptance cases above.
