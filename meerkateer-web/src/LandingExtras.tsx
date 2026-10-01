import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import go from "highlight.js/lib/languages/go";
import javascript from "highlight.js/lib/languages/javascript";
import php from "highlight.js/lib/languages/php";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import { type ReactNode, useMemo, useState } from "react";
import "./landing-extras.css";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("go", go);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("php", php);
hljs.registerLanguage("python", python);
hljs.registerLanguage("rust", rust);

const stories = [
  {
    number: "01",
    title: "Give your team a home",
    copy: "Start Community, create a company, then organize each job or environment in a workspace.",
    image: "/mascot-setup-v1.png",
    alt: "Meerkateer mascot setting up a server and workspace folder",
    link: "/get-started",
    action: "Follow the setup guide",
    theme: "peach",
  },
  {
    number: "02",
    title: "Let Meerkateer keep watch",
    copy: "Enroll a machine with the agent. Add an SDK to a process for heartbeats, events, and deployments.",
    image: "/mascot-monitor-v1.png",
    alt: "Meerkateer mascot checking a dashboard with three friendly servers",
    link: "/about#sdks",
    action: "See the SDKs",
    theme: "sky",
  },
  {
    number: "03",
    title: "Understand the comeback",
    copy: "See a stale or offline state, its timeline, and the fresh signal that marks recovery.",
    image: "/mascot-recover-v1.png",
    alt: "Meerkateer mascot helping a server turn its healthy green light back on",
    link: "/#features",
    action: "Explore the features",
    theme: "mint",
  },
];

export function SystemMapSection() {
  return (
    <section className="landing-section system-section" id="how-it-works">
      <div className="page-width">
        <div className="system-intro">
          <p className="eyebrow">The whole picture</p>
          <h2>Little signals. One clear story.</h2>
          <p>
            Meerkateer connects the machines you run and the apps you build to one friendly place
            for status, incidents, and recovery.
          </p>
        </div>
        <figure
          className="system-picture"
          aria-label="How Meerkateer connects your machines and apps to a reliability dashboard"
        >
          <div className="system-column system-inputs">
            <span className="system-kicker">YOUR WORLD</span>
            <div className="system-node system-machine">
              <span className="system-node-icon" aria-hidden="true">
                ▥
              </span>
              <div>
                <strong>Machines</strong>
                <small>Host agent sends telemetry</small>
              </div>
            </div>
            <div className="system-node system-app">
              <span className="system-node-icon" aria-hidden="true">{`{ }`}</span>
              <div>
                <strong>Apps &amp; game servers</strong>
                <small>SDK sends heartbeats and events</small>
              </div>
            </div>
          </div>
          <div className="system-flow" aria-hidden="true">
            <span>signals</span>
            <b>➜</b>
          </div>
          <div className="system-column system-core">
            <span className="system-kicker">MEERKATEER COMMUNITY</span>
            <img src="/logo.png" alt="" />
            <strong>Collect, remember, explain</strong>
            <div className="system-core-pills">
              <span>API</span>
              <span>PostgreSQL</span>
              <span>Worker</span>
            </div>
            <small>Self-hosted control plane</small>
          </div>
          <div className="system-flow" aria-hidden="true">
            <span>clarity</span>
            <b>➜</b>
          </div>
          <div className="system-column system-output">
            <span className="system-kicker">YOUR VIEW</span>
            <div className="system-console">
              <div className="system-console-head">
                <i />
                <i />
                <i />
                <span>Operator console</span>
              </div>
              <div className="system-console-row">
                <span className="system-status good" />
                <strong>api-01</strong>
                <small>healthy</small>
              </div>
              <div className="system-console-row">
                <span className="system-status watch" />
                <strong>worker-04</strong>
                <small>stale</small>
              </div>
              <div className="system-console-row">
                <span className="system-status good" />
                <strong>api-02</strong>
                <small>recovered</small>
              </div>
            </div>
            <small className="system-output-note">Status + incident timeline</small>
          </div>
        </figure>
        <div className="system-caption">
          <span>
            <b>01</b> Connect a machine or service
          </span>
          <span>
            <b>02</b> Send small operational facts
          </span>
          <span>
            <b>03</b> See what changed and when
          </span>
          <a href="/meerkateer-system-map-v1.png" download>
            Download diagram ↓
          </a>
        </div>
      </div>
    </section>
  );
}

