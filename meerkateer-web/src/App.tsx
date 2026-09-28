import { type FormEvent, useEffect, useState } from "react";
import { AvailabilityBoard } from "./AvailabilityBoard";
import {
  type AgentResponse,
  assignWorkspaceAgent,
  bootstrapCommunity,
  type CreateProjectRequest,
  type CreateServiceRequest,
  createService,
  createWorkspace,
  fetchAgents,
  fetchHealth,
  fetchProjects,
  fetchServices,
  fetchSession,
  fetchTimeline,
  fetchWorkspaceAgents,
  type HealthResponse,
  type IssuedCredentialResponse,
  type IssuedEnrollmentTokenResponse,
  issueServiceCredential,
  issueWorkspaceEnrollmentToken,
  type ProjectResponse,
  passwordLogin,
  type ServiceResponse,
  type SessionResponse,
  setupOwnerPassword,
  type TimelineItemResponse,
  unassignWorkspaceAgent,
} from "./api";
import { GameProbePanel } from "./GameProbePanel";
import { CloudAccessPage, LandingPage } from "./LandingPage";
import { isResourcePath, ResourcePage } from "./ResourcePage";

type HealthState =
  | { phase: "loading" }
  | { phase: "ready"; health: HealthResponse }
  | { phase: "error"; message: string };

type DashboardReady = {
  phase: "ready";
  session: SessionResponse;
  projects: ProjectResponse[];
  projectId: string | null;
  agents: AgentResponse[];
  companyAgents: AgentResponse[];
  services: ServiceResponse[];
  serviceId: string | null;
  timeline: TimelineItemResponse[];
};

type DashboardState =
  | { phase: "loading" }
  | { phase: "signed-out" }
  | { phase: "error"; message: string }
  | DashboardReady;

type ManagementActions = {
  createWorkspace: (input: CreateProjectRequest) => Promise<void>;
  createService: (projectId: string, input: CreateServiceRequest) => Promise<ServiceResponse>;
  issueServiceCredential: (serviceId: string) => Promise<IssuedCredentialResponse>;
  assignAgent: (projectId: string, agentId: string) => Promise<void>;
  unassignAgent: (projectId: string, agentId: string) => Promise<void>;
};

function message(error: unknown): string {
  return error instanceof Error ? error.message : "Unable to reach the control plane";
}

export function App() {
  const path = window.location.pathname.replace(/\/$/, "") || "/";
  if (path === "/") return <LandingPage />;
  if (path === "/cloud") return <CloudAccessPage />;
  if (path === "/login") return <CommunityAccessPage mode="login" />;
  if (path === "/setup") return <CommunityAccessPage mode="setup" />;
  if (path === "/recover") return <CommunityAccessPage mode="recover" />;
  if (isResourcePath(path)) return <ResourcePage path={path} />;
  return <ConsoleApp />;
}

type AccessMode = "login" | "setup" | "recover";

const accessContent = {
  login: {
    eyebrow: "Community sign in",
    title: "Welcome back.",
    copy: "Sign in with the owner email and password for your self-hosted Meerkateer. If you previously used a setup key to sign in, set your password first.",
    link: "/recover",
    linkText: "Set or recover your password",
  },
  setup: {
    eyebrow: "First installation",
    title: "Make it yours.",
    copy: "Start the free Community stack on your infrastructure, then use the setup key from your private .env file once to create your company and owner account.",
    link: "/get-started",
    linkText: "Read the self-hosting guide",
  },
  recover: {
    eyebrow: "Owner access",
    title: "Set a new password.",
    copy: "This installation already has a company. Use the setup key from its private .env file to set or recover the owner's password, then sign in normally.",
    link: "/login",
    linkText: "Back to sign in",
  },
} satisfies Record<
  AccessMode,
  { eyebrow: string; title: string; copy: string; link: string; linkText: string }
>;

