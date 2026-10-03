import { type FormEvent, lazy, type ReactNode, Suspense, useEffect, useState } from "react";
import { AvailabilityBoard } from "./AvailabilityBoard";
import {
  type AdminSummaryResponse,
  type AgentResponse,
  type AgentTelemetrySnapshotResponse,
  type AlertDeliveryResponse,
  type AlertPolicyResponse,
  type AuditEventResponse,
  acknowledgeIncident,
  addIncidentNote,
  assignWorkspaceAgent,
  bootstrapCommunity,
  type CreateMaintenanceWindowRequest,
  type CreateProjectRequest,
  type CreateServiceRequest,
  cancelMaintenanceWindow,
  createMaintenanceWindow,
  createService,
  createWorkspace,
  fetchAdminSummary,
  fetchAgents,
  fetchAgentTelemetry,
  fetchAlertDeliveries,
  fetchAlertPolicy,
  fetchAuditEvents,
  fetchHealth,
  fetchIncidentActivity,
  fetchIncidents,
  fetchMaintenanceWindows,
  fetchProjects,
  fetchServices,
  fetchSession,
  fetchTimeline,
  fetchWorkspaceAgents,
  type HealthResponse,
  type IncidentActivityResponse,
  type IncidentResponse,
  type IssuedCredentialResponse,
  type IssuedEnrollmentTokenResponse,
  issueServiceCredential,
  issueWorkspaceEnrollmentToken,
  type MaintenanceWindowResponse,
  type ProjectResponse,
  passwordLogin,
  replayAlertDelivery,
  type ServiceResponse,
  type SessionResponse,
  setupOwnerPassword,
  type TimelineItemResponse,
  testAlertWebhook,
  type UpdateAlertPolicyRequest,
  unassignWorkspaceAgent,
  updateAlertPolicy,
  updateIncidentAssignment,
} from "./api";
import { GameProbePanel } from "./GameProbePanel";
import { CloudAccessPage, LandingPage, RoadmapPage } from "./LandingPage";

const ResourcePage = lazy(() =>
  import("./ResourcePage").then((module) => ({ default: module.ResourcePage })),
);

const staticResourcePaths = new Set([
  "/get-started",
  "/help",
  "/source",
  "/changelog",
  "/contributing",
  "/license",
  "/security",
  "/governance",
]);

function isResourceRoute(path: string) {
  return path === "/docs" || path.startsWith("/docs/") || staticResourcePaths.has(path);
}

type ConsoleView =
  | "overview"
  | "machines"
  | "services"
  | "incidents"
  | "alerts"
  | "maintenance"
  | "admin"
  | "integrations";
type ConsoleRoute = { view: ConsoleView; serviceId?: string; machineId?: string };

const consoleNavigation: Array<{
  view: ConsoleView;
  href: string;
  label: string;
  icon: string;
}> = [
  { view: "overview", href: "/app", label: "Overview", icon: "⌂" },
  { view: "machines", href: "/app/machines", label: "Machines", icon: "▣" },
  { view: "services", href: "/app/services", label: "Services", icon: "◆" },
  { view: "incidents", href: "/app/incidents", label: "Incidents", icon: "!" },
  { view: "alerts", href: "/app/alerts", label: "Alerts", icon: "◉" },
  { view: "maintenance", href: "/app/maintenance", label: "Maintenance", icon: "☾" },
  { view: "admin", href: "/app/admin", label: "Admin", icon: "⚙" },
  { view: "integrations", href: "/app/integrations", label: "Connect", icon: "+" },
];

function consoleRouteFromPath(path: string): ConsoleRoute | null {
  if (path === "/app") return { view: "overview" };
  const match = consoleNavigation.find((item) => item.href === path);
  if (match) return { view: match.view };
  const detail = path.match(/^\/app\/(machines|services|incidents)\/([^/]+)$/);
  if (detail) {
    try {
      const resourceId = decodeURIComponent(detail[2]);
      if (resourceId.length === 0 || resourceId.length > 128) return null;
      if (detail[1] === "machines") return { view: "machines", machineId: resourceId };
      return {
        view: detail[1] as "services" | "incidents",
        serviceId: resourceId,
      };
    } catch {
      return null;
    }
  }
  return null;
}

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
  incidents: IncidentResponse[];
  incidentActivity: IncidentActivityResponse[];
  alertDeliveries: AlertDeliveryResponse[];
  alertPolicy: AlertPolicyResponse;
  maintenanceWindows: MaintenanceWindowResponse[];
  auditEvents: AuditEventResponse[];
  adminSummary: AdminSummaryResponse | null;
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
  updateAlertPolicy: (input: UpdateAlertPolicyRequest) => Promise<void>;
  createMaintenance: (input: CreateMaintenanceWindowRequest) => Promise<void>;
  cancelMaintenance: (windowId: string) => Promise<void>;
  acknowledgeIncident: (incidentId: string) => Promise<void>;
  updateIncidentAssignment: (incidentId: string, assigned: boolean) => Promise<void>;
  addIncidentNote: (incidentId: string, note: string) => Promise<void>;
  replayAlertDelivery: (deliveryId: string) => Promise<void>;
};

function message(error: unknown): string {
  return error instanceof Error ? error.message : "Unable to reach the control plane";
}

export function App() {
  const path = window.location.pathname.replace(/\/$/, "") || "/";
  if (path === "/" || path === "/about") return <LandingPage />;
  if (path === "/roadmap") return <RoadmapPage />;
  if (path === "/cloud") return <CloudAccessPage />;
  if (path === "/login") {
    const requestedMode = new URLSearchParams(window.location.search).get("mode");
    const mode: AccessMode =
      requestedMode === "setup" || requestedMode === "recover" ? requestedMode : "login";
    return <CommunityAccessPage mode={mode} />;
  }
  if (path === "/setup") return <CommunityAccessPage mode="setup" />;
  if (path === "/recover") return <CommunityAccessPage mode="recover" />;
  if (isResourceRoute(path)) {
    return (
      <Suspense fallback={<main className="page-width resource-loading">Loading guide…</main>}>
        <ResourcePage path={path} />
      </Suspense>
    );
  }
  const consoleRoute = consoleRouteFromPath(path);
  if (consoleRoute) return <ConsoleApp route={consoleRoute} />;
  return <LandingPage />;
}

type AccessMode = "login" | "setup" | "recover";

const accessModeHelp = {
  login: "For normal daily access with your owner email and password.",
  setup: "Use once on a new installation to create its company and first owner.",
  recover: "Use only to set or replace the owner's password with the private setup key.",
} satisfies Record<AccessMode, string>;

function CommunityAccessPage({
  mode: initialMode,
  returnTo = "/app",
}: {
  mode: AccessMode;
  returnTo?: string;
}) {
  const [mode, setMode] = useState<AccessMode>(initialMode);
  const safeReturnTo = returnTo === "/app" || returnTo.startsWith("/app/") ? returnTo : "/app";

  function selectMode(nextMode: AccessMode) {
    setMode(nextMode);
    const query = nextMode === "login" ? "" : `?mode=${nextMode}`;
    window.history.replaceState({}, "", `/login${query}`);
  }

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
      <main className="setup-main setup-main-unified">
        <div className="access-column access-column-unified">
          <div className="access-welcome">
            <img src="/logo.png" alt="" />
            <div>
              <p className="eyebrow">Self-hosted Community</p>
              <h1>Open your company.</h1>
              <p>One place to sign in, complete first setup, or recover owner access.</p>
            </div>
          </div>
          <div className="access-mode-tabs" aria-label="Choose access method" role="tablist">
            {(["login", "setup", "recover"] as const).map((item) => (
              <button
                aria-selected={mode === item}
                className={mode === item ? "is-active" : ""}
                key={item}
                onClick={() => selectMode(item)}
                role="tab"
                type="button"
              >
                {item === "login" ? "Sign in" : item === "setup" ? "First setup" : "Recover"}
              </button>
            ))}
          </div>
          <p className="access-mode-help">{accessModeHelp[mode]}</p>
          <CommunityAccess
            key={mode}
            mode={mode}
            onAuthenticated={() => window.location.assign(safeReturnTo)}
          />
          <p className="access-guide-link">
            Need installation help? <a href="/get-started">Open the self-hosting guide →</a>
          </p>
          {mode === "login" && safeReturnTo !== "/app" ? (
            <p className="access-return-note">
              After sign in, you will return to the page you opened.
            </p>
          ) : null}
        </div>
      </main>
    </div>
  );
}

