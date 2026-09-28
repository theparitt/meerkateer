import {
  ActionStoriesSection,
  IntegrationSection,
  SystemMapSection,
  UpdatesRoadmapSection,
} from "./LandingExtras";

const GITHUB_REPO_URL = "https://github.com/theparitt/meerkateer";
const DOCS_URL = "/docs";
const SELF_HOST_URL = "/get-started";

function GitHubIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="currentColor" width="17" height="17">
      <path d="M12 .75a11.25 11.25 0 0 0-3.56 21.93c.56.1.77-.24.77-.54v-1.91c-3.13.68-3.79-1.33-3.79-1.33-.51-1.3-1.25-1.65-1.25-1.65-1.02-.7.08-.69.08-.69 1.13.08 1.72 1.16 1.72 1.16 1 .1 2.2-.51 2.73-1.02.1-.73.39-1.23.71-1.51-2.5-.28-5.13-1.25-5.13-5.56 0-1.23.44-2.23 1.16-3.01-.12-.28-.5-1.43.11-2.98 0 0 .95-.3 3.1 1.15a10.8 10.8 0 0 1 5.65 0c2.15-1.45 3.1-1.15 3.1-1.15.61 1.55.23 2.7.11 2.98.72.78 1.16 1.78 1.16 3.01 0 4.32-2.64 5.28-5.15 5.55.4.35.76 1.03.76 2.08v3.08c0 .3.2.65.78.54A11.25 11.25 0 0 0 12 .75Z" />
    </svg>
  );
}

function GitHubLink({ children }: { children: React.ReactNode }) {
  return (
    <a className="github-link" href={GITHUB_REPO_URL} target="_blank" rel="noopener noreferrer">
      <GitHubIcon /> {children}
    </a>
  );
}

function Brand() {
  return (
    <a className="brand" href="/" aria-label="Meerkateer home">
      <img className="brand-mascot" src="/logo.png" alt="" />
      <span>
        <img className="brand-wordmark" src="/wordmark.png" alt="Meerkateer" />
        <small>Server reliability</small>
      </span>
    </a>
  );
}

function PublicLinks() {
  return (
    <>
      <a href="/#how-it-works">How it works</a>
      <a href="/#sdks">SDKs</a>
      <a href="/#updates">Updates</a>
      <a href="/#run-it">Community</a>
      <a href={DOCS_URL}>Docs</a>
      <GitHubLink>GitHub</GitHubLink>
    </>
  );
}

function PublicHeader() {
  return (
    <header className="public-header page-width">
      <Brand />
      <nav className="public-nav" aria-label="Primary navigation">
        <PublicLinks />
      </nav>
      <div className="public-actions">
        <a className="text-link" href="/login">
          Sign in
        </a>
        <a className="button button-small button-primary" href="/get-started">
          Get started
        </a>
      </div>
      <details className="public-menu">
        <summary aria-label="Open navigation">Menu</summary>
        <nav aria-label="Mobile navigation">
          <PublicLinks />
          <a href="/login">Sign in</a>
          <a href="/get-started">Get started</a>
        </nav>
      </details>
    </header>
  );
}

function SectionHeading({
  eyebrow,
  title,
  copy,
}: {
  eyebrow: string;
  title: string;
  copy: string;
}) {
  return (
    <div className="section-heading">
      <p className="eyebrow">{eyebrow}</p>
      <h2>{title}</h2>
      <p>{copy}</p>
    </div>
  );
}

function ProductPreview() {
  return (
    <div className="hero-product">
      <img src="/logo.png" alt="Meerkateer mascot" />
      <div className="hero-monitor-card">
        <div className="monitor-card-head">
          <span>Fleet overview</span>
          <span className="live-label">Example</span>
        </div>
        <strong>3 servers online</strong>
        <div className="monitor-row">
          <span className="monitor-status status-good" />
          <span>
            <strong>api-01</strong>
            <small>Online · fresh telemetry</small>
          </span>
          <b>Healthy</b>
        </div>
        <div className="monitor-row">
          <span className="monitor-status status-warn" />
          <span>
            <strong>worker-04</strong>
            <small>Telemetry stale · 8m</small>
          </span>
          <b>Check</b>
        </div>
      </div>
    </div>
  );
}