function CommunityAccessPage({ mode }: { mode: AccessMode }) {
  const content = accessContent[mode];
  return (
    <div className="setup-page page-width">
      <header className="setup-header">
        <a className="brand" href="/" aria-label="Meerkateer home">
          <img className="brand-mascot" src="/logo.png" alt="" />
          <span>
            <img className="brand-wordmark" src="/wordmark.png" alt="Meerkateer" />
            <small>Server reliability</small>
          </span>
        </a>
        <a href="/">← Product overview</a>
      </header>
      <main className="setup-main">
        <div className="setup-intro">
          <p className="eyebrow">{content.eyebrow}</p>
          <h1>{content.title}</h1>
          <p>{content.copy}</p>
          <a className="button button-secondary" href={content.link}>
            {content.linkText} <span aria-hidden="true">↗</span>
          </a>
          {mode === "login" ? (
            <p className="access-extra">
              New installation? <a href="/setup">Set up Community →</a>
            </p>
          ) : null}
          {mode === "setup" ? (
            <p className="access-extra">
              Already created a company? <a href="/recover">Set your owner password →</a>
            </p>
          ) : null}
        </div>
        <CommunityAccess mode={mode} onAuthenticated={() => window.location.assign("/app")} />
      </main>
    </div>
  );
}

function ConsoleApp() {
  const [health, setHealth] = useState<HealthState>({ phase: "loading" });
  const [dashboard, setDashboard] = useState<DashboardState>({ phase: "loading" });

  useEffect(() => {
    const controller = new AbortController();
    fetchHealth(controller.signal)
      .then((value) => setHealth({ phase: "ready", health: value }))
      .catch((error: unknown) => {
        if (!controller.signal.aborted) setHealth({ phase: "error", message: message(error) });
      });
    return () => controller.abort();
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    void loadDashboard(controller.signal, setDashboard);
    return () => controller.abort();
  }, []);

  function reloadDashboard() {
    const controller = new AbortController();
    setDashboard({ phase: "loading" });
    void loadDashboard(controller.signal, setDashboard);
  }

  async function selectProject(projectId: string) {
    if (dashboard.phase !== "ready") return;
    const controller = new AbortController();
    try {
      const [services, agents] = await Promise.all([
        fetchServices(projectId, controller.signal),
        fetchWorkspaceAgents(projectId, controller.signal),
      ]);
      const serviceId = services[0]?.id ?? null;
      const timeline = serviceId ? await fetchTimeline(serviceId, controller.signal) : [];
      setDashboard({ ...dashboard, projectId, agents, services, serviceId, timeline });
    } catch (error) {
      setDashboard({ phase: "error", message: message(error) });
    }
  }

  async function selectService(serviceId: string) {
    if (dashboard.phase !== "ready") return;
    const controller = new AbortController();
    try {
      const timeline = await fetchTimeline(serviceId, controller.signal);
      setDashboard({ ...dashboard, serviceId, timeline });
    } catch (error) {
      setDashboard({ phase: "error", message: message(error) });
    }
  }

  async function createWorkspaceAction(input: CreateProjectRequest) {
    if (dashboard.phase !== "ready") throw new Error("Dashboard is not ready");
    const created = await createWorkspace(input);
    setDashboard({
      ...dashboard,
      projects: [...dashboard.projects, created],
      projectId: created.id,
      agents: [],
      services: [],
      serviceId: null,
      timeline: [],
    });
  }

  async function createServiceAction(projectId: string, input: CreateServiceRequest) {
    if (dashboard.phase !== "ready") throw new Error("Dashboard is not ready");
    const created = await createService(projectId, input);
    setDashboard({
      ...dashboard,
      services: [...dashboard.services, created],
      serviceId: created.id,
      timeline: [],
    });
    return created;
  }

  async function updateAgentAssignment(projectId: string, agentId: string, assign: boolean) {
    if (dashboard.phase !== "ready") throw new Error("Dashboard is not ready");
    if (assign) await assignWorkspaceAgent(projectId, agentId);
    else await unassignWorkspaceAgent(projectId, agentId);
    const controller = new AbortController();
    const [agents, companyAgents] = await Promise.all([
      fetchWorkspaceAgents(projectId, controller.signal),
      fetchAgents(controller.signal),
    ]);
    setDashboard({ ...dashboard, agents, companyAgents });
  }

  if (dashboard.phase === "signed-out") return <CommunityAccessPage mode="login" />;

  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="Meerkateer home">
          <img className="brand-mascot" src="/logo.png" alt="" />
          <span>
            <img className="brand-wordmark" src="/wordmark.png" alt="Meerkateer" />
            <small>Server reliability</small>
          </span>
        </a>
        <span className="preview-badge">
          <span className="preview-dot" aria-hidden="true" />
          {dashboard.phase === "ready" ? dashboard.session.role : "Developer preview"}
        </span>
      </header>

      <main>
        <section className="hero" aria-labelledby="page-title">
          <div className="hero-content">
            <p className="eyebrow">Live operations</p>
            <h1 id="page-title">
              Keep watch.
              <span>Stay ready.</span>
            </h1>
            <p className="hero-copy">
              See the outage, the recovery, and every important moment between — without losing
              sight of your fleet.
            </p>
          </div>
          <div className="hero-mascot" aria-hidden="true">
            <span className="mascot-message">Your tiny uptime guardian</span>
            <img src="/logo.png" alt="" />
          </div>
        </section>

        <section className="status-grid" aria-label="Control plane status">
          <article className="status-card status-card-primary">
            <div>
              <p className="card-label">Control plane</p>
              <HealthContent state={health} />
            </div>
            <span className={`pulse ${health.phase === "ready" ? "pulse-live" : ""}`} />
          </article>
          <SummaryCards dashboard={dashboard} />
        </section>

        <Dashboard
          state={dashboard}
          onAuthenticated={reloadDashboard}
          onProject={(id) => void selectProject(id)}
          onService={(id) => void selectService(id)}
          management={{
            createWorkspace: createWorkspaceAction,
            createService: createServiceAction,
            issueServiceCredential,
            assignAgent: (projectId, agentId) => updateAgentAssignment(projectId, agentId, true),
            unassignAgent: (projectId, agentId) => updateAgentAssignment(projectId, agentId, false),
          }}
        />
      </main>
    </div>
  );
}

