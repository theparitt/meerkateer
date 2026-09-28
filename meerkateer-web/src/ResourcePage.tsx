import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import changelog from "../../CHANGELOG.md?raw";
import contributing from "../../CONTRIBUTING.md?raw";
import governance from "../../GOVERNANCE.md?raw";
import license from "../../LICENSE?raw";
import readme from "../../README.md?raw";
import security from "../../SECURITY.md?raw";
import support from "../../SUPPORT.md?raw";
import pythonSdk from "../../sdk/python/README.md?raw";
import rustSdk from "../../sdk/rust/README.md?raw";

const guideFiles = import.meta.glob("../../docs/**/*.md", {
  eager: true,
  import: "default",
  query: "?raw",
}) as Record<string, string>;

type Document = { title: string; description: string; text: string; file: string };

const documents: Record<string, Document> = {
  "/docs": {
    title: "Documentation",
    description: "The setup, architecture, and reliability guides for this developer preview.",
    text: readme,
    file: "README.md",
  },
  "/docs/python-sdk": {
    title: "Python SDK",
    description: "Send service heartbeats, events, and deployments from a Python process.",
    text: pythonSdk,
    file: "sdk/python/README.md",
  },
  "/docs/rust-sdk": {
    title: "Rust SDK",
    description: "Send typed, async reliability signals from a Rust service.",
    text: rustSdk,
    file: "sdk/rust/README.md",
  },
  "/changelog": {
    title: "Changelog",
    description:
      "Changes recorded in the project source. A public release feed is not available yet.",
    text: changelog,
    file: "CHANGELOG.md",
  },
  "/contributing": {
    title: "Contributing",
    description: "Development workflow and contribution requirements.",
    text: contributing,
    file: "CONTRIBUTING.md",
  },
  "/license": {
    title: "Apache 2.0 license",
    description: "The full license text for Meerkateer Community.",
    text: license,
    file: "LICENSE",
  },
  "/security": {
    title: "Security policy",
    description: "Supported versions, security scope, and how to report a vulnerability.",
    text: security,
    file: "SECURITY.md",
  },
  "/governance": {
    title: "Governance",
    description: "How maintainers make decisions and release software.",
    text: governance,
    file: "GOVERNANCE.md",
  },
};

for (const [file, text] of Object.entries(guideFiles)) {
  const relative = file.replace("../../docs/", "");
  const title = (relative.replace(/\.md$/, "").split("/").at(-1) ?? "guide")
    .replace(/^\d+-/, "")
    .replace(/-/g, " ");
  documents[`/docs/${relative.replace(/\.md$/, "")}`] = {
    title: title.charAt(0).toUpperCase() + title.slice(1),
    description: `Project documentation · ${relative}`,
    text,
    file: `docs/${relative}`,
  };
}

