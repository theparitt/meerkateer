import packageMetadata from "../package.json";
import {
  ActionStoriesSection,
  IntegrationSection,
  SystemMapSection,
  UpdatesRoadmapSection,
} from "./LandingExtras";

const GITHUB_REPO_URL = "https://github.com/theparitt/meerkateer";
const PRODUCT_VERSION = `v${packageMetadata.version}`;
const COMMUNITY_SOURCE_DOWNLOAD_URL =
  "https://github.com/theparitt/meerkateer/archive/refs/heads/main.zip";
const CONTROLLER_PREVIEW_URL = `${GITHUB_REPO_URL}/releases/download/controller-preview`;
const WINDOWS_MSI_URL = `${CONTROLLER_PREVIEW_URL}/meerkateer-controller-windows-x86_64.msi`;
const WINDOWS_CLI_URL = `${CONTROLLER_PREVIEW_URL}/meerkateer-controller-windows-x86_64.zip`;
const UBUNTU_DEB_FILENAME = `meerkateer-controller_${packageMetadata.version}_amd64.deb`;
const UBUNTU_DEB_URL = `${CONTROLLER_PREVIEW_URL}/${UBUNTU_DEB_FILENAME}`;
const UBUNTU_DEB_SHA256_URL = `${UBUNTU_DEB_URL}.sha256`;
const UBUNTU_INSTALL_COMMAND = [
  `curl --fail --location --remote-name ${UBUNTU_DEB_URL}`,
  `curl --fail --location --remote-name ${UBUNTU_DEB_SHA256_URL}`,
  `sha256sum --check ${UBUNTU_DEB_FILENAME}.sha256`,
  `sudo apt install ./${UBUNTU_DEB_FILENAME}`,
].join("\n");
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

function Brand({ cloud = false }: { cloud?: boolean }) {
  return (
    <a
      className={`brand${cloud ? " brand-cloud" : ""}`}
      href="/"
      aria-label={cloud ? "Meerkateer Cloud home" : "Meerkateer home"}
    >
      <img className="brand-mascot" src="/logo.png" alt="" />
      <span>
        <span className="brand-name-row">
          <img className="brand-wordmark" src="/wordmark.png" alt="Meerkateer" />
          {cloud ? <b className="brand-cloud-badge">Cloud</b> : null}
        </span>
        <small>{cloud ? "Managed server reliability" : "Server reliability"}</small>
      </span>
    </a>
  );
}

function PublicLinks() {
  return (
    <>
      <a href="/#how-it-works">How it works</a>
      <a href="/#sdks">SDKs</a>
      <a href="/#controller">Controller</a>
      <a href="/roadmap">Roadmap</a>
      <a href="/#run-it">Community</a>
      <a href={DOCS_URL}>Docs</a>
      <GitHubLink>GitHub</GitHubLink>
    </>
  );
}

function PublicHeader({ cloud = false }: { cloud?: boolean }) {
  return (
    <header className="public-header page-width">
      <Brand cloud={cloud} />
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
    friend: "/friends/machine-scout.png",
    label: "Machines",
    title: "See who's online",
    copy: "See enrolled machines and whether their latest telemetry is still fresh.",
  },
  {
    friend: "/friends/heartbeat-keeper.png",
    label: "Services",
    title: "Follow heartbeats",
    copy: "Watch the services and processes that matter to each workspace.",
  },
  {
    friend: "/friends/timeline-guide.png",
    label: "Timeline",
    title: "Understand the story",
    copy: "See operational events, deployments, incidents, and recovery together.",
  },
  {
    friend: "/friends/workspace-organizer.png",
    label: "Workspaces",
    title: "Keep teams organized",
    copy: "Group machines and services by job or environment within one company.",
  },
  {
    friend: "/friends/state-watcher.png",
    label: "States",
    title: "Catch stale signals",
    copy: "See offline, degraded, stale, or unknown states without guessing what changed.",
  },
  {
    friend: "/friends/access-guardian.png",
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
            <div className="capability-card-head">
              <div>
                <p className="card-label">{capability.label}</p>
                <h3>{capability.title}</h3>
              </div>
              <span className="capability-friend" aria-hidden="true">
                <img
                  src={capability.friend}
                  alt=""
                  width="104"
                  height="104"
                  loading="lazy"
                  decoding="async"
                />
              </span>
            </div>
            <p>{capability.copy}</p>
          </article>
        ))}
      </div>
    </section>
  );
}

