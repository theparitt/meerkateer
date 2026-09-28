import { useState } from "react";
import "./landing-extras.css";

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
    link: "/#sdks",
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

export function IntegrationSection() {
  const [active, setActive] = useState<Sample>("node");
  const [copied, setCopied] = useState(false);
  const sample = samples[active];

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
          <pre role="tabpanel">
            <code>{sample.code}</code>
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

export function UpdatesRoadmapSection() {
  return (
    <section className="landing-section updates-section" id="updates">
      <div className="page-width">
        <div className="updates-heading">
          <div className="section-heading">
            <p className="eyebrow">Project progress</p>
            <h2>New things, tested things, next things.</h2>
            <p>
              Follow the developer preview with a clear view of what works today and what is still
              being built.
            </p>
          </div>
          <a className="button button-secondary" href="/changelog">
            Full changelog ↗
          </a>
        </div>
        <div className="updates-grid">
          <article className="update-card update-news">
            <span className="update-icon" aria-hidden="true">
              ✦
            </span>
            <p className="eyebrow">Latest updates</p>
            <h3>Fresh from the workshop</h3>
            <ul>
              <li>Owner sign-in with email and password</li>
              <li>Workspace, machine, and process setup in the Console</li>
              <li>Public GitHub repository and SDK guides</li>
            </ul>
            <a href="/changelog">See what changed →</a>
          </article>
          <article className="update-card update-tests" id="tests">
            <span className="update-icon" aria-hidden="true">
              ✓
            </span>
            <p className="eyebrow">How we test</p>
            <h3>Signals we verify</h3>
            <ul>
              <li>Web behavior and accessible routes</li>
              <li>Rust and SDK unit tests</li>
              <li>API, database, and agent integration flows</li>
              <li>Protocol contracts and secret checks</li>
            </ul>
            <a href="/docs/phase-status">Read the test evidence →</a>
          </article>
          <article className="update-card update-roadmap">
            <span className="update-icon" aria-hidden="true">
              ↗
            </span>
            <p className="eyebrow">Roadmap</p>
            <h3>What comes next</h3>
            <div className="roadmap-step">
              <b>Now</b>
              <span>Minecraft Java / Paper: manual status test and incident timeline</span>
            </div>
            <div className="roadmap-step">
              <b>Next</b>
              <span>Scheduled probes, Paper TPS/MSPT collector, calm Discord alerts</span>
            </div>
            <div className="roadmap-step">
              <b>Later</b>
              <span>Player status pages, pilot operators, managed Cloud</span>
            </div>
            <a href="/docs/game-server-beta">Explore the Minecraft pilot →</a>
          </article>
        </div>
      </div>
    </section>
  );
}