async function loadDashboard(signal: AbortSignal, setState: (state: DashboardState) => void) {
  try {
    const session = await fetchSession(signal);
    if (!session) return setState({ phase: "signed-out" });
    const [projects, companyAgents] = await Promise.all([
      fetchProjects(signal),
      fetchAgents(signal),
    ]);
    const projectId = projects[0]?.id ?? null;
    const [services, agents] = projectId
      ? await Promise.all([
          fetchServices(projectId, signal),
          fetchWorkspaceAgents(projectId, signal),
        ])
      : [[], []];
    const serviceId = services[0]?.id ?? null;
    const timeline = serviceId ? await fetchTimeline(serviceId, signal) : [];
    setState({
      phase: "ready",
      session,
      projects,
      projectId,
      agents,
      companyAgents,
      services,
      serviceId,
      timeline,
    });
  } catch (error) {
    if (!signal.aborted) setState({ phase: "error", message: message(error) });
  }
}

function HealthContent({ state }: { state: HealthState }) {
  if (state.phase === "loading")
    return (
      <div role="status">
        <h2>Connecting</h2>
        <p>Checking API…</p>
      </div>
    );
  if (state.phase === "error")
    return (
      <div role="alert">
        <h2>API unavailable</h2>
        <p>{state.message}</p>
      </div>
    );
  return (
    <div role="status">
      <h2>{state.health.status === "ok" ? "Operational" : "Degraded"}</h2>
      <p>
        {state.health.service} · {state.health.environment} · v{state.health.version}
      </p>
    </div>
  );
}

function SummaryCards({ dashboard }: { dashboard: DashboardState }) {
  const services = dashboard.phase === "ready" ? dashboard.services : [];
  const agents = dashboard.phase === "ready" ? dashboard.agents : [];
  const healthy = agents.filter((item) => item.connection_state === "online").length;
  const attention =
    services.filter((item) => item.status.state !== "online").length +
    agents.filter((item) => item.connection_state !== "online").length;
  return (
    <>
      <article className="status-card">
        <p className="card-label">Machines online</p>
        <h2>{healthy}</h2>
        <p>Workspace machines with fresh telemetry.</p>
      </article>
      <article className="status-card">
        <p className="card-label">Needs attention</p>
        <h2>{attention}</h2>
        <p>Offline, degraded, stale, or unknown.</p>
      </article>
    </>
  );
}

function Dashboard({
  state,
  onAuthenticated,
  onProject,
  onService,
  management,
}: {
  state: DashboardState;
  onAuthenticated: () => void;
  onProject: (id: string) => void;
  onService: (id: string) => void;
  management: ManagementActions;
}) {
  if (state.phase === "loading")
    return (
      <section className="next-panel" role="status">
        <div>
          <p className="eyebrow">Workspace</p>
          <h2>Loading operations</h2>
        </div>
        <p>Reading tenant-scoped services and incident history…</p>
      </section>
    );
  if (state.phase === "signed-out")
    return <CommunityAccess mode="login" onAuthenticated={onAuthenticated} />;
  if (state.phase === "error")
    return (
      <section className="next-panel" role="alert">
        <div>
          <p className="eyebrow">Workspace</p>
          <h2>Dashboard unavailable</h2>
        </div>
        <p>{state.message}</p>
      </section>
    );
  return (
    <Operations ready={state} onProject={onProject} onService={onService} management={management} />
  );
}