const communityBenefits = [
  "Complete Apache-2.0 reliability core",
  "One company with many workspaces and machines",
  "Your own PostgreSQL database and infrastructure",
  "Agents, SDKs, Console, migrations, and tests are public",
  "No Cloud, billing, or license-server dependency",
];

const cloudBenefits = [
  "Many isolated tenants on a managed control plane",
  "Pinned releases of the same public core—never a fork",
  "Managed upgrades, backups, regions, and service SLOs",
  "Provider quotas, abuse protection, and support operations",
  "Free hosted beta first; billing only after it is proven",
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
          copy="Community is the self-hosted open-source product. Cloud will operate immutable releases of that same core for teams that prefer a managed multi-tenant service."
        />
        <div className="run-grid">
          <RunOption
            variant="community"
            eyebrow="Meerkateer Community"
            title="Open source. Self-hosted. Free."
            copy="Run one company per installation and organize many workspaces, machines, people, and services. Community is a first-class product, not a limited trial."
            benefits={communityBenefits}
            action="Quick start"
            actionHref="/get-started"
            secondary="Explore the source"
            secondaryHref={GITHUB_REPO_URL}
          />
          <RunOption
            variant="cloud"
            eyebrow="Meerkateer Cloud"
            title="Hosted by us. Built from public core."
            copy="The private Cloud repository adds provisioning and provider operations around signed public artifacts; reliability and tenant security remain in open source."
            benefits={cloudBenefits}
            action="View Cloud status"
            actionHref="/cloud"
            secondary="Read project status"
            secondaryHref="/docs/phase-status"
            note="Cloud is planned, not generally available. The first hosted beta is free and begins only after the Operations Beta gates pass."
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

const controllerDownloads = [
  {
    platform: "Windows 10 / 11 · x64",
    title: "Windows installer",
    copy: "Install the headless background controller, use the friendly setup screen, then open the live local dashboard whenever you need it.",
    action: "Download MSI",
    href: WINDOWS_MSI_URL,
    friend: "/friends/machine-scout.png",
    accent: "windows",
  },
  {
    platform: "Ubuntu 22.04+ · amd64",
    title: "Ubuntu server package",
    copy: "Install the hardened systemd service, then use the colorful Rust terminal UI for setup, live signals, diagnostics, and settings.",
    action: "Download DEB",
    href: UBUNTU_DEB_URL,
    friend: "/friends/heartbeat-keeper.png",
    accent: "ubuntu",
  },
  {
    platform: "Windows · portable x64",
    title: "Command line only",
    copy: "Keep automation-friendly commands and launch the same Rust terminal dashboard with meerkateer-controller.exe tui.",
    action: "Download CLI ZIP",
    href: WINDOWS_CLI_URL,
    friend: "/friends/access-guardian.png",
    accent: "cli",
  },
];

const connectionChecks = [
  {
    code: "01",
    title: "Local storage",
    copy: "Can the Controller safely write its protected config and retry state, and is there enough disk space?",
  },
  {
    code: "02",
    title: "Address and DNS",
    copy: "Is the API URL valid, and can this computer resolve the hostname without leaking credentials?",
  },
  {
    code: "03",
    title: "Network route",
    copy: "Can the machine reach the port through its internet, firewall, proxy, container route, or VPN?",
  },
  {
    code: "04",
    title: "TLS and API",
    copy: "Is the certificate trusted, and does Meerkateer's public readiness endpoint answer successfully?",
  },
];

function ControllerDownloadsSection() {
  return (
    <section className="landing-section controller-section" id="controller">
      <div className="page-width">
        <SectionHeading
          eyebrow="Connect a computer"
          title="Install one small Controller."
          copy="The Controller sends outbound reliability signals to your workspace. You choose what it may report, and Meerkateer never exposes a remote shell on the machine."
        />
        <div className="controller-download-grid">
          {controllerDownloads.map((download) => (
            <article className={`controller-download-card ${download.accent}`} key={download.title}>
              <img src={download.friend} alt="" width="112" height="112" loading="lazy" />
              <p className="card-label">{download.platform}</p>
              <h3>{download.title}</h3>
              <p>{download.copy}</p>
              <a className="button button-primary" href={download.href}>
                {download.action}
              </a>
            </article>
          ))}
        </div>
        <div className="controller-linux-command">
          <div className="controller-linux-command-copy">
            <p className="card-label">Ubuntu / Debian terminal install</p>
            <h3>Download, verify, and install from the command line.</h3>
            <p>
              Paste all four lines into a terminal. The checksum is verified before the package is
              installed, so a damaged download stops safely.
            </p>
            <a className="text-link" href={UBUNTU_DEB_URL}>
              Or download the .deb directly →
            </a>
          </div>
          <section aria-label="Ubuntu install command">
            <pre>
              <code>{UBUNTU_INSTALL_COMMAND}</code>
            </pre>
          </section>
        </div>
        <div className="controller-route-panel">
          <div className="controller-route-heading">
            <p className="card-label">Choose where signals go</p>
            <h3>One clear destination during setup.</h3>
            <p>
              The destination is stored locally with the machine credential. It never silently
              changes after enrollment.
            </p>
          </div>
          <div className="controller-route-options">
            <div className="controller-route-option active">
              <span aria-hidden="true">✓</span>
              <div>
                <strong>Local / self-hosted Community</strong>
                <p>Available now. Enter the HTTPS address of the API you operate.</p>
              </div>
            </div>
            <div className="controller-route-option disabled" aria-disabled="true">
              <span aria-hidden="true">☁</span>
              <div>
                <strong>Meerkateer Cloud</strong>
                <p>Coming soon. Visible in setup, but disabled until the hosted service opens.</p>
              </div>
            </div>
          </div>
        </div>
        <div className="controller-test-panel">
          <div className="controller-test-copy">
            <p className="card-label">Test before enrollment</p>
            <h3>Find the broken step, not just “connection failed.”</h3>
            <p>
              Windows Setup and the Rust terminal UI test the route before using the one-time token.
              Runtime failures keep an actionable, redacted reason beside the local config.
            </p>
            <code>meerkateer-controller test-connection --server https://api.example.com</code>
          </div>
          <div className="controller-check-grid">
            {connectionChecks.map((check) => (
              <article key={check.code}>
                <span>{check.code}</span>
                <div>
                  <strong>{check.title}</strong>
                  <p>{check.copy}</p>
                </div>
              </article>
            ))}
          </div>
        </div>
        <p className="controller-error-note">
          Clear error families include invalid config or environment, unwritable/full disk, DNS,
          timeout, firewall or VPN route, proxy, TLS certificate, wrong API path, authentication,
          rate limiting, and API/upstream outage. A failed telemetry batch stays on the machine and
          retries without skipping its sequence.
        </p>
        <div className="controller-signal-note">
          <div>
            <strong>Always visible before it starts</strong>
            <span>Heartbeat · CPU · memory · disk totals · selected process state</span>
          </div>
          <div>
            <strong>Never collected</strong>
            <span>Files · command arguments · environment values · player/chat content</span>
          </div>
          <a className="text-link" href="/docs/agent-cli">
            Installation and CLI guide →
          </a>
        </div>
        <p className="controller-preview-warning">
          The service stays quiet and headless. Run <code>meerkateer-controller tui</code> only when
          you want the local live view; the Web Console remains the home for your whole fleet.
        </p>
        <p className="controller-preview-warning">
          Developer-preview packages are currently unsigned. Verify the adjacent SHA-256 file from
          the preview release before installation. A free Microsoft Store MSIX channel is being
          evaluated separately because background services require restricted Store approval.
        </p>
      </div>
    </section>
  );
}

function FinalCallToAction() {
  return (
    <div className="final-cta-scene">
      <div className="final-landscape" aria-hidden="true">
        <span className="final-sun" />
        <span className="final-mesa final-mesa-left" />
        <span className="final-mesa final-mesa-right" />
        <span className="final-dune final-dune-back" />
        <span className="final-dune final-dune-front" />
        <span className="final-rock final-rock-left" />
        <span className="final-rock final-rock-right" />
      </div>
      <section className="final-cta page-width">
        <img src="/logo.png" alt="" />
        <div>
          <p className="eyebrow">Ready to keep watch?</p>
          <h2>Start with Community.</h2>
          <p>Download the source, run it yourself, and follow the first monitoring journey.</p>
        </div>
        <div className="final-actions">
          <a className="button button-gold" href="/get-started">
            Quick start
          </a>
          <a className="button button-light" href={COMMUNITY_SOURCE_DOWNLOAD_URL}>
            Download source
          </a>
          <GitHubLink>View on GitHub</GitHubLink>
        </div>
      </section>
    </div>
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
          <small>Apache-2.0 · {PRODUCT_VERSION} · Developer Preview</small>
        </div>
        <div>
          <strong>Product</strong>
          <a href="#product">Overview</a>
          <a href="#how-it-works">How it works</a>
          <a href="#features">Features</a>
          <a href="#run-it">Community Edition</a>
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
          <a href={GITHUB_REPO_URL} target="_blank" rel="noopener noreferrer">
            GitHub
          </a>
          <a href="/roadmap">Road to 1.0</a>
          <a href="/roadmap#tests">Release gates</a>
          <a href="/docs/community-first-roadmap">Community strategy</a>
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
            <aside className="version-strip" aria-label="Current Meerkateer version and phase">
              <strong>{PRODUCT_VERSION}</strong>
              <span>Developer Preview</span>
              <span>Current phase: Community Alpha</span>
              <a href="/roadmap">Road to 1.0 →</a>
            </aside>
            <p className="eyebrow">Free, self-hosted Community preview</p>
            <h1>Know what broke, and when it recovered.</h1>
            <p>
              Watch the services you run, follow their heartbeats and host signals, and read the
              evidence around an outage in one place. Self-host Community today; the managed Cloud
              free beta follows after its isolation and operations gates pass.
            </p>
            <div className="hero-actions">
              <a className="button button-primary" href={COMMUNITY_SOURCE_DOWNLOAD_URL}>
                Download Community source
              </a>
              <a className="button button-secondary" href="/get-started">
                Quick start
              </a>
              <GitHubLink>
                Source code <span aria-hidden="true">→</span>
              </GitHubLink>
            </div>
            <ul className="trust-list" aria-label="Meerkateer product attributes">
              <li>Open source</li>
              <li>Self-hostable</li>
              <li>Community Edition</li>
              <li>Managed Cloud planned</li>
            </ul>
            <small className="preview-note">
              Current code version {PRODUCT_VERSION}. No stable GitHub release has been tagged yet.
            </small>
          </div>
          <ProductPreview />
        </section>
        <SystemMapSection />
        <ActionStoriesSection />
        <IntegrationSection />
        <ControllerDownloadsSection />
        <CapabilitySection />
        <DashboardPreview />
        <RunOptionsSection />
        <OpenSourceSection />
        <ResourcesSection />
        <FinalCallToAction />
      </main>
      <Footer />
    </div>
  );
}

export function RoadmapPage() {
  return (
    <div className="landing-page roadmap-page">
      <PublicHeader />
      <main className="roadmap-page-main">
        <section className="roadmap-hero page-width">
          <div className="roadmap-hero-copy">
            <a className="roadmap-back-link" href="/">
              ← Product overview
            </a>
            <p className="eyebrow">Meerkateer · Road to 1.0</p>
            <h1>Built carefully. Proven step by step.</h1>
            <p>
              A transparent plan for turning today&apos;s developer preview into a stable server
              reliability product. Every phase has a purpose, the work it includes, the difficult
              cases we test, and a clear gate it must pass.
            </p>
            <ul className="roadmap-summary" aria-label="Roadmap summary">
              <li>
                <strong>10</strong> milestones
              </li>
              <li>
                <strong>1</strong> shared open core
              </li>
              <li>
                <strong>Evidence</strong> before release
              </li>
            </ul>
          </div>
          <div className="roadmap-hero-art" aria-hidden="true">
            <span className="roadmap-orbit roadmap-orbit-one" />
            <span className="roadmap-orbit roadmap-orbit-two" />
            <img src="/logo.png" alt="" />
            <strong>Onward to v1.0!</strong>
          </div>
        </section>
        <UpdatesRoadmapSection />
      </main>
      <Footer />
    </div>
  );
}

export function CloudAccessPage() {
  return (
    <div className="landing-page access-route cloud-access-page">
      <PublicHeader cloud />
      <div className="cloud-sky" aria-hidden="true">
        <span className="sky-cloud sky-cloud-one" />
        <span className="sky-cloud sky-cloud-two" />
        <span className="sky-cloud sky-cloud-three" />
        <span className="sky-spark sky-spark-one">✦</span>
        <span className="sky-spark sky-spark-two">✦</span>
      </div>
      <main className="access-route-main cloud-access-main page-width">
        <section className="cloud-access-copy">
          <div className="cloud-phase-pill">
            <span aria-hidden="true">☁</span>
            Hosted Beta planned after v0.5 Operations Beta
          </div>
          <p className="eyebrow">Meerkateer Cloud · Free beta first</p>
          <h1>Your servers, watched from the cloud.</h1>
          <p>
            Get the same open-source Meerkateer reliability core without running the control plane
            yourself. We will manage upgrades, backups, regional probes, and service operations for
            you.
          </p>
          <ul className="cloud-benefit-list" aria-label="Planned Meerkateer Cloud benefits">
            <li>
              <span aria-hidden="true">✓</span>
              Isolated companies and workspaces
            </li>
            <li>
              <span aria-hidden="true">✓</span>
              Managed updates and verified backups
            </li>
            <li>
              <span aria-hidden="true">✓</span>
              Regional checks and published service SLOs
            </li>
          </ul>
          <div className="button-row">
            <a className="button button-primary cloud-primary-action" href="/roadmap">
              See the Cloud roadmap
            </a>
            <a className="button button-secondary" href="/get-started">
              Self-host Community now
            </a>
          </div>
          <p className="cloud-availability-note">
            Public Cloud accounts are not open yet. The first hosted beta will be free while we
            prove tenant isolation, restore, support, and real operating cost. Stripe remains off.
          </p>
          <p className="community-console-link">
            Already self-hosting? <a href="/app">Open your Community console →</a>
          </p>
          <a className="back-link" href="/">
            ← Back to overview
          </a>
        </section>
        <aside className="cloud-hero-card" aria-label="Meerkateer Cloud identity">
          <span className="cloud-card-star cloud-card-star-one" aria-hidden="true">
            ✦
          </span>
          <span className="cloud-card-star cloud-card-star-two" aria-hidden="true">
            ✦
          </span>
          <div className="cloud-mascot-stage">
            <span className="cute-cloud-face" aria-hidden="true">
              <i />
              <i />
              <b>⌣</b>
            </span>
            <img src="/logo.png" alt="Meerkateer mascot standing above a friendly cloud" />
          </div>
          <div className="cloud-logo-lockup">
            <img src="/wordmark.png" alt="Meerkateer" />
            <strong>Cloud</strong>
          </div>
          <p>Managed server reliability</p>
          <div className="cloud-preview-card">
            <span className="cloud-preview-dot" aria-hidden="true" />
            <span>
              <small>Availability</small>
              <strong>Hosted Beta · planned</strong>
            </span>
          </div>
        </aside>
      </main>
    </div>
  );
}