const capabilities = [
  {
    icon: "●",
    label: "Machines",
    title: "See who's online",
    copy: "See enrolled machines and whether their latest telemetry is still fresh.",
  },
  {
    icon: "♥",
    label: "Services",
    title: "Follow heartbeats",
    copy: "Watch the services and processes that matter to each workspace.",
  },
  {
    icon: "↗",
    label: "Timeline",
    title: "Understand the story",
    copy: "See operational events, deployments, incidents, and recovery together.",
  },
  {
    icon: "▦",
    label: "Workspaces",
    title: "Keep teams organized",
    copy: "Group machines and services by job or environment within one company.",
  },
  {
    icon: "!",
    label: "States",
    title: "Catch stale signals",
    copy: "See offline, degraded, stale, or unknown states without guessing what changed.",
  },
  {
    icon: "⚿",
    label: "Access",
    title: "Control credentials",
    copy: "Issue scoped enrollment tokens and one-time service keys from the Console.",
  },
];

function CapabilitySection() {
  return (
    <section className="landing-section page-width" id="product">
      <SectionHeading
        eyebrow="What is Meerkateer?"
        title="One place to watch your servers."
        copy="Meerkateer keeps the important reliability signals together so you can see when something goes offline, becomes stale or degraded, and when it recovers."
      />
      <div className="capability-grid" id="features">
        {capabilities.map((capability) => (
          <article className="capability-card" key={capability.label}>
            <span className="capability-icon" aria-hidden="true">
              {capability.icon}
            </span>
            <p className="card-label">{capability.label}</p>
            <h3>{capability.title}</h3>
            <p>{capability.copy}</p>
          </article>
        ))}
      </div>
    </section>
  );
}

const communityBenefits = [
  "Apache-2.0 open-source core",
  "Run on your own server",
  "Keep infrastructure and data under your control",
  "Source available to inspect",
  "No hosted account required",
];

const cloudBenefits = [
  "No control-plane installation",
  "Managed infrastructure",
  "Platform updates handled for you",
  "Create a workspace and start monitoring",
  "The same Meerkateer experience",
];

function RunOption({
  variant,
  eyebrow,
  title,
  copy,
  benefits,
  action,
  actionHref,
  secondary,
  secondaryHref,
  note,
}: {
  variant: "community" | "cloud";
  eyebrow: string;
  title: string;
  copy: string;
  benefits: string[];
  action: string;
  actionHref: string;
  secondary: string;
  secondaryHref: string;
  note?: string;
}) {
  return (
    <article className={`run-card run-card-${variant}`}>
      <p className="eyebrow">{eyebrow}</p>
      <h3>{title}</h3>
      <p className="run-copy">{copy}</p>
      <ul>
        {benefits.map((benefit) => (
          <li key={benefit}>
            <span aria-hidden="true">✓</span>
            {benefit}
          </li>
        ))}
      </ul>
      <div className="run-actions">
        <a className="button button-primary" href={actionHref}>
          {action}
        </a>
        <a className="text-link" href={secondaryHref}>
          {secondary} <span aria-hidden="true">→</span>
        </a>
      </div>
      {note ? <small className="availability-note">{note}</small> : null}
    </article>
  );
}

function RunOptionsSection() {
  return (
    <section className="landing-section run-section" id="run-it">
      <div className="page-width">
        <SectionHeading
          eyebrow="Choose how you run it"
          title="Use Meerkateer your way."
          copy="Same open-source project. Run it on infrastructure you control, or choose the managed service when you do not want to operate the control plane."
        />
        <div className="run-grid">
          <RunOption
            variant="community"
            eyebrow="Meerkateer Community"
            title="Open source. Self-hosted. Free."
            copy="For developers and teams who want full control over where Meerkateer runs. Community is a first-class product, not a limited trial."
            benefits={communityBenefits}
            action="Self-host Meerkateer"
            actionHref="/setup"
            secondary="Explore the source"
            secondaryHref={GITHUB_REPO_URL}
          />
          <RunOption
            variant="cloud"
            eyebrow="Meerkateer Cloud"
            title="Hosted by us. No control-plane upkeep."
            copy="Use Meerkateer without installing, upgrading, or maintaining the control plane yourself. We run the platform for you."
            benefits={cloudBenefits}
            action="Explore Meerkateer Cloud"
            actionHref="/cloud"
            secondary="Cloud availability"
            secondaryHref="/cloud"
            note="Cloud onboarding is still in developer preview in this build."
          />
        </div>
      </div>
    </section>
  );
}