function CommunityAccess({
  onAuthenticated,
  mode,
}: {
  onAuthenticated: () => void;
  mode: AccessMode;
}) {
  const [pending, setPending] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const formElement = event.currentTarget;
    const form = new FormData(formElement);
    const password = String(form.get(mode === "login" ? "password" : "owner_password") ?? "");
    if (mode !== "login" && password !== String(form.get("confirm_password") ?? "")) {
      setFeedback("Passwords do not match.");
      return;
    }
    setPending(true);
    setFeedback(null);
    try {
      if (mode === "login") {
        await passwordLogin(String(form.get("email") ?? ""), password);
      } else if (mode === "recover") {
        await setupOwnerPassword(String(form.get("admin_token") ?? ""), password);
      } else {
        await bootstrapCommunity(String(form.get("admin_token") ?? ""), {
          tenant_slug: String(form.get("tenant_slug") ?? ""),
          tenant_name: String(form.get("tenant_name") ?? ""),
          owner_email: String(form.get("owner_email") ?? ""),
          owner_name: String(form.get("owner_name") ?? ""),
          owner_password: password,
        });
      }
      formElement.reset();
      onAuthenticated();
    } catch (error) {
      const code = message(error);
      const messages: Record<string, string> = {
        invalid_credentials:
          "The email or password is incorrect. If this is an older installation, set an owner password using the setup key.",
        invalid_admin_token:
          "That setup key does not match this installation. Check MEERKATEER_BOOTSTRAP_TOKEN in its private .env file.",
        invalid_bootstrap_token:
          "That setup key does not match this installation. Check MEERKATEER_BOOTSTRAP_TOKEN in its private .env file.",
        bootstrap_already_completed:
          "A company already exists here. Use the existing company and set or recover its owner password.",
        bootstrap_required: "Create the first company before signing in.",
        invalid_password: "Use a password of at least 12 characters.",
        rate_limited: "Too many attempts. Please wait a minute and try again.",
        database_unavailable: "The database is unavailable. Check the local server and try again.",
      };
      setFeedback(
        messages[code] ??
          "Sign in is unavailable right now. Check that the local server is running.",
      );
    } finally {
      setPending(false);
    }
  }

  return (
    <section className="access-panel" aria-label="Community access">
      <div className="access-copy">
        <p className="eyebrow">Meerkateer Community</p>
        <h2>
          {mode === "login"
            ? "Sign in to your company"
            : mode === "setup"
              ? "Create your company"
              : "Recover owner access"}
        </h2>
        <p>
          {mode === "login"
            ? "Use the owner account on your self-hosted installation."
            : mode === "setup"
              ? "The setup key is only needed when creating this installation."
              : "The setup key lets you set a new password for the existing owner account."}
        </p>
      </div>
      <form className="access-form" onSubmit={(event) => void submit(event)}>
        {mode === "login" ? (
          <>
            <label>
              Owner email
              <input autoComplete="email" name="email" required type="email" />
            </label>
            <label>
              Password
              <input autoComplete="current-password" name="password" required type="password" />
            </label>
          </>
        ) : (
          <>
            <label>
              Setup key
              <input autoComplete="off" name="admin_token" required type="password" />
            </label>
            {mode === "setup" ? (
              <>
                <label>
                  Company name
                  <input name="tenant_name" required placeholder="Acme Games" />
                </label>
                <label>
                  Company slug
                  <input
                    name="tenant_slug"
                    pattern="[a-z0-9][a-z0-9_-]*[a-z0-9]"
                    required
                    placeholder="acme-games"
                  />
                </label>
                <label>
                  Owner name
                  <input name="owner_name" required placeholder="Operations Owner" />
                </label>
                <label>
                  Owner email
                  <input name="owner_email" required type="email" placeholder="owner@example.com" />
                </label>
              </>
            ) : null}
            <label>
              New owner password
              <input
                autoComplete="new-password"
                minLength={12}
                name="owner_password"
                required
                type="password"
              />
            </label>
            <label>
              Confirm password
              <input
                autoComplete="new-password"
                minLength={12}
                name="confirm_password"
                required
                type="password"
              />
            </label>
            <p className="password-hint">
              Use at least 12 characters. Your password stays on this installation.
            </p>
          </>
        )}
        <button disabled={pending} type="submit">
          {pending
            ? "Please wait…"
            : mode === "login"
              ? "Sign in"
              : mode === "setup"
                ? "Create company"
                : "Set password and sign in"}
        </button>
      </form>
      {feedback ? (
        <p className="access-feedback" role="alert">
          {feedback}
          {mode === "setup" && feedback.startsWith("A company") ? (
            <>
              <br />
              <a href="/recover">Set owner password →</a>
            </>
          ) : null}
        </p>
      ) : null}
    </section>
  );
}

