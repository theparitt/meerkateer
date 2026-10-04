import type { components } from "./generated/api";

export type HealthResponse = components["schemas"]["HealthResponse"];
export type InstanceStateResponse = components["schemas"]["InstanceStateResponse"];
export type SessionResponse = components["schemas"]["SessionResponse"];
export type CreateProjectRequest = components["schemas"]["CreateProjectRequest"];
export type CreateServiceRequest = components["schemas"]["CreateServiceRequest"];
export type ProjectResponse = components["schemas"]["ProjectResponse"];
export type ServiceResponse = components["schemas"]["ServiceResponse"];
export type IssuedCredentialResponse = components["schemas"]["IssuedCredentialResponse"];
export type AgentResponse = components["schemas"]["AgentResponse"];
export type AgentTelemetrySnapshotResponse =
  components["schemas"]["AgentTelemetrySnapshotResponse"];
export type IssuedEnrollmentTokenResponse = components["schemas"]["IssuedEnrollmentTokenResponse"];
export type TimelineItemResponse = components["schemas"]["TimelineItemResponse"];
export type GameProbeResponse = components["schemas"]["GameProbeResponse"];
export type IncidentResponse = components["schemas"]["IncidentResponse"];
export type IncidentActivityResponse = components["schemas"]["IncidentActivityResponse"];
export type AlertDeliveryResponse = components["schemas"]["AlertDeliveryResponse"];
export type AlertPolicyResponse = components["schemas"]["AlertPolicyResponse"];
export type UpdateAlertPolicyRequest = components["schemas"]["UpdateAlertPolicyRequest"];
export type MaintenanceWindowResponse = components["schemas"]["MaintenanceWindowResponse"];
export type CreateMaintenanceWindowRequest =
  components["schemas"]["CreateMaintenanceWindowRequest"];
export type AuditEventResponse = components["schemas"]["AuditEventResponse"];
export type AdminSummaryResponse = components["schemas"]["AdminSummaryResponse"];
export type MemberResponse = components["schemas"]["MemberResponse"];
export type MemberInvitationResponse = components["schemas"]["MemberInvitationResponse"];
export type CreateMemberInvitationRequest = components["schemas"]["CreateMemberInvitationRequest"];
export type IssuedMemberInvitationResponse =
  components["schemas"]["IssuedMemberInvitationResponse"];
export type InvitationPreviewResponse = components["schemas"]["InvitationPreviewResponse"];
export type ChangePasswordRequest = components["schemas"]["ChangePasswordRequest"];
export type IssuedMemberPasswordResetResponse =
  components["schemas"]["IssuedMemberPasswordResetResponse"];
export type PasswordResetPreviewResponse = components["schemas"]["PasswordResetPreviewResponse"];

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isHealthResponse(value: unknown): value is HealthResponse {
  if (!isObject(value)) return false;
  return (
    (value.status === "ok" || value.status === "degraded") &&
    value.interface === "meerkateer" &&
    value.interface_version === "1" &&
    typeof value.service === "string" &&
    typeof value.environment === "string" &&
    typeof value.version === "string" &&
    isObject(value.checks)
  );
}

async function getJson(path: string, signal: AbortSignal): Promise<unknown> {
  const response = await fetch(path, {
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    signal,
  });
  if (!response.ok) throw new Error(`${path} returned HTTP ${response.status}`);
  return response.json();
}

function csrfToken(): string {
  const prefix = "meerkateer_csrf=";
  const token = document.cookie
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(prefix))
    ?.slice(prefix.length);
  if (!token) throw new Error("Your session is missing CSRF protection; sign in again");
  return decodeURIComponent(token);
}

async function mutateJson(
  method: "POST" | "PUT" | "DELETE",
  path: string,
  body?: unknown,
): Promise<unknown> {
  const response = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: {
      Accept: "application/json",
      ...(body === undefined ? {} : { "Content-Type": "application/json" }),
      "X-Meerkateer-CSRF": csrfToken(),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) {
    let detail = `HTTP ${response.status}`;
    try {
      const payload: unknown = await response.json();
      if (isObject(payload) && typeof payload.code === "string") detail = payload.code;
    } catch {
      // Keep the bounded HTTP status when an intermediary did not return JSON.
    }
    throw new Error(`${path} returned ${detail}`);
  }
  return response.status === 204 ? null : response.json();
}