function projectHref(href: string, file: string): string {
  if (/^(https?:|mailto:|#)/.test(href)) return href;
  const base = new URL(`/${file}`, "https://meerkateer.local");
  const target = new URL(href, base);
  const pathname = target.pathname.replace(/\.md$/, "");
  if (pathname === "/README") return `/docs${target.hash}`;
  if (pathname === "/SUPPORT") return `/help${target.hash}`;
  if (pathname === "/SECURITY") return `/security${target.hash}`;
  if (pathname === "/CONTRIBUTING") return `/contributing${target.hash}`;
  if (pathname === "/GOVERNANCE") return `/governance${target.hash}`;
  if (pathname === "/CHANGELOG") return `/changelog${target.hash}`;
  if (pathname === "/sdk/python/README") return `/docs/python-sdk${target.hash}`;
  if (pathname === "/sdk/rust/README") return `/docs/rust-sdk${target.hash}`;
  if (pathname.startsWith("/docs/")) {
    return documents[pathname] ? `${pathname}${target.hash}` : "/docs";
  }
  return "/source";
}

function headingId(children: React.ReactNode) {
  return String(children)
    .toLowerCase()
    .replace(/[^a-z0-9 -]/g, "")
    .trim()
    .replace(/\s+/g, "-");
}

function MarkdownDocument({ text, file }: { text: string; file: string }) {
  return (
    <div className="resource-markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) => <a href={href ? projectHref(href, file) : "#"}>{children}</a>,
          h1: ({ children }) => <h2 id={headingId(children)}>{children}</h2>,
          h2: ({ children }) => <h3 id={headingId(children)}>{children}</h3>,
          h3: ({ children }) => <h4 id={headingId(children)}>{children}</h4>,
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}

function ResourceHeader() {
  return (
    <header className="resource-header page-width">
      <a className="brand" href="/" aria-label="Meerkateer home">
        <img className="brand-mascot" src="/logo.png" alt="" />
        <span>
          <img className="brand-wordmark" src="/wordmark.png" alt="Meerkateer" />
          <small>Server reliability</small>
        </span>
      </a>
      <nav aria-label="Resource navigation">
        <a href="/docs">Docs</a>
        <a href="/get-started">Getting started</a>
        <a href="/help">Help</a>
        <a href="/source">Source code</a>
      </nav>
    </header>
  );
}

function GuideIndex() {
  const guides = Object.entries(documents).filter(([path]) => path.startsWith("/docs/"));
  return (
    <section className="resource-panel">
      <h2>Browse the guides</h2>
      <div className="resource-card-grid">
        {guides.map(([path, doc]) => (
          <a className="resource-card" href={path} key={path}>
            <strong>{doc.title}</strong>
            <span>{doc.file}</span>
            <b aria-hidden="true">↗</b>
          </a>
        ))}
      </div>
    </section>
  );
}

function GettingStarted() {
  return (
    <div className="resource-prose">
      <section className="resource-panel">
        <h2>1. Start the local stack</h2>
        <p>
          Install Docker Engine 26+ with Compose v2, <code>make</code>, and OpenSSL. From the
          Meerkateer source directory run:
        </p>
        <pre>
          <code>make dev</code>
        </pre>
        <p>
          The console opens at <a href="http://127.0.0.1:6511">http://127.0.0.1:6511</a>. The first
          run creates a private <code>.env</code> containing a setup key.
        </p>
      </section>
      <section className="resource-panel">
        <h2>2. Create the first company</h2>
        <p>
          Open <a href="/setup">Community setup</a>. Enter the{" "}
          <code>MEERKATEER_BOOTSTRAP_TOKEN</code> from the private <code>.env</code>, your company
          details, and an owner password of at least 12 characters.
        </p>
        <p>
          If your company already exists, <a href="/login">sign in</a> with the owner email and
          password. Older installations can <a href="/recover">set an owner password</a> using the
          setup key.
        </p>
      </section>
      <section className="resource-panel">
        <h2>3. Add monitoring</h2>
        <p>
          In the console, create a workspace and enroll a machine or create a monitored service. The{" "}
          <a href="/docs">full README</a> has the agent and SDK commands.
        </p>
        <pre>
          <code>make smoke</code>
        </pre>
        <p>
          Use <code>make down</code> to stop the local stack. This is a developer preview; see{" "}
          <a href="/docs/production-roadmap">the production roadmap</a> before deploying beyond a
          trusted local environment.
        </p>
      </section>
    </div>
  );
}

function Help() {
  return (
    <div className="resource-prose">
      <section className="resource-panel">
        <h2>Common setup checks</h2>
        <ul>
          <li>
            Open <code>http://127.0.0.1:6511</code> after <code>make dev</code> reports a healthy
            stack.
          </li>
          <li>
            Use <a href="/setup">Create company</a> only for a new database. If you see{" "}
            <code>bootstrap_already_completed</code>, use <a href="/login">Sign in</a> or{" "}
            <a href="/recover">set the existing owner's password</a>.
          </li>
          <li>
            The setup key is in the installation's private <code>.env</code>. Keep it out of issue
            reports, screenshots, and chat.
          </li>
          <li>
            Run <code>make smoke</code> for a local health check; use{" "}
            <code>docker compose logs</code> for container diagnostics.
          </li>
        </ul>
      </section>
      <section className="resource-panel">
        <h2>Reporting an issue</h2>
        <p>
          The public issue tracker is not published yet. Keep a redacted report with version,
          operating system, deployment mode, steps to reproduce, expected result, and observed
          result. The <a href="https://github.com/theparitt/meerkateer">GitHub repository</a> is the
          place to follow project issues.
        </p>
        <p>
          For a suspected vulnerability, read the <a href="/security">security policy</a>. Do not
          publish sensitive details in a public report.
        </p>
      </section>
      <section className="resource-panel">
        <MarkdownDocument text={support} file="SUPPORT.md" />
      </section>
    </div>
  );
}

function Source() {
  return (
    <div className="resource-prose">
      <section className="resource-panel resource-source-callout">
        <h2>Explore Meerkateer on GitHub</h2>
        <p>Browse the Community source, project history, and issues in the GitHub repository.</p>
        <a
          className="button button-primary"
          href="https://github.com/theparitt/meerkateer"
          target="_blank"
          rel="noopener noreferrer"
        >
          View GitHub repository ↗
        </a>
      </section>
      <section className="resource-panel">
        <h2>Explore the project</h2>
        <div className="resource-card-grid">
          <a className="resource-card" href="/get-started">
            <strong>Run locally</strong>
            <span>Install and create your company</span>
          </a>
          <a className="resource-card" href="/changelog">
            <strong>Changelog</strong>
            <span>Changes in the preview</span>
          </a>
          <a className="resource-card" href="/contributing">
            <strong>Contributing</strong>
            <span>Workflow and project standards</span>
          </a>
          <a className="resource-card" href="/docs/phase-status">
            <strong>Project status</strong>
            <span>Milestones and remaining work</span>
          </a>
        </div>
      </section>
    </div>
  );
}

const specialPages: Record<
  string,
  { title: string; description: string; content: () => React.JSX.Element }
> = {
  "/get-started": {
    title: "Getting started",
    description: "Start Community locally and create your owner account.",
    content: GettingStarted,
  },
  "/help": {
    title: "Help",
    description: "Troubleshoot setup and prepare a useful support report.",
    content: Help,
  },
  "/source": {
    title: "Source code",
    description: "Inspect, download, and follow the Community project.",
    content: Source,
  },
};

export function isResourcePath(path: string) {
  return path in documents || path in specialPages;
}

export function ResourcePage({ path }: { path: string }) {
  const document = documents[path];
  const special = specialPages[path];
  const title = document?.title ?? special.title;
  const description = document?.description ?? special.description;
  const Content = special?.content;
  return (
    <div className="resource-page">
      <ResourceHeader />
      <main className="resource-main page-width">
        <a className="resource-back" href="/">
          ← Back to overview
        </a>
        <div className="resource-intro">
          <p className="eyebrow">Meerkateer Community</p>
          <h1>{title}</h1>
          <p>{description}</p>
        </div>
        {path === "/docs" ? <GuideIndex /> : null}
        {document ? (
          <section className="resource-panel">
            <MarkdownDocument text={document.text} file={document.file} />
          </section>
        ) : null}
        {Content ? <Content /> : null}
      </main>
      <footer className="resource-bottom page-width">
        <span>Meerkateer · Developer preview</span>
        <a href="/license">Apache-2.0 license</a>
      </footer>
    </div>
  );
}