function Operations({
  ready,
  onProject,
  onService,
  management,
}: {
  ready: DashboardReady;
  onProject: (id: string) => void;
  onService: (id: string) => void;
  management: ManagementActions;
}) {
  const selected = ready.services.find((item) => item.id === ready.serviceId) ?? null;
  const workspace = ready.projects.find((item) => item.id === ready.projectId) ?? null;
  const canManage = ready.session.role === "owner" || ready.session.role === "admin";
  return (
    <section className="operations" aria-label="Service operations dashboard">
      <div className="operations-head">
        <div>
          <p className="eyebrow">Active workspace</p>
          <h2>{workspace?.display_name ?? "No workspace"}</h2>
          <p className="tenant-id">
            {ready.session.email} · company {ready.session.tenant_id}
          </p>
        </div>
        <label>
          Workspace
          <select value={ready.projectId ?? ""} onChange={(event) => onProject(event.target.value)}>
            {ready.projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.display_name}
              </option>
            ))}
          </select>
        </label>
      </div>
      <MachineFleet
        agents={ready.agents}
        canManage={canManage}
        onUnassign={(agentId) =>
          ready.projectId
            ? management.unassignAgent(ready.projectId, agentId)
            : Promise.reject(new Error("No workspace selected"))
        }
      />
      {canManage ? (
        <WorkspaceManagement
          key={ready.projectId ?? "no-workspace"}
          ready={ready}
          actions={management}
        />
      ) : null}
      <div className="service-layout">
        <aside className="service-list" aria-label="Services">
          {ready.services.length === 0 ? (
            <p className="empty">No services in this project.</p>
          ) : (
            ready.services.map((service) => (
              <button
                className={
                  service.id === ready.serviceId ? "service-button selected" : "service-button"
                }
                key={service.id}
                onClick={() => onService(service.id)}
                type="button"
              >
                <span>
                  <strong>{service.slug}</strong>
                  <small>
                    {service.game?.kind === "minecraft_java" ? "Minecraft Java" : "Process"} ·{" "}
                    {service.environment}
                  </small>
                </span>
                <StatusPill state={service.status.state} />
              </button>
            ))
          )}
        </aside>
        <div className="incident-panel">
          {selected ? (
            <>
              <div className="incident-title">
                <div>
                  <p className="eyebrow">Selected service</p>
                  <h2>{selected.slug}</h2>
                </div>
                <StatusPill state={selected.status.state} />
              </div>
              <p className="freshness">
                {selected.status.stale
                  ? "Heartbeat is stale"
                  : selected.status.observed_at
                    ? `Last observed ${formatTime(selected.status.observed_at)}`
                    : "No heartbeat received"}
              </p>
              {selected.game?.kind === "minecraft_java" ? (
                <GameProbePanel key={selected.id} service={selected} />
              ) : null}
              <AvailabilityBoard key={selected.id} service={selected} timeline={ready.timeline} />
              <ol className="timeline">
                {ready.timeline.slice(0, 20).map((item) => (
                  <li
                    key={item.idempotency_key}
                    className={`timeline-${item.state ?? item.severity ?? item.kind}`}
                  >
                    <span className="timeline-dot" />
                    <div>
                      <div className="timeline-row">
                        <strong>{item.title}</strong>
                        <time dateTime={item.observed_at}>{formatTime(item.observed_at)}</time>
                      </div>
                      {item.message ? <p>{item.message}</p> : null}
                      <small>
                        {item.kind}
                        {item.severity ? ` · ${item.severity}` : ""}
                      </small>
                    </div>
                  </li>
                ))}
              </ol>
            </>
          ) : (
            <p className="empty">Select a service to inspect its timeline.</p>
          )}
        </div>
      </div>
    </section>
  );
}