export async function testAlertWebhook(): Promise<void> {
  await mutateJson("POST", "/v1/alerts/test");
}

export async function fetchIncidents(
  projectId: string,
  signal: AbortSignal,
): Promise<IncidentResponse[]> {
  const body = await getJson(
    `/v1/incidents?project_id=${encodeURIComponent(projectId)}&limit=100`,
    signal,
  );
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Incident endpoint returned an unsupported contract");
  }
  return body.items as IncidentResponse[];
}

export async function fetchIncidentActivity(
  projectId: string,
  signal: AbortSignal,
): Promise<IncidentActivityResponse[]> {
  const body = await getJson(
    `/v1/incidents/activity?project_id=${encodeURIComponent(projectId)}&limit=200`,
    signal,
  );
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Incident activity endpoint returned an unsupported contract");
  }
  return body.items as IncidentActivityResponse[];
}

export async function acknowledgeIncident(incidentId: string): Promise<void> {
  await mutateJson("POST", `/v1/incidents/${encodeURIComponent(incidentId)}/acknowledge`);
}

export async function updateIncidentAssignment(
  incidentId: string,
  assigned: boolean,
): Promise<void> {
  await mutateJson("PUT", `/v1/incidents/${encodeURIComponent(incidentId)}/assignment`, {
    assigned,
  });
}

export async function addIncidentNote(incidentId: string, note: string): Promise<void> {
  await mutateJson("POST", `/v1/incidents/${encodeURIComponent(incidentId)}/notes`, { note });
}

export async function fetchAlertDeliveries(
  projectId: string,
  signal: AbortSignal,
): Promise<AlertDeliveryResponse[]> {
  const body = await getJson(
    `/v1/alerts/deliveries?project_id=${encodeURIComponent(projectId)}&limit=100`,
    signal,
  );
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Alert history endpoint returned an unsupported contract");
  }
  return body.items as AlertDeliveryResponse[];
}

export async function replayAlertDelivery(deliveryId: string): Promise<void> {
  await mutateJson("POST", `/v1/alerts/deliveries/${encodeURIComponent(deliveryId)}/replay`);
}

export async function fetchAlertPolicy(signal: AbortSignal): Promise<AlertPolicyResponse> {
  const body = await getJson("/v1/alerts/policy", signal);
  if (!isObject(body) || typeof body.enabled !== "boolean") {
    throw new Error("Alert policy endpoint returned an unsupported contract");
  }
  return body as AlertPolicyResponse;
}

export async function updateAlertPolicy(
  input: UpdateAlertPolicyRequest,
): Promise<AlertPolicyResponse> {
  const body = await mutateJson("PUT", "/v1/alerts/policy", input);
  if (!isObject(body) || typeof body.enabled !== "boolean") {
    throw new Error("Alert policy update returned an unsupported contract");
  }
  return body as AlertPolicyResponse;
}

export async function fetchMaintenanceWindows(
  projectId: string,
  signal: AbortSignal,
): Promise<MaintenanceWindowResponse[]> {
  const body = await getJson(
    `/v1/maintenance-windows?project_id=${encodeURIComponent(projectId)}&limit=100`,
    signal,
  );
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Maintenance endpoint returned an unsupported contract");
  }
  return body.items as MaintenanceWindowResponse[];
}

export async function createMaintenanceWindow(
  input: CreateMaintenanceWindowRequest,
): Promise<MaintenanceWindowResponse> {
  const body = await mutateJson("POST", "/v1/maintenance-windows", input);
  if (!isObject(body) || typeof body.id !== "string") {
    throw new Error("Maintenance creation returned an unsupported contract");
  }
  return body as MaintenanceWindowResponse;
}

export async function cancelMaintenanceWindow(windowId: string): Promise<void> {
  await mutateJson("DELETE", `/v1/maintenance-windows/${encodeURIComponent(windowId)}`);
}

export async function fetchAuditEvents(signal: AbortSignal): Promise<AuditEventResponse[]> {
  const body = await getJson("/v1/audit-events?limit=100", signal);
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Audit endpoint returned an unsupported contract");
  }
  return body.items as AuditEventResponse[];
}