export function ActionStoriesSection() {
  return (
    <section className="landing-section page-width story-section" id="start-here">
      <div className="section-heading">
        <p className="eyebrow">A friendly first voyage</p>
        <h2>From first setup to “all clear.”</h2>
        <p>Three small steps show what Meerkateer does in practice.</p>
      </div>
      <div className="story-grid">
        {stories.map((story) => (
          <article className={`story-card story-${story.theme}`} key={story.number}>
            <div className="story-art">
              <img src={story.image} alt={story.alt} loading="lazy" />
            </div>
            <div className="story-content">
              <span className="story-number">{story.number} / 03</span>
              <h3>{story.title}</h3>
              <p>{story.copy}</p>
              <a href={story.link}>
                {story.action} <span aria-hidden="true">↗</span>
              </a>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

const samples = {
  node: {
    title: "Node.js SDK",
    subtitle: "For JavaScript servers and workers",
    guide: "/docs/node-sdk",
    code: `# Install from this repository\nnpm install ./sdk/node\n\n# Set the Console's service environment values, then:\nimport { Meerkateer } from "@meerkateer/sdk";\n\nconst watch = Meerkateer.fromEnv();\nawait watch.heartbeat("ok", { message: "worker ready" });\nawait watch.event("queue_delay", { level: "warning", message: "jobs delayed" });`,
  },
  go: {
    title: "Go SDK",
    subtitle: "For Go APIs and background services",
    guide: "/docs/go-sdk",
    code: `// Add the module: go get github.com/theparitt/meerkateer/sdk/go@main\nimport (\n    "context"\n    meerkateer "github.com/theparitt/meerkateer/sdk/go"\n)\n\nwatch, err := meerkateer.FromEnv()\nif err != nil { return err }\nreturn watch.Heartbeat(context.Background(), "ok", "worker ready")`,
  },
  python: {
    title: "Python SDK",
    subtitle: "For Python APIs and worker processes",
    guide: "/docs/python-sdk",
    code: `# Install from this repository\npython3 -m pip install ./sdk/python\n\n# Set the Console's service environment values, then:\nfrom meerkateer_sdk import Meerkateer\n\nwatch = Meerkateer.from_env()\nwatch.heartbeat("ok", message="worker ready")\nwatch.event("queue_delay", level="warning", message="jobs delayed")`,
  },
  php: {
    title: "PHP SDK",
    subtitle: "For PHP servers and WordPress integrations",
    guide: "/docs/php-sdk",
    code: `<?php\nrequire_once __DIR__ . '/Meerkateer.php';\n\n// Set the Console's service environment values first.\n$watch = \\Meerkateer\\Client::fromEnv();\n$watch->heartbeat('ok', 'worker ready');\n$watch->event('queue_delay', 'warning', 'jobs delayed');`,
  },
  agent: {
    title: "Host agent",
    subtitle: "For the machine your app runs on",
    guide: "/get-started",
    code: `# Create a workspace enrollment token in the Console.\nread -rsp 'Enrollment token: ' MEERKATEER_ENROLLMENT_TOKEN\nexport MEERKATEER_ENROLLMENT_TOKEN\n\ncargo run --locked -p meerkateer-agent -- enroll \\\n  --server http://127.0.0.1:6510 --name game-host-01\ncargo run --locked -p meerkateer-agent -- doctor\ncargo run --locked -p meerkateer-agent -- run`,
  },
  rust: {
    title: "Rust SDK",
    subtitle: "For a typed async service integration",
    guide: "/docs/rust-sdk",
    code: `use meerkateer_sdk::{HeartbeatStatus, Meerkateer};\n\n// Set the service environment values shown by the Console.\nlet client = Meerkateer::from_env()?;\nclient.heartbeat(HeartbeatStatus::Ok, None).await?;`,
  },
} as const;

type Sample = keyof typeof samples;

const sampleLanguages: Record<Sample, string> = {
  node: "javascript",
  go: "go",
  python: "python",
  php: "php",
  agent: "bash",
  rust: "rust",
};

function highlightedCode(sample: Sample): ReactNode {
  const html = hljs.highlight(samples[sample].code, { language: sampleLanguages[sample] }).value;
  const documentFragment = new DOMParser().parseFromString(html, "text/html");
  function renderNode(node: ChildNode, key: number): ReactNode {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent;
    if (!(node instanceof Element) || node.tagName !== "SPAN") return node.textContent;
    const className = [...node.classList].filter((name) => /^hljs-[\w-]+$/.test(name)).join(" ");
    return (
      <span className={className} key={key}>
        {[...node.childNodes].map(renderNode)}
      </span>
    );
  }
  return [...documentFragment.body.childNodes].map(renderNode);
}

export function IntegrationSection() {
  const [active, setActive] = useState<Sample>("node");
  const [copied, setCopied] = useState(false);
  const sample = samples[active];
  const highlighted = useMemo(() => highlightedCode(active), [active]);

  async function copyCode() {
    try {
      await navigator.clipboard.writeText(sample.code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2200);
    } catch {
      setCopied(false);
    }
  }

  return (
    <section className="landing-section integration-section" id="sdks">
      <div className="page-width integration-layout">
        <div className="integration-copy">
          <p className="eyebrow">SDKs for your stack</p>
          <h2>Send your first signal in your language.</h2>
          <p>
            Create a workspace in the Console. Enroll a machine with the agent, or create a process
            and copy its one-time SDK key. The code then sends the reliability facts you choose to
            report.
          </p>
          <ol>
            <li>
              <b>1</b>
              <span>Create a workspace and process</span>
            </li>
            <li>
              <b>2</b>
              <span>Set the key in your app environment</span>
            </li>
            <li>
              <b>3</b>
              <span>Send a heartbeat, event, or deployment</span>
            </li>
          </ol>
          <a className="button button-secondary" href="/get-started">
            Open the setup guide ↗
          </a>
        </div>
        <div className="integration-code">
          <div className="integration-tabs" role="tablist" aria-label="SDK example">
            {(Object.keys(samples) as Sample[]).map((key) => (
              <button
                type="button"
                role="tab"
                aria-selected={active === key}
                className={active === key ? "selected" : ""}
                key={key}
                onClick={() => {
                  setActive(key);
                  setCopied(false);
                }}
              >
                {samples[key].title}
              </button>
            ))}
          </div>
          <div className="integration-code-head">
            <div>
              <strong>{sample.title}</strong>
              <small>{sample.subtitle}</small>
            </div>
            <button
              type="button"
              onClick={() => void copyCode()}
              aria-label={`Copy ${sample.title} example`}
            >
              {copied ? "Copied ✓" : "Copy code"}
            </button>
          </div>
          <pre role="tabpanel" aria-label={`${sample.title} code example`}>
            <code className={`hljs language-${sampleLanguages[active]}`}>{highlighted}</code>
          </pre>
          <div className="integration-code-foot">
            <span>Use the Console's own values; the snippet shows no real secret.</span>
            <a href={sample.guide}>Read guide →</a>
          </div>
        </div>
      </div>
    </section>
  );
}

const releasePhases = [
  {
    version: "0.1",
    name: "Foundation Preview",
    state: "Current code",
    track: "Shared public core",
    outcome: "A runnable, multi-workspace foundation with durable reliability evidence.",
    features:
      "Tenant-aware PostgreSQL core, owner access, agent enrollment, five SDKs, durable ingest, status and timeline UI, failure labs, and deployment packaging.",
    tests:
      "Contract, API/SQL integration, SDK-language, migration, worker retry/dead-letter, browser-state, and deterministic fleet tests.",
    edges:
      "Duplicate and out-of-order facts, stale recovery, revoked credentials, malformed payloads, database outage, and worker restart between claim and acknowledgement.",
    gate: "Every accepted fact is processed, safely retried, explicitly rejected, or dead-lettered; none silently disappears or invents health.",
  },
  {
    version: "0.2",
    name: "Community Alpha",
    state: "In progress",
    track: "Community critical path",
    outcome: "One clean installation detects, explains, alerts, and recovers from a real failure.",
    features:
      "First-class incident workflow, alert outcome history/retry/dead-letter replay, audited cooldown policy, maintenance suppression, and guided first use are implemented; baseline host/process and HTTP/TLS checks remain.",
    tests:
      "Fresh VM setup, two workspaces and machines, process/endpoint failure, receiver 429/500/timeout, service restarts, and three clean end-to-end repetitions.",
    edges:
      "Clock skew, agent double-enrollment, queued alert after deletion, workspace reassignment, full spool, IPv6, Unicode names, and browser refresh during setup.",
    gate: "Failure → evidence → one incident → alert → fresh recovery completes without manual SQL, Cloud, billing, or AI credentials.",
  },
  {
    version: "0.3",
    name: "Monitoring Alpha",
    state: "Planned",
    track: "Community critical path",
    outcome: "Production-shaped host, process, network, and first game/SME monitoring.",
    features:
      "Linux and Windows services, bounded collectors, offline spool, HTTP/TCP/DNS/TLS probes, Minecraft compatibility, and safe configuration rollback.",
    tests:
      "Thirty-minute control-plane outage, spool drain, OS/service restart and upgrade, hostile probe fixtures, Minecraft protocol matrix, and resource benchmarks.",
    edges:
      "PID reuse, permission denial, full disk, split-horizon DNS, DNS rebinding, private redirect, invalid certificates, sleep/resume, and malformed game responses.",
    gate: "No unbounded collector, no duplicate drained facts, published compatibility, and reference usage below the declared CPU, memory, disk, and network ceilings.",
  },
  {
    version: "0.4",
    name: "Security Beta",
    state: "Planned",
    track: "Community critical path",
    outcome: "The identity, role, credential, and tenant boundary is ready for broader exposure.",
    features:
      "Invitations, least-privilege roles, revocation, optional OIDC, distributed abuse controls, signed agent config, secure credential storage, and complete audit evidence.",
    tests:
      "Two-tenant read/write/ID matrix across API, SQL, workers, exports, caches and pools; invite/session/CSRF tests; rotation under load; scans, fuzzing, and independent review.",
    edges:
      "Mixed-tenant batches, pooled connection reuse, stale caches, deleted members with queued jobs, last-owner removal, confusable identities, and replica rate-limit bypass.",
    gate: "Zero known cross-tenant access and no unresolved critical or high security finding.",
  },
  {
    version: "0.5",
    name: "Operations Beta",
    state: "Planned",
    track: "Community critical path",
    outcome: "Data, migrations, alerts, and the control plane remain recoverable under failure.",
    features:
      "Versioned upgrades, encrypted off-host backup, restore, retention, quotas, alert replay/cooldown, metrics, trace correlation, watchdogs, and runbooks.",
    tests:
      "Restore to another host, upgrades from supported versions, interrupted migration, database/worker/network/disk/receiver fault injection, and RPO/RTO measurement.",
    edges:
      "Corrupt archive, wrong key, partial upload, schema newer than binary, poison job, pool exhaustion, webhook recovery during retry, and retention/export races.",
    gate: "A verified restore and data-preserving upgrade meet published RPO/RTO; every fault is visible and has a rehearsed operator response.",
  },
  {
    version: "0.6",
    name: "Public Preview",
    state: "Planned",
    track: "Community critical path",
    outcome:
      "An external operator can install and understand Meerkateer without repository knowledge.",
    features:
      "Signed OCI and agent artifacts, checksums, SBOM and provenance, complete operator guides, guided onboarding, diagnostics, responsive UI, and WCAG 2.2 AA review.",
    tests:
      "Fresh-operator install-to-recovery study, signature and SBOM validation, keyboard/screen-reader/mobile passes, proxy/custom-CA/IPv6/firewall installs, upgrade and uninstall.",
    edges:
      "Port collision, read-only directories, low disk, proxy auth failure, missing CA, wrong architecture, mismatched agent/server versions, and retained-data uninstall.",
    gate: "At least three new operators complete the supported journey from published artifacts with no maintainer intervention.",
  },
  {
    version: "0.7",
    name: "Scale Beta",
    state: "Planned",
    track: "Community critical path",
    outcome: "Meerkateer publishes an honest, measured operating envelope.",
    features:
      "Reference topologies, compatibility window, backpressure behavior, retention sizing, rolling upgrades, capacity dashboards, and sustained fault soak.",
    tests:
      "Sustained and 10× burst load, reconnect storms, queue catch-up, high cardinality, mixed agent versions, rolling upgrade/rollback, and a 14-day fault-injected soak.",
    edges:
      "Noisy workspace, simultaneous quotas, millions of short incidents, hot indexes, autovacuum lag, long exports, backup/retention overlap, and regional latency.",
    gate: "Publish hardware, topology, throughput, p50/p95/p99 latency, storage, retention, rejection, and recovery limits with no silent loss or unbounded growth.",
  },
  {
    version: "0.8",
    name: "Hosted Beta",
    state: "Parallel after 0.5",
    track: "Cloud track · free first",
    outcome: "Named users validate a managed multi-tenant service before billing is introduced.",
    features:
      "Private Cloud provisioner and operations repo, tenant lifecycle, company switching, regional probes, quotas, backups, support workflows, SLOs, and cost measurement.",
    tests:
      "Concurrent provisioning, cross-tenant attack matrix across every data path, tenant export/delete, region and provider failures, restore drills, fairness, abuse, and cost tests.",
    edges:
      "Partial provisioning, duplicate signup, multi-company identity, deletion during incident, offline agents during region move, support impersonation, and backup-expiry erasure.",
    gate: "No isolation failure, successful region/restore drills, published beta commitments, sustainable measured cost, and Stripe remains disabled.",
  },
  {
    version: "0.9",
    name: "Release Candidate",
    state: "Planned",
    track: "Community critical path",
    outcome: "Feature-frozen artifacts prove the complete supported journey under real operation.",
    features:
      "Immutable release candidate, current documentation and risk register, independent review, rehearsed rollback and recovery, and representative operator trial.",
    tests:
      "Promote one digest through the full matrix, disaster recovery by a new operator, every supported platform/profile, and 3–5 operators for at least 14 days.",
    edges:
      "Upgrade during incident, restore while agents buffer, key rotation during rollout, retention during export, receiver outage during restart, and rollback with newer agents.",
    gate: "No severity-one/two defect, critical/high finding, silent loss, tenant leak, unrecoverable migration, or unsupported documentation gap.",
  },
  {
    version: "1.0",
    name: "Stable Community",
    state: "Target",
    track: "Supported release",
    outcome: "A production-supported Community release with public operating contracts.",
    features:
      "Published security, compatibility, deprecation, support, recovery, retention and capacity contracts; consistent versioning and named operational owners.",
    tests:
      "All release gates from signed artifacts, supported platform install/restart/upgrade/rollback/uninstall, restore targets, detection latency, and final 14-day soak evidence.",
    edges:
      "Revoked signing key, corrupt artifact, bad cache or mirror, urgent dependency disclosure, schema regression, incompatible agent rollout, and post-release rollback communication.",
    gate: "The system can be installed, operated, diagnosed, upgraded, restored, and removed without private maintainer knowledge and with no open release blocker.",
  },
];

export function UpdatesRoadmapSection() {
  return (
    <section className="landing-section updates-section" id="roadmap">
      <div className="page-width">
        <div className="updates-heading">
          <div className="section-heading">
            <p className="eyebrow">The release path</p>
            <h2>Ten milestones, one dependable product.</h2>
            <p>
              Follow the plan from the current foundation through monitoring, security, operations,
              scale, and the final stable release. Open a phase to inspect all of its evidence.
            </p>
          </div>
          <a className="button button-secondary" href="/docs/phase-status">
            Verified delivery status ↗
          </a>
        </div>
        <section className="repository-boundary" aria-label="Community and Cloud repository model">
          <article className="repository-card repository-public">
            <p className="eyebrow">Public · Apache-2.0</p>
            <h3>One product core</h3>
            <p>
              Server, worker, agent, Console, SDKs, migrations, tenant security, tests, and
              Community deployment live in <code>theparitt/meerkateer</code>.
            </p>
          </article>
          <div className="repository-flow" aria-hidden="true">
            <span>signed version + digest</span>
            <b>→</b>
          </div>
          <article className="repository-card repository-private">
            <p className="eyebrow">Private hosted operations</p>
            <h3>No Cloud fork</h3>
            <p>
              <code>theparitt/meerkateer-cloud</code> pins public artifacts and adds provisioning,
              regions, quotas, SLOs, support, and later billing—not a copy of core.
            </p>
          </article>
        </section>
        <p className="repository-note">
          Community is one company per installation with many workspaces and machines. Cloud is many
          tenants on a managed control plane. Reliability and security stay open source.{" "}
          <a href="/docs/repository-and-cloud-boundary">Read the repository contract →</a>
        </p>
        <section className="phase-grid" id="tests" aria-label="Meerkateer release milestones">
          {releasePhases.map((phase, index) => (
            <article
              className={`phase-card phase-${phase.state.toLowerCase().replaceAll(" ", "-")}`}
              key={phase.version}
            >
              <span className="phase-timeline-marker" aria-hidden="true">
                {String(index + 1).padStart(2, "0")}
              </span>
              <div className="phase-card-head">
                <span className="phase-version">v{phase.version}</span>
                <span className="phase-state">{phase.state}</span>
              </div>
              <p className="phase-track">{phase.track}</p>
              <h3>{phase.name}</h3>
              <p className="phase-outcome">{phase.outcome}</p>
              <details open={phase.state === "In progress" || phase.state === "Current code"}>
                <summary>Features, tests, edge cases, and pass gate</summary>
                <dl>
                  <div>
                    <dt>Features</dt>
                    <dd>{phase.features}</dd>
                  </div>
                  <div>
                    <dt>How we test it</dt>
                    <dd>{phase.tests}</dd>
                  </div>
                  <div>
                    <dt>Edge cases</dt>
                    <dd>{phase.edges}</dd>
                  </div>
                  <div className="phase-gate">
                    <dt>Pass gate</dt>
                    <dd>{phase.gate}</dd>
                  </div>
                </dl>
              </details>
            </article>
          ))}
        </section>
        <div className="roadmap-actions">
          <a className="button button-primary" href="/docs/roadmap-to-1.0">
            Read the technical acceptance plan
          </a>
          <a className="button button-secondary" href="/docs/phase-status">
            Check verified delivery status
          </a>
        </div>
      </div>
    </section>
  );
}