function WorkspaceManagement({
  ready,
  actions,
}: {
  ready: DashboardReady;
  actions: ManagementActions;
}) {
  const [pending, setPending] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [issuedToken, setIssuedToken] = useState<IssuedEnrollmentTokenResponse | null>(null);
  const [issuedService, setIssuedService] = useState<{
    service: ServiceResponse;
    credential: IssuedCredentialResponse;
  } | null>(null);
  const [agentId, setAgentId] = useState("");
  const [serviceId, setServiceId] = useState("");
  const [monitorKind, setMonitorKind] = useState("process");
  const assignedIds = new Set(ready.agents.map((agent) => agent.id));
  const availableAgents = ready.companyAgents.filter((agent) => !assignedIds.has(agent.id));

  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    setPending("create");
    setFeedback(null);
    try {
      await actions.createWorkspace({
        slug: String(data.get("slug") ?? ""),
        display_name: String(data.get("display_name") ?? ""),
      });
      form.reset();
    } catch (error) {
      setFeedback(message(error));
    } finally {
      setPending(null);
    }
  }

  async function issueToken() {
    if (!ready.projectId) return;
    setPending("token");
    setFeedback(null);
    setIssuedToken(null);
    try {
      const token = await issueWorkspaceEnrollmentToken(ready.projectId);
      setIssuedToken(token);
      setFeedback("Enrollment token issued. It is shown only in this browser memory.");
    } catch (error) {
      setFeedback(message(error));
    } finally {
      setPending(null);
    }
  }

  async function assign(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!ready.projectId || !agentId) return;
    setPending("assign");
    setFeedback(null);
    try {
      await actions.assignAgent(ready.projectId, agentId);
      setAgentId("");
      setFeedback("Machine assigned to this workspace.");
    } catch (error) {
      setFeedback(message(error));
    } finally {
      setPending(null);
    }
  }

  async function createProcess(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!ready.projectId) return;
    const form = event.currentTarget;
    const data = new FormData(form);
    setPending("process");
    setFeedback(null);
    setIssuedService(null);
    try {
      const service = await actions.createService(ready.projectId, {
        slug: String(data.get("slug") ?? ""),
        environment: String(
          data.get("environment") ?? "production",
        ) as CreateServiceRequest["environment"],
        game:
          monitorKind === "minecraft_java"
            ? {
                kind: "minecraft_java",
                host: String(data.get("game_host") ?? "").trim(),
                port: Number(data.get("game_port") ?? 25565),
              }
            : undefined,
      });
      if (monitorKind === "minecraft_java") {
        setFeedback(
          "Minecraft instance saved. Select it and run a status test from this control plane.",
        );
      } else {
        const credential = await actions.issueServiceCredential(service.id);
        setIssuedService({ service, credential });
        setFeedback("Process created. Copy its SDK key now; it will not be shown again.");
      }
      form.reset();
      setMonitorKind("process");
    } catch (error) {
      setFeedback(message(error));
    } finally {
      setPending(null);
    }
  }

  async function issueProcessKey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const service = ready.services.find((item) => item.id === serviceId);
    if (!service) return;
    setPending("service-key");
    setFeedback(null);
    setIssuedService(null);
    try {
      const credential = await actions.issueServiceCredential(service.id);
      setIssuedService({ service, credential });
      setFeedback("New SDK key issued. Copy it now; it will not be shown again.");
      setServiceId("");
    } catch (error) {
      setFeedback(message(error));
    } finally {
      setPending(null);
    }
  }

  async function copyToken() {
    if (!issuedToken) return;
    try {
      await navigator.clipboard.writeText(issuedToken.secret);
      setFeedback("Enrollment token copied.");
    } catch {
      setFeedback("Copy was blocked by the browser; select the token manually.");
    }
  }

  async function copyServiceKey() {
    if (!issuedService) return;
    try {
      await navigator.clipboard.writeText(issuedService.credential.secret);
      setFeedback("SDK key copied.");
    } catch {
      setFeedback("Copy was blocked by the browser; select the SDK key manually.");
    }
  }

  return (
    <details className="workspace-management">
      <summary>Manage workspaces, machines, and processes</summary>
      <div className="management-grid">
        <form className="management-card" onSubmit={(event) => void create(event)}>
          <p className="eyebrow">New workspace</p>
          <label>
            Name
            <input name="display_name" maxLength={128} required placeholder="Bangkok game fleet" />
          </label>
          <label>
            Slug
            <input
              name="slug"
              maxLength={64}
              pattern="[a-z0-9][a-z0-9_-]*[a-z0-9]"
              required
              placeholder="bangkok-fleet"
            />
          </label>
          <button disabled={pending !== null} type="submit">
            {pending === "create" ? "Creating…" : "Create workspace"}
          </button>
        </form>

        <section className="management-card" aria-label="Workspace enrollment">
          <p className="eyebrow">Enroll a new machine</p>
          <p>Create a ten-minute, one-time token scoped to the selected workspace.</p>
          <button
            disabled={!ready.projectId || pending !== null}
            onClick={() => void issueToken()}
            type="button"
          >
            {pending === "token" ? "Issuing…" : "Issue enrollment token"}
          </button>
        </section>

        <form className="management-card" onSubmit={(event) => void assign(event)}>
          <p className="eyebrow">Assign existing machine</p>
          <label>
            Company machine
            <select value={agentId} onChange={(event) => setAgentId(event.target.value)}>
              <option value="">Select a machine</option>
              {availableAgents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.display_name} · {agent.connection_state.replace("_", " ")}
                </option>
              ))}
            </select>
          </label>
          <button disabled={!agentId || pending !== null} type="submit">
            {pending === "assign" ? "Assigning…" : "Assign machine"}
          </button>
        </form>

        <form className="management-card" onSubmit={(event) => void createProcess(event)}>
          <p className="eyebrow">Add a game instance or process</p>
          <label>
            Monitor type
            <select value={monitorKind} onChange={(event) => setMonitorKind(event.target.value)}>
              <option value="process">Application process with SDK</option>
              <option value="minecraft_java">Minecraft Java / Paper</option>
            </select>
          </label>
          <label>
            Process slug
            <input
              name="slug"
              maxLength={64}
              pattern="[a-z0-9][a-z0-9_-]*[a-z0-9]"
              required
              placeholder="game-server-01"
            />
          </label>
          <label>
            Environment
            <select defaultValue="production" name="environment">
              <option value="production">Production</option>
              <option value="staging">Staging</option>
              <option value="development">Development</option>
              <option value="test">Test</option>
              <option value="local">Local</option>
            </select>
          </label>
          {monitorKind === "minecraft_java" ? (
            <>
              <label>
                Public game address
                <input name="game_host" maxLength={253} required placeholder="play.example.com" />
              </label>
              <label>
                Java server port
                <input
                  name="game_port"
                  type="number"
                  min={1}
                  max={65535}
                  defaultValue={25565}
                  required
                />
              </label>
              <p>
                One instance per service. The first test queries Minecraft status from the Community
                control plane; scheduled checks and Discord alerts are coming later.
              </p>
            </>
          ) : null}
          <button disabled={!ready.projectId || pending !== null} type="submit">
            {pending === "process"
              ? "Creating…"
              : monitorKind === "minecraft_java"
                ? "Add Minecraft instance"
                : "Create process + SDK key"}
          </button>
        </form>

        <form className="management-card" onSubmit={(event) => void issueProcessKey(event)}>
          <p className="eyebrow">Issue another SDK key</p>
          <label>
            Existing process
            <select value={serviceId} onChange={(event) => setServiceId(event.target.value)}>
              <option value="">Select a process</option>
              {ready.services.map((service) => (
                <option key={service.id} value={service.id}>
                  {service.slug} · {service.environment}
                </option>
              ))}
            </select>
          </label>
          <button disabled={!serviceId || pending !== null} type="submit">
            {pending === "service-key" ? "Issuing…" : "Issue SDK key"}
          </button>
        </form>
      </div>

      {issuedToken ? (
        <section className="one-time-secret" aria-label="One-time enrollment token">
          <div>
            <p className="eyebrow">Copy now — shown once</p>
            <code>{issuedToken.secret}</code>
            <small>Expires {formatTime(issuedToken.expires_at)}</small>
            <p className="enrollment-command-note">
              Set <code>MEERKATEER_ENROLLMENT_TOKEN</code> on the machine, then run:
            </p>
            <code className="enrollment-command">
              {`meerkateer-agent enroll --server ${suggestedAgentServerUrl()} --name "game-host-01"`}
            </code>
          </div>
          <div className="secret-actions">
            <button onClick={() => void copyToken()} type="button">
              Copy token
            </button>
            <button className="button-secondary" onClick={() => setIssuedToken(null)} type="button">
              Dismiss
            </button>
          </div>
        </section>
      ) : null}
      {issuedService ? (
        <section className="one-time-secret" aria-label="One-time process SDK key">
          <div>
            <p className="eyebrow">Process SDK key — copy now</p>
            <code>{issuedService.credential.secret}</code>
            <small>
              {issuedService.service.slug} · {issuedService.service.environment} · shown once
            </small>
            <p className="enrollment-command-note">Set these on the monitored process:</p>
            <code className="enrollment-command">
              {`MEERKATEER_URL=${suggestedAgentServerUrl()}\nMEERKATEER_SERVICE_KEY=<paste-key>\nMEERKATEER_PROJECT=${ready.projects.find((item) => item.id === ready.projectId)?.slug ?? "workspace"}\nMEERKATEER_SERVICE=${issuedService.service.slug}\nMEERKATEER_ENVIRONMENT=${issuedService.service.environment}`}
            </code>
          </div>
          <div className="secret-actions">
            <button onClick={() => void copyServiceKey()} type="button">
              Copy SDK key
            </button>
            <button
              className="button-secondary"
              onClick={() => setIssuedService(null)}
              type="button"
            >
              Dismiss
            </button>
          </div>
        </section>
      ) : null}
      {feedback ? (
        <p className="management-feedback" role="status">
          {feedback}
        </p>
      ) : null}
    </details>
  );
}