export async function fetchAdminSummary(signal: AbortSignal): Promise<AdminSummaryResponse> {
  const body = await getJson("/v1/admin/summary", signal);
  if (!isObject(body) || typeof body.tenant_id !== "string") {
    throw new Error("Admin summary returned an unsupported contract");
  }
  return body as AdminSummaryResponse;
}

export async function fetchMembers(signal: AbortSignal): Promise<MemberResponse[]> {
  const body = await getJson("/v1/members", signal);
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Member endpoint returned an unsupported contract");
  }
  return body.items as MemberResponse[];
}

export async function fetchMemberInvitations(
  signal: AbortSignal,
): Promise<MemberInvitationResponse[]> {
  const body = await getJson("/v1/members/invitations", signal);
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Invitation endpoint returned an unsupported contract");
  }
  return body.items as MemberInvitationResponse[];
}

export async function createMemberInvitation(
  input: CreateMemberInvitationRequest,
): Promise<IssuedMemberInvitationResponse> {
  const body = await mutateJson("POST", "/v1/members/invitations", input);
  if (!isObject(body) || typeof body.secret !== "string" || !isObject(body.invitation)) {
    throw new Error("Invitation creation returned an unsupported contract");
  }
  return body as IssuedMemberInvitationResponse;
}

export async function cancelMemberInvitation(invitationId: string): Promise<void> {
  await mutateJson("DELETE", `/v1/members/invitations/${encodeURIComponent(invitationId)}`);
}

export async function createMemberPasswordReset(
  userId: string,
  expiresInSeconds = 86_400,
): Promise<IssuedMemberPasswordResetResponse> {
  const body = await mutateJson(
    "POST",
    `/v1/members/${encodeURIComponent(userId)}/password-reset-links`,
    { expires_in_seconds: expiresInSeconds },
  );
  if (!isObject(body) || typeof body.secret !== "string" || typeof body.expires_at !== "string") {
    throw new Error("Password reset creation returned an unsupported contract");
  }
  return body as IssuedMemberPasswordResetResponse;
}

export async function changePassword(input: ChangePasswordRequest): Promise<void> {
  await mutateJson("PUT", "/v1/session/password", input);
}

export async function updateMemberRole(
  userId: string,
  role: "admin" | "operator" | "viewer",
): Promise<void> {
  await mutateJson("PUT", `/v1/members/${encodeURIComponent(userId)}/role`, { role });
}

export async function removeMember(userId: string): Promise<void> {
  await mutateJson("DELETE", `/v1/members/${encodeURIComponent(userId)}`);
}