function ConsoleApp({ route }: { route: ConsoleRoute }) {
  const { view } = route;
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
    void loadDashboard(controller.signal, setDashboard, route.serviceId);
    return () => controller.abort();
  }, [route.serviceId]);

  function reloadDashboard() {
    const controller = new AbortController();
    setHealth({ phase: "loading" });
    setDashboard({ phase: "loading" });
    fetchHealth(controller.signal)
      .then((value) => setHealth({ phase: "ready", health: value }))
      .catch((error: unknown) => {
        if (!controller.signal.aborted) setHealth({ phase: "error", message: message(error) });
      });
    void loadDashboard(controller.signal, setDashboard, route.serviceId);
  }

  async function selectProject(projectId: string) {
    if (dashboard.phase !== "ready") return;
    const controller = new AbortController();
    try {
      const [services, agents, incidents, incidentActivity, alertDeliveries, maintenanceWindows] =
        await Promise.all([
          fetchServices(projectId, controller.signal),
          fetchWorkspaceAgents(projectId, controller.signal),
          fetchIncidents(projectId, controller.signal),
          fetchIncidentActivity(projectId, controller.signal),
          fetchAlertDeliveries(projectId, controller.signal),
          fetchMaintenanceWindows(projectId, controller.signal),
        ]);
      const serviceId = services[0]?.id ?? null;
      const timeline = serviceId ? await fetchTimeline(serviceId, controller.signal) : [];
      setDashboard({
        ...dashboard,
        projectId,
        agents,
        services,
        serviceId,
        timeline,
        incidents,
        incidentActivity,
        alertDeliveries,
        maintenanceWindows,
      });
      if (
        view === "machines" ||
        view === "services" ||
        view === "incidents" ||
        view === "alerts" ||
        view === "maintenance"
      ) {
        window.history.replaceState({}, "", `/app/${view}`);
      }
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
      incidents: [],
      incidentActivity: [],
      alertDeliveries: [],
      maintenanceWindows: [],
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

  async function updateAlertPolicyAction(input: UpdateAlertPolicyRequest) {
    if (dashboard.phase !== "ready") throw new Error("Dashboard is not ready");
    const alertPolicy = await updateAlertPolicy(input);
    setDashboard({ ...dashboard, alertPolicy });
  }

  async function createMaintenanceAction(input: CreateMaintenanceWindowRequest) {
    if (dashboard.phase !== "ready" || !dashboard.projectId) {
      throw new Error("Dashboard is not ready");
    }
    await createMaintenanceWindow(input);
    const controller = new AbortController();
    const maintenanceWindows = await fetchMaintenanceWindows(
      dashboard.projectId,
      controller.signal,
    );
    setDashboard({ ...dashboard, maintenanceWindows });
  }

  async function cancelMaintenanceAction(windowId: string) {
    if (dashboard.phase !== "ready" || !dashboard.projectId) {
      throw new Error("Dashboard is not ready");
    }
    await cancelMaintenanceWindow(windowId);
    const controller = new AbortController();
    const maintenanceWindows = await fetchMaintenanceWindows(
      dashboard.projectId,
      controller.signal,
    );
    setDashboard({ ...dashboard, maintenanceWindows });
  }

  async function refreshIncidentData() {
    if (dashboard.phase !== "ready" || !dashboard.projectId) {
      throw new Error("Dashboard is not ready");
    }
    const controller = new AbortController();
    const [incidents, incidentActivity] = await Promise.all([
      fetchIncidents(dashboard.projectId, controller.signal),
      fetchIncidentActivity(dashboard.projectId, controller.signal),
    ]);
    setDashboard({ ...dashboard, incidents, incidentActivity });
  }

  async function acknowledgeIncidentAction(incidentId: string) {
    await acknowledgeIncident(incidentId);
    await refreshIncidentData();
  }

  async function updateIncidentAssignmentAction(incidentId: string, assigned: boolean) {
    await updateIncidentAssignment(incidentId, assigned);
    await refreshIncidentData();
  }

  async function addIncidentNoteAction(incidentId: string, note: string) {
    await addIncidentNote(incidentId, note);
    await refreshIncidentData();
  }

  async function replayAlertDeliveryAction(deliveryId: string) {
    if (dashboard.phase !== "ready" || !dashboard.projectId) {
      throw new Error("Dashboard is not ready");
    }
    await replayAlertDelivery(deliveryId);
    const controller = new AbortController();
    const alertDeliveries = await fetchAlertDeliveries(dashboard.projectId, controller.signal);
    setDashboard({ ...dashboard, alertDeliveries });
  }

  if (dashboard.phase === "signed-out") {
    return <CommunityAccessPage mode="login" returnTo={window.location.pathname} />;
  }

  return (
    <div className="app-shell">
      <a className="skip-link" href="#console-content">
        Skip to content
      </a>
      <header className="topbar">
        <a className="brand" href="/app" aria-label="Meerkateer console home">
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

      <main className="console-main">
        <Dashboard
          state={dashboard}
          health={health}
          view={view}
          machineId={route.machineId}
          onAuthenticated={reloadDashboard}
          onProject={(id) => void selectProject(id)}
          management={{
            createWorkspace: createWorkspaceAction,
            createService: createServiceAction,
            issueServiceCredential,
            assignAgent: (projectId, agentId) => updateAgentAssignment(projectId, agentId, true),
            unassignAgent: (projectId, agentId) => updateAgentAssignment(projectId, agentId, false),
            updateAlertPolicy: updateAlertPolicyAction,
            createMaintenance: createMaintenanceAction,
            cancelMaintenance: cancelMaintenanceAction,
            acknowledgeIncident: acknowledgeIncidentAction,
            updateIncidentAssignment: updateIncidentAssignmentAction,
            addIncidentNote: addIncidentNoteAction,
            replayAlertDelivery: replayAlertDeliveryAction,
          }}
        />
      </main>
    </div>
  );
}

function AlertTestPanel() {
  const [state, setState] = useState<"idle" | "sending" | "sent" | "unconfigured" | "failed">(
    "idle",
  );
  async function sendTest() {
    setState("sending");
    try {
      await testAlertWebhook();
      setState("sent");
    } catch (error) {
      const detail = message(error);
      setState(detail.includes("alert_webhook_not_configured") ? "unconfigured" : "failed");
    }
  }
  return (
    <section className="alert-test-panel" aria-labelledby="alert-test-title">
      <div>
        <p className="eyebrow">Community alerts</p>
        <h2 id="alert-test-title">Test your alert channel</h2>
        <p>
          Set MEERKATEER_ALERT_WEBHOOK_URL in the private .env file and restart the API and worker.
        </p>
        {state === "sent" ? <p role="status">Test message delivered.</p> : null}
        {state === "unconfigured" ? <p role="status">Webhook is not configured yet.</p> : null}
        {state === "failed" ? (
          <p role="alert">Delivery failed. Check the receiver and try again.</p>
        ) : null}
      </div>
      <button
        className="button button-secondary"
        type="button"
        disabled={state === "sending"}
        onClick={() => void sendTest()}
      >
        {state === "sending" ? "Sending…" : "Send test alert"}
      </button>
    </section>
  );
}

function IncidentBoard({
  incidents,
  activity,
  canOperate,
  currentUserId,
  onAcknowledge,
  onAssignment,
  onNote,
}: {
  incidents: IncidentResponse[];
  activity: IncidentActivityResponse[];
  canOperate: boolean;
  currentUserId: string;
  onAcknowledge: (incidentId: string) => Promise<void>;
  onAssignment: (incidentId: string, assigned: boolean) => Promise<void>;
  onNote: (incidentId: string, note: string) => Promise<void>;
}) {
  if (incidents.length === 0) {
    return (
      <EmptyOperations title="No incidents yet" copy="Fresh offline evidence will open one here." />
    );
  }
  return (
    <div className="operations-list">
      {incidents.map((incident) => {
        const incidentActivity = activity.filter((item) => item.incident_id === incident.id);
        return (
          <IncidentCard
            key={incident.id}
            incident={incident}
            activity={incidentActivity}
            canOperate={canOperate}
            currentUserId={currentUserId}
            onAcknowledge={onAcknowledge}
            onAssignment={onAssignment}
            onNote={onNote}
          />
        );
      })}
    </div>
  );
}

function IncidentCard({
  incident,
  activity,
  canOperate,
  currentUserId,
  onAcknowledge,
  onAssignment,
  onNote,
}: {
  incident: IncidentResponse;
  activity: IncidentActivityResponse[];
  canOperate: boolean;
  currentUserId: string;
  onAcknowledge: (incidentId: string) => Promise<void>;
  onAssignment: (incidentId: string, assigned: boolean) => Promise<void>;
  onNote: (incidentId: string, note: string) => Promise<void>;
}) {
  const [note, setNote] = useState("");
  const [pending, setPending] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const assignedToMe = incident.assigned_to === currentUserId;

  async function run(label: string, action: () => Promise<void>) {
    setPending(label);
    setFeedback(null);
    try {
      await action();
      setFeedback(`${label} saved.`);
    } catch (error) {
      setFeedback(message(error));
    } finally {
      setPending(null);
    }
  }

  async function submitNote(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const value = note.trim();
    if (!value) return;
    await run("Note", () => onNote(incident.id, value));
    setNote("");
  }

  return (
    <article className="operations-row incident-workflow-card">
      <span className={`state-dot state-${incident.status}`} aria-hidden="true" />
      <div className="incident-workflow-main">
        <div className="incident-heading-row">
          <div>
            <strong>{incident.title}</strong>
            <p>
              {incident.service} · {incident.cause.replaceAll("_", " ")} · started{" "}
              {formatMoment(incident.started_at)}
            </p>
          </div>
          <div className="operations-row-meta">
            <span className={`status-pill status-${incident.status}`}>{incident.status}</span>
            <small>
              {incident.resolved_at
                ? `Recovered ${formatMoment(incident.resolved_at)}`
                : "Needs attention"}
            </small>
          </div>
        </div>

        <div className="incident-ownership">
          <span>
            {incident.acknowledged_at
              ? `Acknowledged by ${incident.acknowledged_by ?? "an operator"}`
              : "Not acknowledged"}
          </span>
          <span>{incident.assignee ? `Assigned to ${incident.assignee}` : "Unassigned"}</span>
        </div>

        {canOperate ? (
          <div className="incident-actions">
            {!incident.acknowledged_at ? (
              <button
                className="button button-secondary"
                type="button"
                disabled={pending !== null}
                onClick={() => void run("Acknowledgement", () => onAcknowledge(incident.id))}
              >
                Acknowledge
              </button>
            ) : null}
            <button
              className="button button-secondary"
              type="button"
              disabled={pending !== null}
              onClick={() =>
                void run(assignedToMe ? "Unassignment" : "Assignment", () =>
                  onAssignment(incident.id, !assignedToMe),
                )
              }
            >
              {assignedToMe ? "Unassign me" : "Assign to me"}
            </button>
          </div>
        ) : null}

        {canOperate ? (
          <form className="incident-note-form" onSubmit={(event) => void submitNote(event)}>
            <label htmlFor={`incident-note-${incident.id}`}>Add operator note</label>
            <div>
              <textarea
                id={`incident-note-${incident.id}`}
                value={note}
                maxLength={2000}
                rows={2}
                placeholder="What did you check or change?"
                onChange={(event) => setNote(event.target.value)}
              />
              <button
                className="button button-primary"
                type="submit"
                disabled={!note.trim() || pending !== null}
              >
                {pending === "Note" ? "Saving…" : "Add note"}
              </button>
            </div>
          </form>
        ) : null}

        {feedback ? (
          <p role="status" className="incident-feedback">
            {feedback}
          </p>
        ) : null}
        {activity.length > 0 ? (
          <div className="incident-activity">
            <strong>Operator activity</strong>
            <ul>
              {activity.slice(0, 5).map((item) => (
                <li key={item.id}>
                  <span>{item.note ?? item.kind.replaceAll("_", " ")}</span>
                  <small>
                    {item.actor} · {formatMoment(item.created_at)}
                  </small>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </div>
    </article>
  );
}

function AlertPolicyPanel({
  policy,
  canManage,
  onSave,
}: {
  policy: AlertPolicyResponse;
  canManage: boolean;
  onSave: (input: UpdateAlertPolicyRequest) => Promise<void>;
}) {
  const [enabled, setEnabled] = useState(policy.enabled);
  const [notifyDown, setNotifyDown] = useState(policy.notify_down);
  const [notifyRecovered, setNotifyRecovered] = useState(policy.notify_recovered);
  const [cooldownSeconds, setCooldownSeconds] = useState(String(policy.cooldown_seconds));
  const [feedback, setFeedback] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  async function save() {
    setPending(true);
    setFeedback(null);
    try {
      await onSave({
        enabled,
        notify_down: notifyDown,
        notify_recovered: notifyRecovered,
        cooldown_seconds: Number(cooldownSeconds),
      });
      setFeedback("Policy saved.");
    } catch (error) {
      setFeedback(message(error));
    } finally {
      setPending(false);
    }
  }
  return (
    <section className="operations-card" aria-labelledby="alert-policy-title">
      <div>
        <p className="eyebrow">Policy</p>
        <h2 id="alert-policy-title">Transition notifications</h2>
        <p>Policy changes are audited. The receiver URL stays in the installation environment.</p>
      </div>
      <label className="cooldown-control" htmlFor="alert-cooldown-seconds">
        <span>Repeat-down cooldown</span>
        <span>
          <input
            id="alert-cooldown-seconds"
            type="number"
            min="0"
            max="86400"
            step="60"
            value={cooldownSeconds}
            disabled={!canManage}
            onChange={(event) => setCooldownSeconds(event.target.value)}
          />{" "}
          seconds
        </span>
        <small>Use 0 to alert on every new outage. Cooldown never hides incident evidence.</small>
      </label>
      <div className="choice-grid">
        <label>
          <input
            type="checkbox"
            checked={enabled}
            disabled={!canManage}
            onChange={(event) => setEnabled(event.target.checked)}
          />{" "}
          Alerts enabled
        </label>
        <label>
          <input
            type="checkbox"
            checked={notifyDown}
            disabled={!canManage}
            onChange={(event) => setNotifyDown(event.target.checked)}
          />{" "}
          Notify when down
        </label>
        <label>
          <input
            type="checkbox"
            checked={notifyRecovered}
            disabled={!canManage}
            onChange={(event) => setNotifyRecovered(event.target.checked)}
          />{" "}
          Notify when recovered
        </label>
      </div>
      {canManage ? (
        <button
          className="button button-secondary"
          type="button"
          disabled={pending}
          onClick={() => void save()}
        >
          {pending ? "Saving…" : "Save policy"}
        </button>
      ) : (
        <PermissionNotice />
      )}
      {feedback ? <p role="status">{feedback}</p> : null}
    </section>
  );
}

function AlertDeliveryBoard({
  deliveries,
  canOperate,
  onReplay,
}: {
  deliveries: AlertDeliveryResponse[];
  canOperate: boolean;
  onReplay: (deliveryId: string) => Promise<void>;
}) {
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);

  async function replay(deliveryId: string) {
    setPendingId(deliveryId);
    setFeedback(null);
    try {
      await onReplay(deliveryId);
      setFeedback("Replay queued as a new delivery.");
    } catch (error) {
      setFeedback(message(error));
    } finally {
      setPendingId(null);
    }
  }

  return (
    <section className="operations-card" aria-labelledby="delivery-title">
      <div>
        <p className="eyebrow">History</p>
        <h2 id="delivery-title">Delivery outcomes</h2>
      </div>
      {deliveries.length === 0 ? (
        <p>No down or recovery transition has been recorded in this workspace yet.</p>
      ) : (
        <div className="operations-list">
          {deliveries.map((delivery) => (
            <article className="operations-row" key={delivery.id}>
              <span className={`state-dot state-${delivery.status}`} aria-hidden="true" />
              <div>
                <strong>
                  {delivery.service} {delivery.transition}
                </strong>
                <p>
                  {delivery.suppression_reason ??
                    delivery.last_error ??
                    `Observed ${formatMoment(delivery.observed_at)}`}
                </p>
                {delivery.replay_of ? (
                  <small>Replay of {delivery.replay_of.slice(0, 8)}</small>
                ) : null}
              </div>
              <div className="operations-row-meta">
                <span className={`status-pill status-${delivery.status}`}>
                  {delivery.status.replaceAll("_", " ")}
                </span>
                <small>
                  {delivery.attempts} attempt{delivery.attempts === 1 ? "" : "s"}
                </small>
                {canOperate && delivery.status === "dead_lettered" ? (
                  <button
                    className="text-button replay-button"
                    type="button"
                    disabled={pendingId !== null}
                    onClick={() => void replay(delivery.id)}
                  >
                    {pendingId === delivery.id ? "Queueing…" : "Replay"}
                  </button>
                ) : null}
              </div>
            </article>
          ))}
        </div>
      )}
      {feedback ? <p role="status">{feedback}</p> : null}
    </section>
  );
}

function MaintenanceBoard({
  projectId,
  services,
  windows,
  canManage,
  onCreate,
  onCancel,
}: {
  projectId: string | null;
  services: ServiceResponse[];
  windows: MaintenanceWindowResponse[];
  canManage: boolean;
  onCreate: (input: CreateMaintenanceWindowRequest) => Promise<void>;
  onCancel: (windowId: string) => Promise<void>;
}) {
  const [pending, setPending] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!projectId) return;
    const formElement = event.currentTarget;
    const form = new FormData(formElement);
    setPending(true);
    setFeedback(null);
    try {
      await onCreate({
        project_id: projectId,
        service_id: String(form.get("service_id")),
        title: String(form.get("title")),
        reason: String(form.get("reason")),
        starts_at: new Date(String(form.get("starts_at"))).toISOString(),
        ends_at: new Date(String(form.get("ends_at"))).toISOString(),
      });
      formElement.reset();
      setFeedback("Maintenance window scheduled.");
    } catch (error) {
      setFeedback(message(error));
    } finally {
      setPending(false);
    }
  }
  return (
    <>
      {canManage ? (
        <form className="operations-card operations-form" onSubmit={(event) => void submit(event)}>
          <div>
            <p className="eyebrow">Schedule</p>
            <h2>Plan quiet time</h2>
          </div>
          <label>
            Service
            <select name="service_id" required defaultValue="">
              <option value="" disabled>
                Select a service
              </option>
              {services.map((service) => (
                <option key={service.id} value={service.id}>
                  {service.slug}
                </option>
              ))}
            </select>
          </label>
          <label>
            Title
            <input name="title" maxLength={128} required placeholder="Database upgrade" />
          </label>
          <label>
            Reason
            <textarea
              name="reason"
              maxLength={1024}
              required
              placeholder="Why alerts should be quiet"
            />
          </label>
          <div className="time-grid">
            <label>
              Starts
              <input name="starts_at" type="datetime-local" required />
            </label>
            <label>
              Ends
              <input name="ends_at" type="datetime-local" required />
            </label>
          </div>
          <button
            className="button button-primary"
            disabled={pending || services.length === 0}
            type="submit"
          >
            {pending ? "Scheduling…" : "Schedule window"}
          </button>
          {feedback ? <p role="status">{feedback}</p> : null}
        </form>
      ) : (
        <PermissionNotice />
      )}
      <div className="operations-list">
        {windows.length === 0 ? (
          <EmptyOperations
            title="No maintenance planned"
            copy="Scheduled windows will appear here without hiding health evidence."
          />
        ) : (
          windows.map((window) => {
            const active =
              !window.cancelled_at &&
              new Date(window.starts_at) <= new Date() &&
              new Date(window.ends_at) > new Date();
            const upcoming = !window.cancelled_at && new Date(window.starts_at) > new Date();
            const status = window.cancelled_at
              ? "cancelled"
              : active
                ? "active"
                : upcoming
                  ? "scheduled"
                  : "completed";
            return (
              <article className="operations-row" key={window.id}>
                <span className={`state-dot state-${status}`} aria-hidden="true" />
                <div>
                  <strong>{window.title}</strong>
                  <p>
                    {window.service ||
                      services.find((service) => service.id === window.service_id)?.slug}{" "}
                    · {window.reason}
                  </p>
                  <small>
                    {formatMoment(window.starts_at)} → {formatMoment(window.ends_at)}
                  </small>
                </div>
                <div className="operations-row-meta">
                  <span className={`status-pill status-${status}`}>{status}</span>
                  {canManage && !window.cancelled_at && (active || upcoming) ? (
                    <button
                      className="text-button"
                      type="button"
                      onClick={() => void onCancel(window.id)}
                    >
                      Cancel
                    </button>
                  ) : null}
                </div>
              </article>
            );
          })
        )}
      </div>
    </>
  );
}

function AdminBoard({
  summary,
  events,
}: {
  summary: AdminSummaryResponse;
  events: AuditEventResponse[];
}) {
  const workerCopy =
    summary.worker_status === "healthy"
      ? "Background jobs are moving normally."
      : summary.worker_status === "stalled"
        ? "No worker cycle has completed for over 30 seconds. Alerts and background processing may be delayed."
        : "No worker cycle has been recorded yet. Start the worker before relying on alerts.";
  const workerState =
    summary.worker_status === "healthy"
      ? "online"
      : summary.worker_status === "stalled"
        ? "offline"
        : "unknown";
  const counts = [
    ["Workspaces", summary.projects],
    ["Services", summary.services],
    ["Machines", summary.agents],
    ["Open incidents", summary.open_incidents],
    ["Pending alerts", summary.pending_alerts],
    ["Dead letters", summary.dead_lettered_alerts],
    ["Maintenance now", summary.active_maintenance_windows],
  ];
  return (
    <>
      <section
        className={`operations-card worker-runtime worker-runtime-${summary.worker_status}`}
        aria-live="polite"
      >
        <div className="worker-runtime-heading">
          <span className={`state-dot state-${workerState}`} aria-hidden="true" />
          <div>
            <p className="eyebrow">Background worker</p>
            <h2>
              {summary.worker_status === "healthy"
                ? "Worker is healthy"
                : summary.worker_status === "stalled"
                  ? "Worker is stalled"
                  : "Worker has not checked in"}
            </h2>
            <p>{workerCopy}</p>
          </div>
        </div>
        <dl className="worker-runtime-facts">
          <div>
            <dt>Last cycle</dt>
            <dd>
              {summary.worker_last_cycle_at
                ? formatMoment(summary.worker_last_cycle_at)
                : "Not recorded"}
            </dd>
          </div>
          <div>
            <dt>Last batch</dt>
            <dd>
              {summary.worker_last_cycle_claimed} claimed · {summary.worker_last_cycle_completed}{" "}
              completed · {summary.worker_last_cycle_retried} retried ·{" "}
              {summary.worker_last_cycle_dead_lettered} dead-lettered
            </dd>
          </div>
          <div>
            <dt>Oldest pending alert</dt>
            <dd>
              {summary.oldest_pending_alert_at
                ? formatMoment(summary.oldest_pending_alert_at)
                : "None"}
            </dd>
          </div>
        </dl>
      </section>
      <div className="admin-counts">
        {counts.map(([label, value]) => (
          <article className="status-card" key={label}>
            <p className="card-label">{label}</p>
            <h2>{value}</h2>
          </article>
        ))}
      </div>
      <section className="operations-card">
        <div>
          <p className="eyebrow">Append-only</p>
          <h2>Audit history</h2>
          <p>
            {summary.deployment_mode} installation · tenant {summary.tenant_id}
          </p>
        </div>
        {events.length === 0 ? (
          <p>No administrative changes have been recorded yet.</p>
        ) : (
          <div className="operations-list">
            {events.map((event) => (
              <article className="operations-row" key={event.id}>
                <span className="state-dot state-delivered" aria-hidden="true" />
                <div>
                  <strong>{event.action.replaceAll(".", " ")}</strong>
                  <p>
                    {event.actor_type} changed {event.target_type}
                  </p>
                </div>
                <div className="operations-row-meta">
                  <small>{formatMoment(event.occurred_at)}</small>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>
    </>
  );
}

function EmptyOperations({ title, copy }: { title: string; copy: string }) {
  return (
    <div className="operations-empty">
      <img src="/logo.png" alt="" />
      <div>
        <strong>{title}</strong>
        <p>{copy}</p>
      </div>
    </div>
  );
}

function formatMoment(value: string) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(value),
  );
}

async function loadDashboard(
  signal: AbortSignal,
  setState: (state: DashboardState) => void,
  requestedServiceId?: string,
) {
  try {
    const session = await fetchSession(signal);
    if (!session) return setState({ phase: "signed-out" });
    const [projects, companyAgents, alertPolicy] = await Promise.all([
      fetchProjects(signal),
      fetchAgents(signal),
      fetchAlertPolicy(signal),
    ]);
    const projectId = projects[0]?.id ?? null;
    const [services, agents, incidents, incidentActivity, alertDeliveries, maintenanceWindows] =
      projectId
        ? await Promise.all([
            fetchServices(projectId, signal),
            fetchWorkspaceAgents(projectId, signal),
            fetchIncidents(projectId, signal),
            fetchIncidentActivity(projectId, signal),
            fetchAlertDeliveries(projectId, signal),
            fetchMaintenanceWindows(projectId, signal),
          ])
        : [[], [], [], [], [], []];
    const serviceId = requestedServiceId
      ? services.some((service) => service.id === requestedServiceId)
        ? requestedServiceId
        : null
      : (services[0]?.id ?? null);
    const timeline = serviceId ? await fetchTimeline(serviceId, signal) : [];
    const canAdmin = session.role === "owner" || session.role === "admin";
    const [auditEvents, adminSummary] = canAdmin
      ? await Promise.all([fetchAuditEvents(signal), fetchAdminSummary(signal)])
      : [[], null];
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
      incidents,
      incidentActivity,
      alertDeliveries,
      alertPolicy,
      maintenanceWindows,
      auditEvents,
      adminSummary,
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
  health,
  view,
  machineId,
  onAuthenticated,
  onProject,
  management,
}: {
  state: DashboardState;
  health: HealthState;
  view: ConsoleView;
  machineId?: string;
  onAuthenticated: () => void;
  onProject: (id: string) => void;
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
          <h2>{health.phase === "error" ? "API unavailable" : "Dashboard unavailable"}</h2>
        </div>
        <div className="console-error-actions">
          <p>{health.phase === "error" ? health.message : state.message}</p>
          <button className="button button-secondary" type="button" onClick={onAuthenticated}>
            Retry connection
          </button>
        </div>
      </section>
    );
  return (
    <Operations
      ready={state}
      health={health}
      view={view}
      machineId={machineId}
      onRefresh={onAuthenticated}
      onProject={onProject}
      management={management}
    />
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
              <a href="/login?mode=recover">Set owner password →</a>
            </>
          ) : null}
        </p>
      ) : null}
    </section>
  );
}

function Operations({
  ready,
  health,
  view,
  machineId,
  onRefresh,
  onProject,
  management,
}: {
  ready: DashboardReady;
  health: HealthState;
  view: ConsoleView;
  machineId?: string;
  onRefresh: () => void;
  onProject: (id: string) => void;
  management: ManagementActions;
}) {
  const selected = ready.services.find((item) => item.id === ready.serviceId) ?? null;
  const workspace = ready.projects.find((item) => item.id === ready.projectId) ?? null;
  const canManage = ready.session.role === "owner" || ready.session.role === "admin";
  return (
    <div className="console-layout">
      <aside className="console-sidebar">
        <ConsoleNavigation view={view} />
        <div className="console-sidebar-context">
          <p className="eyebrow">Signed in as</p>
          <strong>{ready.session.display_name}</strong>
          <small>{ready.session.email}</small>
          <span>{ready.session.role}</span>
        </div>
      </aside>

      <section className="console-content" id="console-content" aria-label="Workspace console">
        <div className="operations-head">
          <div>
            <p className="eyebrow">Active workspace</p>
            <h2>{workspace?.display_name ?? "No workspace yet"}</h2>
          </div>
          <div className="operations-controls">
            <label>
              Workspace
              <select
                aria-label="Active workspace"
                value={ready.projectId ?? ""}
                disabled={ready.projects.length === 0}
                onChange={(event) => onProject(event.target.value)}
              >
                {ready.projects.length === 0 ? <option value="">No workspaces</option> : null}
                {ready.projects.map((project) => (
                  <option key={project.id} value={project.id}>
                    {project.display_name}
                  </option>
                ))}
              </select>
            </label>
            <button className="console-refresh" type="button" onClick={onRefresh}>
              ↻ Refresh
            </button>
          </div>
        </div>

        {view === "overview" ? (
          <OverviewPage ready={ready} health={health} selected={selected} />
        ) : null}
        {view === "machines" ? (
          <ConsolePage
            title="Machines"
            eyebrow="Fleet"
            copy="Every computer assigned to this workspace, with its latest connection state."
          >
            {machineId ? (
              <MachineDetail agent={ready.agents.find((agent) => agent.id === machineId) ?? null} />
            ) : null}
            <MachineFleet
              agents={ready.agents}
              selectedAgentId={machineId}
              canManage={canManage}
              onUnassign={(agentId) =>
                ready.projectId
                  ? management.unassignAgent(ready.projectId, agentId)
                  : Promise.reject(new Error("No workspace selected"))
              }
            />
            {canManage ? (
              <a className="console-inline-link" href="/app/integrations">
                Enroll or assign a machine →
              </a>
            ) : null}
          </ConsolePage>
        ) : null}
        {view === "services" ? (
          <ConsolePage
            title="Services"
            eyebrow="What you watch"
            copy="Processes and game instances monitored inside this workspace."
          >
            <div className="service-layout">
              <ServiceList ready={ready} view="services" />
              <ServiceDetail
                service={selected}
                timeline={ready.timeline}
                mode="service"
                empty={
                  ready.services.length > 0
                    ? "Service not found in this workspace."
                    : "No services are connected to this workspace yet."
                }
              />
            </div>
          </ConsolePage>
        ) : null}
        {view === "incidents" ? (
          <ConsolePage
            title="Incidents"
            eyebrow="Incidents"
            copy="Durable downtime records open from fresh failure evidence and resolve only after a newer recovery."
          >
            <IncidentBoard
              incidents={ready.incidents}
              activity={ready.incidentActivity}
              canOperate={ready.session.role !== "viewer"}
              currentUserId={ready.session.user_id}
              onAcknowledge={management.acknowledgeIncident}
              onAssignment={management.updateIncidentAssignment}
              onNote={management.addIncidentNote}
            />
          </ConsolePage>
        ) : null}
        {view === "alerts" ? (
          <ConsolePage
            title="Alerts"
            eyebrow="Notifications"
            copy="Verify the notification path that tells your team when a service goes down or recovers."
          >
            <section className="alert-capability" aria-labelledby="alert-capability-title">
              <div>
                <p className="eyebrow">Community capability</p>
                <h2 id="alert-capability-title">One installation webhook</h2>
                <p>
                  Every down and recovery transition now has a durable result: delivered, retrying,
                  dead-lettered, suppressed, disabled, or unconfigured.
                </p>
              </div>
              <span className="preview-badge">
                {ready.alertPolicy.webhook_configured ? "Webhook ready" : "Webhook not configured"}
              </span>
            </section>
            <AlertPolicyPanel
              policy={ready.alertPolicy}
              canManage={canManage}
              onSave={management.updateAlertPolicy}
            />
            <AlertDeliveryBoard
              deliveries={ready.alertDeliveries}
              canOperate={ready.session.role !== "viewer"}
              onReplay={management.replayAlertDelivery}
            />
            {ready.session.role === "owner" ? <AlertTestPanel /> : null}
          </ConsolePage>
        ) : null}
        {view === "maintenance" ? (
          <ConsolePage
            title="Maintenance windows"
            eyebrow="Planned work"
            copy="Suppress notifications during planned work while keeping incidents and health evidence visible."
          >
            <MaintenanceBoard
              projectId={ready.projectId}
              services={ready.services}
              windows={ready.maintenanceWindows}
              canManage={ready.session.role !== "viewer"}
              onCreate={management.createMaintenance}
              onCancel={management.cancelMaintenance}
            />
          </ConsolePage>
        ) : null}
        {view === "admin" ? (
          <ConsolePage
            title="Administration"
            eyebrow="Installation"
            copy="Tenant-scoped operational totals and the append-only administrative audit trail."
          >
            {ready.adminSummary ? (
              <AdminBoard summary={ready.adminSummary} events={ready.auditEvents} />
            ) : (
              <PermissionNotice />
            )}
          </ConsolePage>
        ) : null}
        {view === "integrations" ? (
          <ConsolePage
            title="Connect your systems"
            eyebrow="Connect"
            copy="Add workspaces, enroll machines, create monitored services, and test your alert receiver."
          >
            {canManage ? (
              <ConnectionGuide
                key={ready.projectId ?? "no-workspace"}
                ready={ready}
                actions={management}
              />
            ) : (
              <PermissionNotice />
            )}
          </ConsolePage>
        ) : null}
      </section>
      <nav className="console-mobile-nav" aria-label="Mobile console navigation">
        {consoleNavigation.map((item) => (
          <a
            key={item.view}
            href={item.href}
            aria-current={view === item.view ? "page" : undefined}
          >
            <span aria-hidden="true">{item.icon}</span>
            {item.label}
          </a>
        ))}
      </nav>
    </div>
  );
}

function ConsoleNavigation({ view }: { view: ConsoleView }) {
  return (
    <nav aria-label="Console navigation">
      <p className="console-nav-label">Workspace</p>
      {consoleNavigation.map((item) => (
        <a key={item.view} href={item.href} aria-current={view === item.view ? "page" : undefined}>
          <span aria-hidden="true">{item.icon}</span>
          {item.label}
        </a>
      ))}
    </nav>
  );
}

function ConsolePage({
  title,
  eyebrow,
  copy,
  children,
}: {
  title: string;
  eyebrow: string;
  copy: string;
  children: ReactNode;
}) {
  return (
    <div className="console-page">
      <header className="console-page-header">
        <p className="eyebrow">{eyebrow}</p>
        <h1>{title}</h1>
        <p>{copy}</p>
      </header>
      {children}
    </div>
  );
}

function OverviewPage({
  ready,
  health,
  selected,
}: {
  ready: DashboardReady;
  health: HealthState;
  selected: ServiceResponse | null;
}) {
  const serviceIssues = ready.services.filter((item) => item.status.state !== "online");
  const machineIssues = ready.agents.filter((item) => item.connection_state !== "online");
  const issueCount = serviceIssues.length + machineIssues.length;
  return (
    <ConsolePage
      title="Everything at a glance"
      eyebrow="Overview"
      copy="A calm summary of this workspace. Open a focused page when something needs attention."
    >
      <section className="console-welcome" aria-label="Workspace greeting">
        <div>
          <strong>
            {issueCount === 0
              ? "All quiet on watch."
              : `${issueCount} item${issueCount === 1 ? "" : "s"} need a look.`}
          </strong>
          <p>
            {issueCount === 0
              ? "Meerkateer has not found an unhealthy machine or service in this workspace."
              : "The details below come from the latest machine and service telemetry."}
          </p>
        </div>
        <img src="/logo.png" alt="" aria-hidden="true" />
      </section>
      <section className="status-grid" aria-label="Workspace status">
        <article className="status-card status-card-primary">
          <div>
            <p className="card-label">Control plane</p>
            <HealthContent state={health} />
          </div>
          <span className={`pulse ${health.phase === "ready" ? "pulse-live" : ""}`} />
        </article>
        <SummaryCards dashboard={ready} />
      </section>
      <section className="overview-grid">
        <article className="overview-card">
          <div className="overview-card-head">
            <div>
              <p className="eyebrow">Needs attention</p>
              <h2>{issueCount === 0 ? "Nothing right now" : `${issueCount} active`}</h2>
            </div>
            <a href="/app/incidents">View evidence →</a>
          </div>
          {issueCount === 0 ? (
            <p className="empty">Machines are fresh and services report online.</p>
          ) : (
            <ul className="attention-list">
              {serviceIssues.map((service) => (
                <li key={service.id}>
                  <span>
                    <strong>{service.slug}</strong>
                    <small>Service · {service.environment}</small>
                  </span>
                  <StatusPill state={service.status.state} />
                </li>
              ))}
              {machineIssues.map((agent) => (
                <li key={agent.id}>
                  <span>
                    <strong>{agent.display_name}</strong>
                    <small>Machine</small>
                  </span>
                  <StatusPill state={agent.connection_state} />
                </li>
              ))}
            </ul>
          )}
        </article>
        <article className="overview-card">
          <div className="overview-card-head">
            <div>
              <p className="eyebrow">Recent evidence</p>
              <h2>{selected?.slug ?? "No service selected"}</h2>
            </div>
            <a href="/app/services">Services →</a>
          </div>
          <Timeline
            items={ready.timeline.slice(0, 4)}
            empty="No evidence received for this service yet."
          />
        </article>
      </section>
    </ConsolePage>
  );
}

function ServiceList({ ready, view }: { ready: DashboardReady; view: "services" | "incidents" }) {
  const [query, setQuery] = useState("");
  const [stateFilter, setStateFilter] = useState("all");
  const [environmentFilter, setEnvironmentFilter] = useState("all");
  const environments = [...new Set(ready.services.map((service) => service.environment))].sort();
  const visibleServices = ready.services.filter((service) => {
    const matchesQuery = service.slug.toLowerCase().includes(query.trim().toLowerCase());
    const matchesState = stateFilter === "all" || service.status.state === stateFilter;
    const matchesEnvironment =
      environmentFilter === "all" || service.environment === environmentFilter;
    return matchesQuery && matchesState && matchesEnvironment;
  });
  return (
    <aside className="service-list" aria-label="Services">
      {ready.services.length === 0 ? (
        <div className="empty-state">
          <strong>No services yet</strong>
          <p>Connect a process or game instance to begin monitoring.</p>
          <a href="/app/integrations">Connect a service →</a>
        </div>
      ) : (
        <>
          {/* biome-ignore lint/a11y/useSemanticElements: jsdom does not implement the HTML search element yet. */}
          <div className="list-toolbar service-toolbar" role="search" aria-label="Filter services">
            <label>
              <span>Search</span>
              <input
                aria-label="Search services"
                type="search"
                value={query}
                placeholder="Service name"
                onChange={(event) => setQuery(event.target.value)}
              />
            </label>
            <label>
              <span>State</span>
              <select
                aria-label="Filter services by state"
                value={stateFilter}
                onChange={(event) => setStateFilter(event.target.value)}
              >
                <option value="all">All states</option>
                <option value="online">Online</option>
                <option value="degraded">Degraded</option>
                <option value="offline">Offline</option>
                <option value="unknown">Unknown</option>
              </select>
            </label>
            <label>
              <span>Environment</span>
              <select
                aria-label="Filter services by environment"
                value={environmentFilter}
                onChange={(event) => setEnvironmentFilter(event.target.value)}
              >
                <option value="all">All environments</option>
                {environments.map((environment) => (
                  <option key={environment} value={environment}>
                    {environment}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="service-results" aria-live="polite">
            {visibleServices.length === 0 ? (
              <p className="empty filtered-empty">No services match these filters.</p>
            ) : (
              visibleServices.map((service) => (
                <a
                  className={
                    service.id === ready.serviceId ? "service-button selected" : "service-button"
                  }
                  aria-current={service.id === ready.serviceId ? "page" : undefined}
                  href={`/app/${view}/${encodeURIComponent(service.id)}`}
                  key={service.id}
                >
                  <span>
                    <strong>{service.slug}</strong>
                    <small>
                      {service.game?.kind === "minecraft_java" ? "Minecraft Java" : "Process"} ·{" "}
                      {service.environment}
                    </small>
                  </span>
                  <StatusPill state={service.status.state} />
                </a>
              ))
            )}
          </div>
        </>
      )}
    </aside>
  );
}

function ServiceDetail({
  service,
  timeline,
  mode,
  empty,
}: {
  service: ServiceResponse | null;
  timeline: TimelineItemResponse[];
  mode: "service" | "incident";
  empty: string;
}) {
  if (!service) {
    return (
      <div className="incident-panel">
        <p className="empty">{empty}</p>
      </div>
    );
  }
  return (
    <div className="incident-panel">
      <div className="incident-title">
        <div>
          <p className="eyebrow">
            {mode === "incident" ? "Selected timeline" : "Selected service"}
          </p>
          <h2>{service.slug}</h2>
        </div>
        <StatusPill state={service.status.state} />
      </div>
      <p className="freshness">
        {service.status.stale
          ? "Heartbeat is stale"
          : service.status.observed_at
            ? `Last observed ${formatTime(service.status.observed_at)}`
            : "No heartbeat received"}
      </p>
      {service.game?.kind === "minecraft_java" ? (
        <GameProbePanel key={service.id} service={service} />
      ) : null}
      {mode === "incident" ? (
        <>
          <AvailabilityBoard key={service.id} service={service} timeline={timeline} />
          <Timeline
            items={timeline.slice(0, 20)}
            empty="No state changes have been recorded yet."
          />
        </>
      ) : (
        <div className="service-facts">
          <div>
            <small>Monitor type</small>
            <strong>
              {service.game?.kind === "minecraft_java" ? "Minecraft Java" : "SDK process"}
            </strong>
          </div>
          <div>
            <small>Environment</small>
            <strong>{service.environment}</strong>
          </div>
          <div>
            <small>Reported state</small>
            <strong>{service.status.reported_state ?? "Unknown"}</strong>
          </div>
          <a href="/app/incidents">Open evidence timeline →</a>
        </div>
      )}
    </div>
  );
}

function Timeline({ items, empty }: { items: TimelineItemResponse[]; empty: string }) {
  if (items.length === 0) return <p className="empty timeline-empty">{empty}</p>;
  return (
    <ol className="timeline">
      {items.map((item) => (
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
  );
}

function PermissionNotice() {
  return (
    <section className="permission-notice" role="note">
      <strong>Administrator access required</strong>
      <p>You can view reliability data, but only an owner or admin can connect systems.</p>
    </section>
  );
}

type ConnectionPath = "machine" | "application";

function ConnectionGuide({
  ready,
  actions,
}: {
  ready: DashboardReady;
  actions: ManagementActions;
}) {
  const [path, setPath] = useState<ConnectionPath | null>(null);
  const hasWorkspace = ready.projects.length > 0 && ready.projectId !== null;
  const hasConnection = ready.agents.length > 0 || ready.services.length > 0;
  const hasFreshSignal =
    ready.agents.some((agent) => agent.connection_state === "online") ||
    ready.services.some((service) => Boolean(service.status.observed_at));

  return (
    <section className="connection-guide" aria-labelledby="connection-guide-title">
      <div className="connection-guide-intro">
        <div>
          <p className="eyebrow">First signal journey</p>
          <h2 id="connection-guide-title">Four small steps to useful monitoring</h2>
          <p>
            Follow the current step. Meerkateer uses real workspace data to mark progress—nothing
            turns green until evidence arrives.
          </p>
        </div>
        <ol className="setup-progress" aria-label="Connection progress">
          <ProgressStep label="Company created" state="complete" />
          <ProgressStep label="Workspace ready" state={hasWorkspace ? "complete" : "current"} />
          <ProgressStep
            label="System connected"
            state={hasConnection ? "complete" : hasWorkspace ? "current" : "pending"}
          />
          <ProgressStep
            label="Fresh signal received"
            state={hasFreshSignal ? "complete" : hasConnection ? "current" : "pending"}
          />
        </ol>
      </div>

      <section className="connection-choice" aria-labelledby="connection-choice-title">
        <div className="connection-choice-head">
          <p className="eyebrow">Choose a path</p>
          <h3 id="connection-choice-title">What would you like to watch?</h3>
          <p>You can use both later. Start with the one that gets you useful evidence fastest.</p>
        </div>
        <div className="connection-choice-grid">
          <button
            className={
              path === "machine" ? "connection-choice-card selected" : "connection-choice-card"
            }
            type="button"
            aria-pressed={path === "machine"}
            onClick={() => setPath("machine")}
          >
            <span className="connection-choice-icon" aria-hidden="true">
              ▣
            </span>
            <span>
              <strong>Connect a machine</strong>
              <small>Install the host agent and watch machine freshness.</small>
            </span>
            <span aria-hidden="true">→</span>
          </button>
          <button
            className={
              path === "application" ? "connection-choice-card selected" : "connection-choice-card"
            }
            type="button"
            aria-pressed={path === "application"}
            onClick={() => setPath("application")}
          >
            <span className="connection-choice-icon" aria-hidden="true">
              ◆
            </span>
            <span>
              <strong>Connect an application</strong>
              <small>Use an SDK, or add a Minecraft Java instance.</small>
            </span>
            <span aria-hidden="true">→</span>
          </button>
        </div>
      </section>

      <WorkspaceManagement ready={ready} actions={actions} path={path} />
    </section>
  );
}

function ProgressStep({
  label,
  state,
}: {
  label: string;
  state: "complete" | "current" | "pending";
}) {
  return (
    <li className={`progress-${state}`} aria-current={state === "current" ? "step" : undefined}>
      <span aria-hidden="true">{state === "complete" ? "✓" : ""}</span>
      <strong>{label}</strong>
      <small>{state === "complete" ? "Done" : state === "current" ? "Next" : "Waiting"}</small>
    </li>
  );
}

function WorkspaceManagement({
  ready,
  actions,
  path,
}: {
  ready: DashboardReady;
  actions: ManagementActions;
  path: ConnectionPath | null;
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
    <details className="workspace-management" open>
      <summary>
        {!ready.projectId
          ? "Create the first workspace"
          : path === "machine"
            ? "Machine connection steps"
            : path === "application"
              ? "Application connection steps"
              : "Choose a connection path above"}
      </summary>
      <div className="management-grid">
        <form className="management-card" onSubmit={(event) => void create(event)}>
          <p className="eyebrow">{ready.projectId ? "Another workspace" : "Step 1 · Workspace"}</p>
          <h3>{ready.projectId ? "Create a workspace" : "Name your first workspace"}</h3>
          <p>
            A workspace is one job, fleet, or environment containing many machines and services.
          </p>
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

        {path === "machine" ? (
          <>
            <section
              className="management-card management-card-primary"
              aria-label="Workspace enrollment"
            >
              <p className="eyebrow">Step 2 · New machine</p>
              <h3>Issue an enrollment token</h3>
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
              <p className="eyebrow">Already enrolled?</p>
              <h3>Assign an existing machine</h3>
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
          </>
        ) : null}

        {path === "application" ? (
          <>
            <form
              className="management-card management-card-primary"
              onSubmit={(event) => void createProcess(event)}
            >
              <p className="eyebrow">Step 2 · Service</p>
              <h3>Add a game instance or process</h3>
              <label>
                Monitor type
                <select
                  value={monitorKind}
                  onChange={(event) => setMonitorKind(event.target.value)}
                >
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
                    <input
                      name="game_host"
                      maxLength={253}
                      required
                      placeholder="play.example.com"
                    />
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
                    One instance per service. The first test queries Minecraft status from the
                    Community control plane; scheduled checks and Discord alerts are coming later.
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
              <p className="eyebrow">Existing service</p>
              <h3>Issue another SDK key</h3>
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

            <section className="management-card sdk-guides" aria-label="SDK guides">
              <p className="eyebrow">Step 3 · Send a signal</p>
              <h3>Use your preferred SDK</h3>
              <p>
                Keep the key in an environment variable, then send heartbeats from your process.
              </p>
              <div>
                <a href="/docs/node-sdk">Node.js</a>
                <a href="/docs/go-sdk">Go</a>
                <a href="/docs/rust-sdk">Rust</a>
                <a href="/docs/python-sdk">Python</a>
                <a href="/docs/php-sdk">PHP</a>
              </div>
            </section>
          </>
        ) : null}

        {path === null && ready.projectId ? (
          <div className="management-path-placeholder">
            <img src="/logo.png" alt="" aria-hidden="true" />
            <div>
              <strong>Choose machine or application above.</strong>
              <p>We will show only the setup steps for that path.</p>
            </div>
          </div>
        ) : null}
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
  selectedAgentId,
  canManage,
  onUnassign,
}: {
  agents: AgentResponse[];
  selectedAgentId?: string;
  canManage: boolean;
  onUnassign: (agentId: string) => Promise<void>;
}) {
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [stateFilter, setStateFilter] = useState("all");
  const normalizedQuery = query.trim().toLowerCase();
  const visibleAgents = agents.filter(
    (agent) =>
      (stateFilter === "all" || agent.connection_state === stateFilter) &&
      (agent.display_name.toLowerCase().includes(normalizedQuery) ||
        agent.id.toLowerCase().includes(normalizedQuery)),
  );

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
        <>
          {/* biome-ignore lint/a11y/useSemanticElements: jsdom does not implement the HTML search element yet. */}
          <div className="list-toolbar machine-toolbar" role="search" aria-label="Filter machines">
            <label>
              <span>Search machines</span>
              <input
                aria-label="Search machines"
                type="search"
                value={query}
                placeholder="Name or machine ID"
                onChange={(event) => setQuery(event.target.value)}
              />
            </label>
            <label>
              <span>Connection state</span>
              <select
                aria-label="Filter machines by state"
                value={stateFilter}
                onChange={(event) => setStateFilter(event.target.value)}
              >
                <option value="all">All states</option>
                <option value="online">Online</option>
                <option value="stale">Stale</option>
                <option value="never_seen">Never seen</option>
                <option value="revoked">Revoked</option>
                <option value="quarantined">Quarantined</option>
              </select>
            </label>
          </div>
          {visibleAgents.length === 0 ? (
            <p className="empty filtered-empty" aria-live="polite">
              No machines match these filters.
            </p>
          ) : (
            <div className="machine-grid" aria-live="polite">
              {visibleAgents.map((agent) => (
                <article
                  className={
                    agent.id === selectedAgentId ? "machine-card selected" : "machine-card"
                  }
                  key={agent.id}
                >
                  <div className="machine-card-title">
                    <span className={`machine-light state-${agent.connection_state}`} />
                    <strong>
                      <a
                        href={`/app/machines/${encodeURIComponent(agent.id)}`}
                        aria-current={agent.id === selectedAgentId ? "page" : undefined}
                      >
                        {agent.display_name}
                      </a>
                    </strong>
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
        </>
      )}
      {failure ? (
        <p className="management-feedback" role="alert">
          {failure}
        </p>
      ) : null}
    </section>
  );
}

type MachineTelemetryState =
  | { phase: "loading" }
  | { phase: "ready"; telemetry: AgentTelemetrySnapshotResponse }
  | { phase: "error"; message: string };

function MachineDetail({ agent }: { agent: AgentResponse | null }) {
  const [telemetryState, setTelemetryState] = useState<MachineTelemetryState>({
    phase: "loading",
  });
  const [reload, setReload] = useState(0);

  useEffect(() => {
    if (!agent) return;
    void reload;
    const controller = new AbortController();
    setTelemetryState({ phase: "loading" });
    void fetchAgentTelemetry(agent.id, controller.signal)
      .then((telemetry) => setTelemetryState({ phase: "ready", telemetry }))
      .catch((error: unknown) => {
        if (!controller.signal.aborted) {
          setTelemetryState({ phase: "error", message: message(error) });
        }
      });
    return () => controller.abort();
  }, [agent, reload]);

  if (!agent) {
    return (
      <section className="machine-detail machine-detail-missing" role="status">
        <strong>Machine not found in this workspace.</strong>
        <p>It may have been removed, or the link belongs to another workspace.</p>
        <a href="/app/machines">Back to machines →</a>
      </section>
    );
  }
  return (
    <section className="machine-detail" aria-labelledby="machine-detail-title">
      <div className="machine-detail-title">
        <div>
          <p className="eyebrow">Machine detail</p>
          <h2 id="machine-detail-title">{agent.display_name}</h2>
        </div>
        <StatusPill state={agent.connection_state} />
      </div>
      <dl>
        <div>
          <dt>Last evidence</dt>
          <dd>{agent.last_seen_at ? formatTime(agent.last_seen_at) : "No telemetry received"}</dd>
        </div>
        <div>
          <dt>Enrollment state</dt>
          <dd>{agent.status.replace("_", " ")}</dd>
        </div>
        <div>
          <dt>Machine ID</dt>
          <dd>{agent.id}</dd>
        </div>
      </dl>
      {telemetryState.phase === "loading" ? (
        <div className="machine-telemetry-state" role="status">
          Reading the latest machine snapshotโ€ฆ
        </div>
      ) : telemetryState.phase === "error" ? (
        <div className="machine-telemetry-state telemetry-error" role="alert">
          <div>
            <strong>Machine metrics unavailable</strong>
            <p>{telemetryState.message}</p>
          </div>
          <button type="button" onClick={() => setReload((value) => value + 1)}>
            Retry
          </button>
        </div>
      ) : (
        <MachineTelemetry telemetry={telemetryState.telemetry} />
      )}
    </section>
  );
}

function serviceStateLabel(state: string, running: boolean | null) {
  if (running === true) return state === "reloading" ? "Reloading (available)" : "Running";
  if (running === false) {
    if (state === "activating") return "Starting";
    if (state === "deactivating") return "Stopping";
    if (state === "failed") return "Failed";
    return "Not running";
  }
  const labels: Record<string, string> = {
    not_found: "Service not found",
    permission_denied: "Permission denied",
    manager_unavailable: "Service manager unavailable",
    query_timeout: "Status check timed out",
    unsupported_platform: "Unsupported platform",
    query_failed: "Status check failed",
    unknown: "State unknown",
  };
  return labels[state] ?? "State unknown";
}

function MachineTelemetry({ telemetry }: { telemetry: AgentTelemetrySnapshotResponse }) {
  const stopped = telemetry.processes.filter((process) => !process.running);
  const unhealthyServices = telemetry.services.filter((service) => service.running !== true);
  return (
    <section className="machine-telemetry" aria-labelledby="machine-telemetry-title">
      <div className="machine-telemetry-heading">
        <div>
          <p className="eyebrow">Host snapshot</p>
          <h3 id="machine-telemetry-title">What the agent can see</h3>
        </div>
        <span className={`collection-pill collection-${telemetry.collection_state}`}>
          {telemetry.collection_state}
        </span>
      </div>

      {telemetry.snapshot_stale ? (
        <div className="telemetry-notice telemetry-stale" role="status">
          <strong>This snapshot is stale.</strong> Values and process states may no longer describe
          the machine now.
        </div>
      ) : null}
      {stopped.length > 0 ? (
        <div className="telemetry-notice telemetry-process-down" role="alert">
          <strong>
            {stopped.length} watched {stopped.length === 1 ? "process is" : "processes are"} not
            running.
          </strong>
          <span>{stopped.map((process) => process.name).join(", ")}</span>
        </div>
      ) : null}
      {unhealthyServices.length > 0 ? (
        <div className="telemetry-notice telemetry-service-down" role="alert">
          <strong>OS service attention needed.</strong>{" "}
          {unhealthyServices.map((service) => `${service.name} (${service.state})`).join(", ")}
        </div>
      ) : null}

      <div className="machine-metrics-grid">
        <MachineMetric
          label="CPU"
          value={formatPercent(telemetry.cpu_usage_percent)}
          percent={telemetry.cpu_usage_percent}
        />
        <MachineMetric
          label="Memory"
          value={formatCapacity(telemetry.memory)}
          percent={telemetry.memory?.utilization_percent ?? null}
        />
        <MachineMetric
          label="Disk"
          value={formatCapacity(telemetry.disk)}
          percent={telemetry.disk?.utilization_percent ?? null}
        />
        <MachineMetric
          label="Inodes"
          value={formatInodes(telemetry.inodes)}
          percent={telemetry.inodes?.utilization_percent ?? null}
        />
      </div>

      <div className="machine-evidence-meta">
        <span>
          <strong>Platform</strong>
          {telemetry.platform && telemetry.architecture
            ? `${telemetry.platform} / ${telemetry.architecture}`
            : "Not reported"}
        </span>
        <span>
          <strong>Observed</strong>
          {telemetry.observed_at ? formatTime(telemetry.observed_at) : "No host snapshot"}
        </span>
      </div>

      <div className="process-evidence">
        <div className="process-evidence-heading">
          <h4>Watched processes</h4>
          <small>{telemetry.processes.length} configured in the latest batch</small>
        </div>
        {telemetry.processes.length === 0 ? (
          <p>No process names were configured for this agent run.</p>
        ) : (
          <ul>
            {telemetry.processes.map((process) => (
              <li key={process.name} className={process.running ? "process-up" : "process-down"}>
                <span className="process-dot" aria-hidden="true" />
                <strong>{process.name}</strong>
                <span>
                  {process.running
                    ? `${process.instances} ${process.instances === 1 ? "instance" : "instances"}`
                    : "Not running"}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="process-evidence service-evidence">
        <div className="process-evidence-heading">
          <h4>Watched OS services</h4>
          <small>{telemetry.services.length} configured in the latest batch</small>
        </div>
        {telemetry.services.length === 0 ? (
          <p>No systemd or Windows Service names were configured for this agent run.</p>
        ) : (
          <ul>
            {telemetry.services.map((service) => (
              <li
                key={service.name}
                className={
                  service.running === true
                    ? "process-up"
                    : service.running === false
                      ? "process-down"
                      : "service-unknown"
                }
              >
                <span className="process-dot" aria-hidden="true" />
                <strong>{service.name}</strong>
                <span>{serviceStateLabel(service.state, service.running)}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      {telemetry.missing_metrics.length > 0 ? (
        <details className="missing-metrics">
          <summary>{telemetry.missing_metrics.length} expected metrics unavailable</summary>
          <p>{telemetry.missing_metrics.join(", ")}</p>
        </details>
      ) : null}
    </section>
  );
}

function MachineMetric({
  label,
  value,
  percent,
}: {
  label: string;
  value: string;
  percent: number | null;
}) {
  const bounded = percent === null ? 100 : Math.max(0, Math.min(100, percent));
  return (
    <article className="machine-metric">
      <span>{label}</span>
      <strong>{value}</strong>
      <div
        className={percent === null ? "metric-track metric-missing" : "metric-track"}
        aria-hidden="true"
      >
        <span style={{ width: `${bounded}%` }} />
      </div>
    </article>
  );
}

function formatPercent(value: number | null): string {
  return value === null ? "Not reported" : `${Math.round(value)}%`;
}

function formatCapacity(
  value: AgentTelemetrySnapshotResponse["memory"] | AgentTelemetrySnapshotResponse["disk"],
): string {
  if (!value) return "Not reported";
  return `${formatBytes(value.used_bytes)} / ${formatBytes(value.total_bytes)}`;
}

function formatInodes(value: AgentTelemetrySnapshotResponse["inodes"]): string {
  if (!value) return "Not available";
  return `${Math.round(value.used).toLocaleString()} / ${Math.round(value.total).toLocaleString()}`;
}

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "Unknown";
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  const digits = size >= 10 || unit === 0 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unit]}`;
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