function MachineFleet({
  agents,
  canManage,
  onUnassign,
}: {
  agents: AgentResponse[];
  canManage: boolean;
  onUnassign: (agentId: string) => Promise<void>;
}) {
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  async function unassign(agentId: string) {
    setPendingId(agentId);
    setFailure(null);
    try {
      await onUnassign(agentId);
    } catch (error) {
      setFailure(message(error));
    } finally {
      setPendingId(null);
    }
  }

  return (
    <section className="machine-fleet" aria-label="Workspace machines">
      <div className="machine-fleet-head">
        <div>
          <p className="eyebrow">Machines</p>
          <h3>{agents.length} connected to this workspace</h3>
        </div>
        <span className="fleet-count">
          {agents.filter((item) => item.connection_state === "online").length} online
        </span>
      </div>
      {agents.length === 0 ? (
        <p className="empty machine-empty">
          No machines assigned. Issue an enrollment token from this workspace to attach the first
          computer automatically.
        </p>
      ) : (
        <div className="machine-grid">
          {agents.map((agent) => (
            <article className="machine-card" key={agent.id}>
              <div className="machine-card-title">
                <span className={`machine-light state-${agent.connection_state}`} />
                <strong>{agent.display_name}</strong>
              </div>
              <StatusPill state={agent.connection_state} />
              <p>
                {agent.last_seen_at
                  ? `Last seen ${formatTime(agent.last_seen_at)}`
                  : "Waiting for first telemetry"}
              </p>
              <small>{agent.id}</small>
              {canManage ? (
                <button
                  className="machine-remove"
                  disabled={pendingId !== null}
                  onClick={() => void unassign(agent.id)}
                  type="button"
                >
                  {pendingId === agent.id ? "Removing…" : "Remove from workspace"}
                </button>
              ) : null}
            </article>
          ))}
        </div>
      )}
      {failure ? (
        <p className="management-feedback" role="alert">
          {failure}
        </p>
      ) : null}
    </section>
  );
}

function StatusPill({ state }: { state: string }) {
  return <span className={`status-pill state-${state}`}>{state.replace("_", " ")}</span>;
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(value),
  );
}

function suggestedAgentServerUrl(): string {
  const url = new URL(window.location.origin);
  const loopback =
    url.hostname === "localhost" || url.hostname === "127.0.0.1" || url.hostname === "[::1]";
  if (url.port === "6511" && loopback) {
    url.port = "6510";
  }
  if (url.protocol === "http:" && !loopback) {
    return "https://your-meerkateer.example.com";
  }
  return url.origin;
}