async function invitationMutation(path: string, body: unknown): Promise<unknown> {
  const response = await fetch(path, {
    method: "POST",
    credentials: "same-origin",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    let detail = `HTTP ${response.status}`;
    try {
      const payload: unknown = await response.json();
      if (isObject(payload) && typeof payload.code === "string") detail = payload.code;
    } catch {
      // Keep the HTTP status if the response was not JSON.
    }
    throw new Error(detail);
  }
  return response.status === 204 ? null : response.json();
}

export async function inspectMemberInvitation(token: string): Promise<InvitationPreviewResponse> {
  const body = await invitationMutation("/v1/member-invitations/inspect", { token });
  if (!isObject(body) || typeof body.company !== "string") {
    throw new Error("Invitation preview returned an unsupported contract");
  }
  return body as InvitationPreviewResponse;
}

export async function acceptMemberInvitation(
  token: string,
  password: string,
): Promise<SessionResponse> {
  const body = await invitationMutation("/v1/member-invitations/accept", { token, password });
  if (!isObject(body) || typeof body.user_id !== "string") {
    throw new Error("Invitation acceptance returned an unsupported contract");
  }
  return body as SessionResponse;
}

export async function declineMemberInvitation(token: string): Promise<void> {
  await invitationMutation("/v1/member-invitations/decline", { token });
}

export async function inspectMemberPasswordReset(
  token: string,
): Promise<PasswordResetPreviewResponse> {
  const body = await invitationMutation("/v1/member-password-resets/inspect", { token });
  if (!isObject(body) || typeof body.company !== "string") {
    throw new Error("Password reset preview returned an unsupported contract");
  }
  return body as PasswordResetPreviewResponse;
}

export async function acceptMemberPasswordReset(
  token: string,
  password: string,
): Promise<SessionResponse> {
  const body = await invitationMutation("/v1/member-password-resets/accept", { token, password });
  if (!isObject(body) || typeof body.user_id !== "string") {
    throw new Error("Password reset acceptance returned an unsupported contract");
  }
  return body as SessionResponse;
}

export async function fetchHealth(signal: AbortSignal): Promise<HealthResponse> {
  const body = await getJson("/health", signal);
  if (!isHealthResponse(body)) throw new Error("Health endpoint returned an unsupported contract");
  return body;
}

export async function fetchInstanceState(
  signal: AbortSignal,
): Promise<InstanceStateResponse | null> {
  const response = await fetch("/v1/instance", { headers: { Accept: "application/json" }, signal });
  if (
    response.status === 404 ||
    !response.headers.get("Content-Type")?.includes("application/json")
  ) {
    return null;
  }
  if (!response.ok) throw new Error(`Instance state returned HTTP ${response.status}`);
  const body: unknown = await response.json();
  if (
    !isObject(body) ||
    (body.deployment_mode !== "community" && body.deployment_mode !== "cloud") ||
    typeof body.setup_required !== "boolean"
  ) {
    throw new Error("Instance state returned an unsupported contract");
  }
  return body as InstanceStateResponse;
}

export async function fetchSession(signal: AbortSignal): Promise<SessionResponse | null> {
  const response = await fetch("/v1/session", {
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    signal,
  });
  if (response.status === 401) return null;
  if (!response.ok) throw new Error(`Session endpoint returned HTTP ${response.status}`);
  const body: unknown = await response.json();
  if (!isObject(body) || typeof body.tenant_id !== "string" || typeof body.email !== "string") {
    throw new Error("Session endpoint returned an unsupported contract");
  }
  return body as SessionResponse;
}

export async function signOut(): Promise<void> {
  await mutateJson("DELETE", "/v1/session");
}

async function authenticationMutation(
  path: string,
  adminToken: string,
  body?: unknown,
): Promise<unknown> {
  const response = await fetch(path, {
    method: "POST",
    credentials: "same-origin",
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${adminToken}`,
      ...(body === undefined ? {} : { "Content-Type": "application/json" }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) {
    let detail = `HTTP ${response.status}`;
    try {
      const payload: unknown = await response.json();
      if (isObject(payload) && typeof payload.code === "string") detail = payload.code;
    } catch {
      // Keep the HTTP status if an intermediary did not return JSON.
    }
    throw new Error(detail);
  }
  return (await response.json()) as SessionResponse;
}

export async function passwordLogin(email: string, password: string): Promise<SessionResponse> {
  const response = await fetch("/v1/session/password-login", {
    method: "POST",
    credentials: "same-origin",
    headers: { Accept: "application/json", "Content-Type": "application/json" },
    body: JSON.stringify({ email: email.trim().toLowerCase(), password }),
  });
  if (!response.ok) {
    let detail = `HTTP ${response.status}`;
    try {
      const payload: unknown = await response.json();
      if (isObject(payload) && typeof payload.code === "string") detail = payload.code;
    } catch {
      // Keep the HTTP status if the response was not JSON.
    }
    throw new Error(detail);
  }
  return (await response.json()) as SessionResponse;
}

export async function setupOwnerPassword(adminToken: string, password: string): Promise<void> {
  await authenticationMutation("/v1/session/password-setup", adminToken, { password });
}

export async function bootstrapCommunity(
  adminToken: string,
  input: {
    tenant_slug: string;
    tenant_name: string;
    owner_email: string;
    owner_name: string;
    owner_password: string;
  },
): Promise<void> {
  await authenticationMutation("/v1/bootstrap", adminToken, input);
}

export async function fetchProjects(signal: AbortSignal): Promise<ProjectResponse[]> {
  const body = await getJson("/v1/projects", signal);
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Project endpoint returned an unsupported contract");
  }
  return body.items as ProjectResponse[];
}

export async function fetchServices(
  projectId: string,
  signal: AbortSignal,
): Promise<ServiceResponse[]> {
  const body = await getJson(`/v1/projects/${encodeURIComponent(projectId)}/services`, signal);
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Service endpoint returned an unsupported contract");
  }
  return body.items as ServiceResponse[];
}

export async function fetchWorkspaceAgents(
  projectId: string,
  signal: AbortSignal,
): Promise<AgentResponse[]> {
  const body = await getJson(`/v1/projects/${encodeURIComponent(projectId)}/agents`, signal);
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Workspace machine endpoint returned an unsupported contract");
  }
  return body.items as AgentResponse[];
}

export async function fetchAgents(signal: AbortSignal): Promise<AgentResponse[]> {
  const body = await getJson("/v1/agents", signal);
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Company machine endpoint returned an unsupported contract");
  }
  return body.items as AgentResponse[];
}

export async function fetchAgentTelemetry(
  agentId: string,
  signal: AbortSignal,
): Promise<AgentTelemetrySnapshotResponse> {
  const body = await getJson(`/v1/agents/${encodeURIComponent(agentId)}/telemetry`, signal);
  if (
    !isObject(body) ||
    body.agent_id !== agentId ||
    !Array.isArray(body.processes) ||
    !Array.isArray(body.missing_metrics)
  ) {
    throw new Error("Machine telemetry endpoint returned an unsupported contract");
  }
  return body as AgentTelemetrySnapshotResponse;
}

export async function createWorkspace(input: CreateProjectRequest): Promise<ProjectResponse> {
  const body = await mutateJson("POST", "/v1/projects", input);
  if (!isObject(body) || typeof body.id !== "string" || typeof body.display_name !== "string") {
    throw new Error("Workspace creation returned an unsupported contract");
  }
  return body as ProjectResponse;
}

export async function createService(
  projectId: string,
  input: CreateServiceRequest,
): Promise<ServiceResponse> {
  const body = await mutateJson(
    "POST",
    `/v1/projects/${encodeURIComponent(projectId)}/services`,
    input,
  );
  if (!isObject(body) || typeof body.id !== "string" || typeof body.slug !== "string") {
    throw new Error("Process creation returned an unsupported contract");
  }
  return body as ServiceResponse;
}

export async function issueServiceCredential(serviceId: string): Promise<IssuedCredentialResponse> {
  const body = await mutateJson(
    "POST",
    `/v1/services/${encodeURIComponent(serviceId)}/credentials`,
  );
  if (
    !isObject(body) ||
    typeof body.credential_id !== "string" ||
    typeof body.secret !== "string"
  ) {
    throw new Error("SDK credential response was unsupported");
  }
  return body as IssuedCredentialResponse;
}

export async function issueWorkspaceEnrollmentToken(
  projectId: string,
  expiresInSeconds = 600,
): Promise<IssuedEnrollmentTokenResponse> {
  const body = await mutateJson(
    "POST",
    `/v1/projects/${encodeURIComponent(projectId)}/enrollment-tokens`,
    { expires_in_seconds: expiresInSeconds },
  );
  if (
    !isObject(body) ||
    typeof body.token_id !== "string" ||
    typeof body.secret !== "string" ||
    body.project_id !== projectId
  ) {
    throw new Error("Enrollment token endpoint returned an unsupported contract");
  }
  return body as IssuedEnrollmentTokenResponse;
}

export async function assignWorkspaceAgent(projectId: string, agentId: string): Promise<void> {
  await mutateJson(
    "PUT",
    `/v1/projects/${encodeURIComponent(projectId)}/agents/${encodeURIComponent(agentId)}`,
  );
}

export async function unassignWorkspaceAgent(projectId: string, agentId: string): Promise<void> {
  await mutateJson(
    "DELETE",
    `/v1/projects/${encodeURIComponent(projectId)}/agents/${encodeURIComponent(agentId)}`,
  );
}

export async function fetchTimeline(
  serviceId: string,
  signal: AbortSignal,
): Promise<TimelineItemResponse[]> {
  const body = await getJson(
    `/v1/services/${encodeURIComponent(serviceId)}/timeline?limit=100`,
    signal,
  );
  if (!isObject(body) || !Array.isArray(body.items)) {
    throw new Error("Timeline endpoint returned an unsupported contract");
  }
  return body.items as TimelineItemResponse[];
}

export async function testMinecraftStatus(serviceId: string): Promise<GameProbeResponse> {
  const body = await mutateJson("POST", `/v1/services/${encodeURIComponent(serviceId)}/game-probe`);
  if (!isObject(body) || typeof body.state !== "string" || typeof body.observed_at !== "string") {
    throw new Error("Minecraft status probe returned an unsupported contract");
  }
  return body as GameProbeResponse;
}