function OpenSourceSection() {
  return (
    <section className="landing-section page-width open-source-section">
      <div>
        <p className="eyebrow">Open source</p>
        <h2>Open by default.</h2>
        <p>
          Meerkateer Community runs on infrastructure you control. Explore the project on GitHub,
          inspect how it works, and follow its progress.
        </p>
        <div className="button-row">
          <a
            className="button button-primary github-link"
            href={GITHUB_REPO_URL}
            target="_blank"
            rel="noopener noreferrer"
          >
            <GitHubIcon /> View on GitHub
          </a>
          <a className="button button-secondary" href={SELF_HOST_URL}>
            Read self-hosting guide
          </a>
        </div>
      </div>
      <div
        className="terminal-card"
        role="img"
        aria-label="Terminal showing the local startup command"
      >
        <div className="terminal-top">
          <span />
          <span />
          <span />
          <small>meerkateer — community</small>
        </div>
        <code>
          <span>$</span> make dev
        </code>
        <p>PostgreSQL · API · Worker · Console</p>
        <strong>
          <span aria-hidden="true">●</span> Ready on localhost
        </strong>
      </div>
    </section>
  );
}

function DashboardPreview() {
  return (
    <section className="landing-section preview-section">
      <div className="page-width preview-layout">
        <SectionHeading
          eyebrow="Product preview"
          title="See what needs attention."
          copy="Current state, machine freshness, and the events around an outage stay together in the selected workspace."
        />
        <div className="mock-dashboard">
          <div className="mock-topbar">
            <span>Bangkok game fleet</span>
            <small>Illustrative preview</small>
          </div>
          <div className="mock-stats">
            <div>
              <small>Control plane</small>
              <strong>
                <i className="mock-dot online" />
                Operational
              </strong>
            </div>
            <div>
              <small>Machines online</small>
              <strong>12</strong>
            </div>
            <div>
              <small>Needs attention</small>
              <strong className="attention-number">2</strong>
            </div>
          </div>
          <div className="mock-activity">
            <h3>Recent activity</h3>
            <div>
              <i className="mock-dot online" />
              <span>
                <strong>api-prod-02 recovered</strong>
                <small>Healthy heartbeat received</small>
              </span>
              <time>2m ago</time>
            </div>
            <div>
              <i className="mock-dot warning" />
              <span>
                <strong>worker-04 telemetry stale</strong>
                <small>Last heartbeat is outside the freshness window</small>
              </span>
              <time>8m ago</time>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

const resources = [
  [
    "Docs",
    "Documentation",
    "Installation, configuration, deployment, and usage guides.",
    DOCS_URL,
    "Read the docs",
  ],
  [
    "Help",
    "Help",
    "Troubleshoot setup and prepare a clear, redacted issue report.",
    "/help",
    "Get help",
  ],
  [
    "Code",
    "GitHub repository",
    "Browse the code, issues, and project history on GitHub.",
    GITHUB_REPO_URL,
    "Open GitHub",
  ],
];

function ResourcesSection() {
  return (
    <section className="landing-section page-width resources-section" id="resources">
      <SectionHeading
        eyebrow="Docs and help"
        title="You're not on your own."
        copy="Start with the documentation, find setup help, or explore the Community source."
      />
      <div className="resource-grid">
        {resources.map(([icon, title, copy, href, action]) => (
          <article key={title}>
            <span className="resource-icon" aria-hidden="true">
              {icon}
            </span>
            <h3>{title}</h3>
            <p>{copy}</p>
            <a className="text-link" href={href}>
              {action} <span aria-hidden="true">→</span>
            </a>
          </article>
        ))}
      </div>
    </section>
  );
}

function FinalCallToAction() {
  return (
    <section className="final-cta page-width">
      <img src="/logo.png" alt="" />
      <div>
        <p className="eyebrow">Ready to keep watch?</p>
        <h2>Choose the setup that works for you.</h2>
        <p>Self-host Meerkateer for free, or use the managed service when available.</p>
      </div>
      <div className="final-actions">
        <a className="button button-gold" href="/get-started">
          Start with Community
        </a>
        <a className="button button-light" href="/cloud">
          Explore Cloud preview
        </a>
        <GitHubLink>View on GitHub</GitHubLink>
      </div>
    </section>
  );
}

function Footer() {
  return (
    <footer className="public-footer">
      <div className="page-width footer-layout">
        <div className="footer-brand">
          <Brand />
          <p>
            Open-source server reliability for teams that want a clear view of outages and recovery.
          </p>
          <small>Apache-2.0 · Developer preview</small>
        </div>
        <div>
          <strong>Product</strong>
          <a href="#product">Overview</a>
          <a href="#how-it-works">How it works</a>
          <a href="#features">Features</a>
          <a href="/cloud">Cloud</a>
          <a href="/#run-it">Community Edition</a>
        </div>
        <div>
          <strong>Resources</strong>
          <a href={DOCS_URL}>Documentation</a>
          <a href={SELF_HOST_URL}>Getting started</a>
          <a href="#sdks">SDKs and examples</a>
          <a href="/help">Help</a>
        </div>
        <div>
          <strong>Open source</strong>
          <GitHubLink>GitHub</GitHubLink>
          <a href="#updates">Updates</a>
          <a href="#tests">Tests</a>
          <a href="/docs/commercial-readiness-plan">Roadmap</a>
          <a href="/docs/phase-status">Project status</a>
          <a href="/changelog">Changelog</a>
          <a href="/contributing">Contributing</a>
        </div>
        <div>
          <strong>Project</strong>
          <a href="/license">License</a>
          <a href="/security">Security</a>
          <a href="/governance">Governance</a>
        </div>
      </div>
    </footer>
  );
}

export function LandingPage() {
  return (
    <div className="landing-page">
      <PublicHeader />
      <main className="landing-main">
        <section className="landing-hero page-width">
          <div className="landing-hero-copy">
            <p className="eyebrow">Game-server reliability · Minecraft pilot</p>
            <h1>Know when your servers need you.</h1>
            <p>
              Keep your game panel. Add a clearer view of server response, internal signals,
              outages, and recovery. Minecraft Java / Paper is the first pilot; Community is free to
              self-host and managed Cloud is on the roadmap.
            </p>
            <div className="hero-actions">
              <a className="button button-primary" href="/get-started">
                Get started with Community
              </a>
              <a className="button button-secondary" href="/cloud">
                Explore Cloud preview
              </a>
              <GitHubLink>
                View on GitHub <span aria-hidden="true">→</span>
              </GitHubLink>
              <a className="text-link" href="/docs/game-server-beta">
                Minecraft pilot plan <span aria-hidden="true">→</span>
              </a>
            </div>
            <ul className="trust-list" aria-label="Meerkateer product attributes">
              <li>Open source</li>
              <li>Self-hostable</li>
              <li>Community Edition</li>
              <li>Cloud planned</li>
            </ul>
            <small className="preview-note">
              Developer preview · Cloud onboarding is not yet generally available.
            </small>
          </div>
          <ProductPreview />
        </section>
        <SystemMapSection />
        <ActionStoriesSection />
        <IntegrationSection />
        <CapabilitySection />
        <DashboardPreview />
        <UpdatesRoadmapSection />
        <RunOptionsSection />
        <OpenSourceSection />
        <ResourcesSection />
        <FinalCallToAction />
      </main>
      <Footer />
    </div>
  );
}

export function CloudAccessPage() {
  return (
    <div className="landing-page access-route">
      <PublicHeader />
      <main className="access-route-main page-width">
        <section>
          <p className="eyebrow">Meerkateer Cloud</p>
          <h1>Managed Meerkateer is in developer preview.</h1>
          <p>
            This build does not enable public Cloud accounts yet. Community remains fully usable and
            free to self-host while managed onboarding is completed.
          </p>
          <div className="button-row">
            <a className="button button-primary" href="/setup">
              Set up Community
            </a>
            <a className="button button-secondary" href="/docs/phase-status">
              Follow development
            </a>
          </div>
          <p className="community-console-link">
            Already self-hosting? <a href="/app">Open your Community console →</a>
          </p>
          <a className="back-link" href="/">
            ← Back to overview
          </a>
        </section>
        <img src="/logo.png" alt="Meerkateer mascot" />
      </main>
    </div>
  );
}
