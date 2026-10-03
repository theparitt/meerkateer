//! HTTP surface for the Meerkateer control plane.

mod minecraft_probe;
mod rate_limit;

use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use actix_web::{
    HttpRequest, HttpResponse, Responder,
    cookie::{Cookie, SameSite, time::Duration as CookieDuration},
    http::{StatusCode, header},
    web,
};
use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};
use meerkateer_config::{DeploymentMode, ServerConfig};
use meerkateer_identity::{
    Action, IssuedAgentCredential, IssuedEnrollmentToken, IssuedServiceKey, IssuedSession,
    Principal, Role, constant_time_secret_eq, hash_local_password, verify_local_password,
};
use meerkateer_protocol::mka1::{PROTOCOL_VERSION, TelemetryBatch, TelemetryRecord};
use meerkateer_protocol::mks1::{
    BuildInfo, CheckResult, DeployIngest, EventIngest, HealthResponse, HeartbeatIngest,
    HeartbeatStatus, INTERFACE, INTERFACE_VERSION, MetadataCapabilities, MetadataEndpoints,
    MetadataResponse, ReadyResponse, SafeCheckError, ServerInfo,
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use tokio::{sync::Semaphore, time::timeout};

use crate::rate_limit::{RateLimiter, Scope as RateLimitScope};

static DUMMY_LOCAL_PASSWORD_HASH: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub started_at: Instant,
    pub build: BuildInfo,
    pub database: Option<PgPool>,
    pub credential_verifier: Arc<Semaphore>,
    rate_limiter: Arc<RateLimiter>,
}

impl AppState {
    #[must_use]
    pub fn new(config: Arc<ServerConfig>, database: Option<PgPool>) -> Self {
        Self {
            config,
            started_at: Instant::now(),
            build: BuildInfo::current(),
            database,
            credential_verifier: Arc::new(Semaphore::new(8)),
            rate_limiter: Arc::new(RateLimiter::new()),
        }
    }
}

pub fn configure_routes(config: &mut web::ServiceConfig) {
    config
        .route("/live", web::get().to(live))
        .route("/health", web::get().to(health))
        .route("/ready", web::get().to(ready))
        .route("/metrics", web::get().to(metrics))
        .route("/openapi.json", web::get().to(openapi))
        .route("/v1/instance", web::get().to(instance_state))
        .service(
            web::resource("/v1/agent/telemetry")
                .app_data(web::PayloadConfig::new(2 * 1024 * 1024))
                .route(web::post().to(ingest_agent_telemetry)),
        )
        .service(
            web::scope("/v1")
                .app_data(web::JsonConfig::default().limit(16 * 1024))
                .app_data(web::PayloadConfig::new(16 * 1024))
                .route("/bootstrap", web::post().to(bootstrap))
                .service(session_routes())
                .service(alert_routes())
                .service(incident_routes())
                .route(
                    "/maintenance-windows",
                    web::get().to(list_maintenance_windows),
                )
                .route(
                    "/maintenance-windows",
                    web::post().to(create_maintenance_window),
                )
                .route(
                    "/maintenance-windows/{window_id}",
                    web::delete().to(cancel_maintenance_window),
                )
                .route("/audit-events", web::get().to(list_audit_events))
                .route("/admin/summary", web::get().to(admin_summary))
                .route("/ingest/heartbeat", web::post().to(ingest_heartbeat))
                .route("/ingest/event", web::post().to(ingest_event))
                .route("/ingest/deploy", web::post().to(ingest_deploy))
                .route("/projects", web::get().to(list_projects))
                .route("/projects", web::post().to(create_project))
                .route("/enrollment-tokens", web::post().to(issue_enrollment_token))
                .route(
                    "/projects/{project_id}/enrollment-tokens",
                    web::post().to(issue_project_enrollment_token),
                )
                .route("/agents/enroll", web::post().to(enroll_agent))
                .route("/agents", web::get().to(list_agents))
                .route(
                    "/agents/{agent_id}/telemetry",
                    web::get().to(get_agent_telemetry),
                )
                .route("/agents/{agent_id}", web::delete().to(revoke_agent))
                .route(
                    "/projects/{project_id}/agents",
                    web::get().to(list_project_agents),
                )
                .route(
                    "/projects/{project_id}/agents/{agent_id}",
                    web::put().to(assign_project_agent),
                )
                .route(
                    "/projects/{project_id}/agents/{agent_id}",
                    web::delete().to(unassign_project_agent),
                )
                .route(
                    "/agents/{agent_id}/credentials/rotate",
                    web::post().to(rotate_agent_credential),
                )
                .route(
                    "/projects/{project_id}/services",
                    web::get().to(list_services),
                )
                .route(
                    "/projects/{project_id}/services",
                    web::post().to(create_service),
                )
                .route(
                    "/services/{service_id}/credentials",
                    web::post().to(issue_service_credential),
                )
                .route(
                    "/services/{service_id}/timeline",
                    web::get().to(get_service_timeline),
                )
                .route(
                    "/services/{service_id}/game-probe",
                    web::post().to(test_game_probe),
                )
                .route(
                    "/services/{service_id}/credentials/{credential_id}/rotate",
                    web::post().to(rotate_service_credential),
                )
                .route(
                    "/services/{service_id}/credentials/{credential_id}",
                    web::delete().to(revoke_service_credential),
                ),
        )
        .route("/server-info", web::get().to(server_info))
        .route("/.well-known/meerkateer.json", web::get().to(metadata));
}

fn session_routes() -> actix_web::Scope {
    web::scope("")
        .route("/session/password-login", web::post().to(password_login))
        .route("/session/password-setup", web::post().to(password_setup))
        .route("/session", web::get().to(current_session))
        .route("/session", web::delete().to(logout))
}

fn incident_routes() -> actix_web::Scope {
    web::scope("/incidents")
        .route("", web::get().to(list_incidents))
        .route("/activity", web::get().to(list_incident_activity))
        .route(
            "/{incident_id}/acknowledge",
            web::post().to(acknowledge_incident),
        )
        .route(
            "/{incident_id}/assignment",
            web::put().to(update_incident_assignment),
        )
        .route("/{incident_id}/notes", web::post().to(add_incident_note))
}

fn alert_routes() -> actix_web::Scope {
    web::scope("/alerts")
        .route("/test", web::post().to(test_alert_webhook))
        .route("/policy", web::get().to(get_alert_policy))
        .route("/policy", web::put().to(update_alert_policy))
        .route("/deliveries", web::get().to(list_alert_deliveries))
        .route(
            "/deliveries/{delivery_id}/replay",
            web::post().to(replay_alert_delivery),
        )
}

#[derive(Debug, Serialize)]
struct LiveResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct InstanceStateResponse {
    deployment_mode: &'static str,
    setup_required: bool,
}

fn alert_webhook_url() -> Result<Option<reqwest::Url>, ()> {
    let value = match std::env::var("MEERKATEER_ALERT_WEBHOOK_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Ok(None),
    };
    let url = reqwest::Url::parse(&value).map_err(|_| ())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(());
    }
    let host = url.host_str().ok_or(())?;
    let loopback = host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if url.scheme() != "https" && !loopback {
        return Err(());
    }
    Ok(Some(url))
}

async fn test_alert_webhook(request: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session.principal.role != Role::Owner {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    if state.config.deployment_mode != DeploymentMode::Community {
        return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
    }
    let Ok(Some(url)) = alert_webhook_url() else {
        return HttpResponse::Conflict().json(ErrorResponse {
            code: "alert_webhook_not_configured",
        });
    };
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let owns_bootstrap: Result<bool, _> = sqlx::query_scalar(
        "SELECT bootstrap_tenant_id = $1 FROM system_state WHERE singleton_id = 1",
    )
    .bind(session.principal.tenant_id)
    .fetch_one(database)
    .await;
    if !matches!(owns_bootstrap, Ok(true)) {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let Ok(client) = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
    else {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            code: "alert_transport_unavailable",
        });
    };
    let response = client
        .post(url)
        .json(&serde_json::json!({ "content": "[TEST] Meerkateer Community alert channel is connected." }))
        .send()
        .await;
    match response {
        Ok(response) if response.status().is_success() => HttpResponse::NoContent().finish(),
        _ => HttpResponse::BadGateway().json(ErrorResponse {
            code: "alert_delivery_failed",
        }),
    }
}

async fn instance_state(state: web::Data<AppState>) -> HttpResponse {
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let setup_required: Result<bool, _> = sqlx::query_scalar(
        "SELECT bootstrap_completed_at IS NULL FROM system_state WHERE singleton_id = 1",
    )
    .fetch_one(database)
    .await;
    match setup_required {
        Ok(setup_required) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(InstanceStateResponse {
                deployment_mode: state.config.deployment_mode.as_str(),
                setup_required,
            }),
        Err(_) => database_unavailable(),
    }
}

async fn live() -> impl Responder {
    web::Json(LiveResponse { status: "ok" })
}

async fn openapi() -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .body(include_str!("../../openapi/meerkateer.openapi.json"))
}

#[derive(Deserialize)]
struct BootstrapRequest {
    tenant_slug: String,
    tenant_name: String,
    owner_email: String,
    owner_name: String,
    owner_password: String,
}

#[derive(Deserialize)]
struct PasswordLoginRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct PasswordSetupRequest {
    password: String,
}

#[derive(Debug, Serialize)]
struct BootstrapResponse {
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
    role: &'static str,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    code: &'static str,
}

#[derive(Debug, Serialize)]
struct SessionResponse {
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
    role: String,
    email: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct CreateProjectRequest {
    slug: String,
    display_name: String,
}

#[derive(Debug, Serialize)]
struct ProjectResponse {
    id: uuid::Uuid,
    slug: String,
    display_name: String,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ProjectListResponse {
    items: Vec<ProjectResponse>,
}

#[derive(Debug, Deserialize)]
struct CreateServiceRequest {
    slug: String,
    environment: String,
    game: Option<GameConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameConfig {
    kind: String,
    host: String,
    port: u16,
}

#[derive(Debug, Serialize)]
struct ServiceResponse {
    id: uuid::Uuid,
    project_id: uuid::Uuid,
    slug: String,
    environment: String,
    created_at: chrono::DateTime<Utc>,
    game: Option<GameConfig>,
    status: ServiceStatusResponse,
}

#[derive(Debug, Serialize)]
struct ServiceStatusResponse {
    state: String,
    reported_state: Option<String>,
    stale: bool,
    last_sequence: Option<i64>,
    observed_at: Option<chrono::DateTime<Utc>>,
    updated_at: Option<chrono::DateTime<Utc>>,
}

impl ServiceStatusResponse {
    fn unknown() -> Self {
        Self {
            state: "unknown".to_owned(),
            reported_state: None,
            stale: false,
            last_sequence: None,
            observed_at: None,
            updated_at: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct ServiceListResponse {
    items: Vec<ServiceResponse>,
}

#[derive(Debug, Deserialize)]
struct TimelineQuery {
    limit: Option<u16>,
}

#[derive(Debug, Serialize)]
struct TimelineItemResponse {
    idempotency_key: uuid::Uuid,
    kind: String,
    state: Option<String>,
    severity: Option<String>,
    title: String,
    message: Option<String>,
    observed_at: chrono::DateTime<Utc>,
    received_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct TimelineResponse {
    items: Vec<TimelineItemResponse>,
}

#[derive(Debug, Deserialize)]
struct ProjectListQuery {
    project_id: uuid::Uuid,
    limit: Option<u16>,
}

#[derive(Debug, Serialize)]
struct IncidentResponse {
    id: uuid::Uuid,
    project_id: uuid::Uuid,
    service_id: uuid::Uuid,
    service: String,
    status: String,
    severity: String,
    title: String,
    cause: String,
    started_at: chrono::DateTime<Utc>,
    last_observed_at: chrono::DateTime<Utc>,
    resolved_at: Option<chrono::DateTime<Utc>>,
    acknowledged_at: Option<chrono::DateTime<Utc>>,
    acknowledged_by: Option<String>,
    assigned_to: Option<uuid::Uuid>,
    assignee: Option<String>,
}

type IncidentRow = (
    uuid::Uuid,
    uuid::Uuid,
    uuid::Uuid,
    String,
    String,
    String,
    String,
    String,
    chrono::DateTime<Utc>,
    chrono::DateTime<Utc>,
    Option<chrono::DateTime<Utc>>,
    Option<chrono::DateTime<Utc>>,
    Option<String>,
    Option<uuid::Uuid>,
    Option<String>,
);

#[derive(Debug, Serialize)]
struct IncidentListResponse {
    items: Vec<IncidentResponse>,
}

#[derive(Debug, Serialize)]
struct IncidentActivityResponse {
    id: uuid::Uuid,
    incident_id: uuid::Uuid,
    kind: String,
    actor: String,
    note: Option<String>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct IncidentActivityListResponse {
    items: Vec<IncidentActivityResponse>,
}

#[derive(Debug, Deserialize)]
struct UpdateIncidentAssignmentRequest {
    assigned: bool,
}

#[derive(Debug, Deserialize)]
struct AddIncidentNoteRequest {
    note: String,
}

#[derive(Debug, Serialize)]
struct AlertDeliveryResponse {
    id: uuid::Uuid,
    project_id: uuid::Uuid,
    service_id: uuid::Uuid,
    service: String,
    incident_id: Option<uuid::Uuid>,
    replay_of: Option<uuid::Uuid>,
    transition: String,
    status: String,
    observed_at: chrono::DateTime<Utc>,
    created_at: chrono::DateTime<Utc>,
    delivered_at: Option<chrono::DateTime<Utc>>,
    attempts: i32,
    last_error: Option<String>,
    suppression_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct AlertDeliveryListResponse {
    items: Vec<AlertDeliveryResponse>,
}

#[derive(Debug, Deserialize)]
struct UpdateAlertPolicyRequest {
    enabled: bool,
    notify_down: bool,
    notify_recovered: bool,
    cooldown_seconds: i32,
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
struct WebhookConfigured(bool);

#[derive(Debug, Serialize)]
struct AlertPolicyResponse {
    enabled: bool,
    notify_down: bool,
    notify_recovered: bool,
    cooldown_seconds: i32,
    webhook_configured: WebhookConfigured,
    updated_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct CreateMaintenanceWindowRequest {
    project_id: uuid::Uuid,
    service_id: uuid::Uuid,
    title: String,
    reason: String,
    starts_at: chrono::DateTime<Utc>,
    ends_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct MaintenanceWindowResponse {
    id: uuid::Uuid,
    project_id: uuid::Uuid,
    service_id: uuid::Uuid,
    service: String,
    title: String,
    reason: String,
    starts_at: chrono::DateTime<Utc>,
    ends_at: chrono::DateTime<Utc>,
    created_at: chrono::DateTime<Utc>,
    cancelled_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct MaintenanceWindowListResponse {
    items: Vec<MaintenanceWindowResponse>,
}

#[derive(Debug, Deserialize)]
struct LimitQuery {
    limit: Option<u16>,
}

#[derive(Debug, Serialize)]
struct AuditEventResponse {
    id: uuid::Uuid,
    actor_type: String,
    actor_id: Option<uuid::Uuid>,
    action: String,
    target_type: String,
    target_id: Option<uuid::Uuid>,
    details: serde_json::Value,
    occurred_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AuditEventListResponse {
    items: Vec<AuditEventResponse>,
}

#[derive(Debug, Serialize)]
struct AdminSummaryResponse {
    tenant_id: uuid::Uuid,
    deployment_mode: &'static str,
    projects: i64,
    services: i64,
    agents: i64,
    open_incidents: i64,
    pending_alerts: i64,
    dead_lettered_alerts: i64,
    active_maintenance_windows: i64,
    oldest_pending_alert_at: Option<chrono::DateTime<Utc>>,
    worker_status: &'static str,
    worker_started_at: Option<chrono::DateTime<Utc>>,
    worker_last_cycle_at: Option<chrono::DateTime<Utc>>,
    worker_last_cycle_claimed: i32,
    worker_last_cycle_completed: i32,
    worker_last_cycle_retried: i32,
    worker_last_cycle_dead_lettered: i32,
}

#[derive(Debug, Deserialize)]
struct IssueEnrollmentTokenRequest {
    expires_in_seconds: u32,
}

#[derive(Debug, Serialize)]
struct IssuedEnrollmentTokenResponse {
    token_id: uuid::Uuid,
    project_id: Option<uuid::Uuid>,
    prefix: String,
    secret: String,
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct EnrollAgentRequest {
    installation_id: uuid::Uuid,
    display_name: String,
}

#[derive(Debug, Serialize)]
struct EnrolledAgentResponse {
    agent_id: uuid::Uuid,
    project_id: Option<uuid::Uuid>,
    credential_id: uuid::Uuid,
    secret: String,
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AgentResponse {
    id: uuid::Uuid,
    display_name: String,
    status: String,
    connection_state: String,
    enrolled_at: chrono::DateTime<Utc>,
    last_seen_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct AgentListResponse {
    items: Vec<AgentResponse>,
}

#[derive(Debug, Serialize)]
struct AgentCapacityResponse {
    used_bytes: f64,
    total_bytes: f64,
    utilization_percent: f64,
}

#[derive(Debug, Serialize)]
struct AgentCountCapacityResponse {
    used: f64,
    total: f64,
    utilization_percent: f64,
}

#[derive(Debug, Serialize)]
struct AgentProcessResponse {
    name: String,
    running: bool,
    instances: u32,
}

#[derive(Debug, Serialize)]
struct AgentServiceResponse {
    name: String,
    running: Option<bool>,
    state: String,
}

#[derive(Debug, Serialize)]
struct AgentTelemetrySnapshotResponse {
    agent_id: uuid::Uuid,
    connection_state: String,
    collection_state: &'static str,
    observed_at: Option<chrono::DateTime<Utc>>,
    received_at: Option<chrono::DateTime<Utc>>,
    snapshot_stale: bool,
    platform: Option<String>,
    architecture: Option<String>,
    cpu_usage_percent: Option<f64>,
    memory: Option<AgentCapacityResponse>,
    disk: Option<AgentCapacityResponse>,
    inodes: Option<AgentCountCapacityResponse>,
    processes: Vec<AgentProcessResponse>,
    services: Vec<AgentServiceResponse>,
    missing_metrics: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct RotatedAgentCredentialResponse {
    credential_id: uuid::Uuid,
    secret: String,
    expires_at: chrono::DateTime<Utc>,
    previous_valid_until: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct IngestAcknowledgement {
    status: &'static str,
    idempotency_key: uuid::Uuid,
    received_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AgentTelemetryAcknowledgement {
    protocol_version: &'static str,
    agent_id: uuid::Uuid,
    batch_id: uuid::Uuid,
    status: &'static str,
    accepted_through_sequence: u64,
    rejected: Vec<AgentTelemetryRejection>,
    server_time: String,
}

#[derive(Debug, Serialize)]
struct AgentTelemetryRejection {
    sequence: u64,
    reason: &'static str,
}

#[derive(Debug, Clone)]
struct AuthenticatedService {
    tenant: uuid::Uuid,
    project: uuid::Uuid,
    service: uuid::Uuid,
    credential: uuid::Uuid,
    service_slug: String,
    project_slug: String,
    environment: String,
}

#[derive(Debug, Clone, Copy)]
enum IngestKind {
    Heartbeat,
    Event,
    Deploy,
}

impl IngestKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Heartbeat => "heartbeat",
            Self::Event => "event",
            Self::Deploy => "deploy",
        }
    }

    const fn maximum_age(self) -> ChronoDuration {
        match self {
            Self::Heartbeat => ChronoDuration::minutes(15),
            Self::Event | Self::Deploy => ChronoDuration::hours(24),
        }
    }
}

#[derive(Debug)]
struct AuthenticatedSession {
    session_id: uuid::Uuid,
    principal: Principal,
    csrf_digest: [u8; 32],
    email: String,
    display_name: String,
}

#[derive(Debug, Clone, Copy)]
enum AuthenticationFailure {
    Unauthorized,
    RateLimited,
    Database,
}

async fn current_session(request: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    match authenticate_session(&request, &state).await {
        Ok(session) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(SessionResponse {
                tenant_id: session.principal.tenant_id,
                user_id: session.principal.user_id,
                role: session.principal.role.as_str().to_owned(),
                email: session.email,
                display_name: session.display_name,
            }),
        Err(AuthenticationFailure::Unauthorized) => unauthorized(),
        Err(AuthenticationFailure::RateLimited) => rate_limited(),
        Err(AuthenticationFailure::Database) => {
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                code: "database_unavailable",
            })
        }
    }
}

async fn logout(request: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    if revoke_session(database, &session).await.is_err() {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    }

    HttpResponse::NoContent()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(expired_session_cookie(&state, "meerkateer_session", true))
        .cookie(expired_session_cookie(&state, "meerkateer_csrf", false))
        .finish()
}

fn expired_session_cookie(
    state: &AppState,
    name: &'static str,
    http_only: bool,
) -> Cookie<'static> {
    Cookie::build(name, "")
        .http_only(http_only)
        .secure(state.config.environment == "production")
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(CookieDuration::ZERO)
        .finish()
}

async fn revoke_session(database: &PgPool, session: &AuthenticatedSession) -> Result<(), ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .map_err(|_| ())?;
    let result = sqlx::query(
        "UPDATE sessions SET revoked_at = now() \
         WHERE tenant_id = $1 AND id = $2 AND revoked_at IS NULL",
    )
    .bind(session.principal.tenant_id)
    .bind(session.session_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ())?;
    if result.rows_affected() == 1 {
        sqlx::query(
            "INSERT INTO audit_events \
             (tenant_id, actor_type, actor_id, action, target_type, target_id) \
             VALUES ($1, 'user', $2, 'session.logout', 'session', $3)",
        )
        .bind(session.principal.tenant_id)
        .bind(session.principal.user_id)
        .bind(session.session_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
    }
    transaction.commit().await.map_err(|_| ())
}

fn valid_local_password(value: &str) -> bool {
    (12..=1024).contains(&value.len())
}

async fn hash_owner_password(state: &AppState, value: &str) -> Result<String, ()> {
    let permit = Arc::clone(&state.credential_verifier)
        .acquire_owned()
        .await
        .map_err(|_| ())?;
    let password = secrecy::SecretString::from(value.to_owned());
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        hash_local_password(&password)
    })
    .await
    .map_err(|_| ())?
    .map_err(|_| ())
}

async fn password_login(
    request: HttpRequest,
    body: web::Json<PasswordLoginRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    if require_rate_limit(&request, &state, RateLimitScope::PasswordLogin)
        .await
        .is_err()
    {
        return rate_limited();
    }
    if state.config.deployment_mode != DeploymentMode::Community {
        return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
    }
    if !valid_email(&body.email.trim().to_ascii_lowercase())
        || !valid_local_password(&body.password)
    {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            code: "invalid_credentials",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let tenant_id = match select_bootstrap_tenant(database).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return HttpResponse::Conflict().json(ErrorResponse {
                code: "bootstrap_required",
            });
        }
        Err(()) => return database_unavailable(),
    };
    let Ok(password_hash) = find_owner_password_hash(database, tenant_id, &body.email).await else {
        return database_unavailable();
    };
    let Ok(permit) = Arc::clone(&state.credential_verifier).acquire_owned().await else {
        return database_unavailable();
    };
    let password = secrecy::SecretString::from(body.password.clone());
    let known_owner = password_hash.is_some();
    let verified = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let hash = password_hash.unwrap_or_else(|| {
            DUMMY_LOCAL_PASSWORD_HASH
                .get_or_init(|| {
                    hash_local_password(&secrecy::SecretString::from(
                        "not-a-real-community-owner-password".to_owned(),
                    ))
                    .unwrap_or_default()
                })
                .clone()
        });
        verify_local_password(&password, &hash)
    })
    .await;
    if !known_owner || !matches!(verified, Ok(Ok(()))) {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            code: "invalid_credentials",
        });
    }
    let session = IssuedSession::issue(tenant_id);
    let expires_at = Utc::now() + ChronoDuration::hours(12);
    let identity = match insert_local_owner_session(
        database,
        tenant_id,
        &session,
        expires_at,
        "session.password_login",
    )
    .await
    {
        Ok(Some(identity)) => identity,
        Ok(None) => {
            return HttpResponse::Conflict().json(ErrorResponse {
                code: "owner_not_found",
            });
        }
        Err(()) => return database_unavailable(),
    };
    authenticated_session_response(&state, &session, identity, expires_at)
}

async fn find_owner_password_hash(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    email: &str,
) -> Result<Option<String>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    sqlx::query("SELECT set_config('meerkateer.tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
    let result = sqlx::query_scalar::<_, Option<String>>(
        "SELECT users.local_password_hash FROM users \
         JOIN memberships ON memberships.user_id = users.id \
         WHERE memberships.tenant_id = $1 AND memberships.role = 'owner' \
           AND users.email = $2 AND users.disabled_at IS NULL \
         ORDER BY memberships.created_at, users.id LIMIT 1",
    )
    .bind(tenant_id)
    .bind(email.trim().to_ascii_lowercase())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(result.flatten())
}

async fn password_setup(
    request: HttpRequest,
    body: web::Json<PasswordSetupRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    if require_rate_limit(&request, &state, RateLimitScope::Authentication)
        .await
        .is_err()
    {
        return rate_limited();
    }
    if state.config.deployment_mode != DeploymentMode::Community {
        return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
    }
    let Some(expected_token) = &state.config.bootstrap_token else {
        return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
    };
    let supplied_token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if !supplied_token
        .is_some_and(|value| constant_time_secret_eq(value, expected_token.expose_secret()))
    {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            code: "invalid_admin_token",
        });
    }
    if !valid_local_password(&body.password) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_password",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let tenant_id = match select_bootstrap_tenant(database).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return HttpResponse::Conflict().json(ErrorResponse {
                code: "bootstrap_required",
            });
        }
        Err(()) => return database_unavailable(),
    };
    let Ok(hash) = hash_owner_password(&state, &body.password).await else {
        return database_unavailable();
    };
    let Ok(changed) = set_owner_password_hash(database, tenant_id, &hash).await else {
        return database_unavailable();
    };
    if !changed {
        return HttpResponse::Conflict().json(ErrorResponse {
            code: "owner_not_found",
        });
    }
    let session = IssuedSession::issue(tenant_id);
    let expires_at = Utc::now() + ChronoDuration::hours(12);
    let identity = match insert_local_owner_session(
        database,
        tenant_id,
        &session,
        expires_at,
        "session.password_setup",
    )
    .await
    {
        Ok(Some(identity)) => identity,
        Ok(None) => {
            return HttpResponse::Conflict().json(ErrorResponse {
                code: "owner_not_found",
            });
        }
        Err(()) => return database_unavailable(),
    };
    authenticated_session_response(&state, &session, identity, expires_at)
}

async fn set_owner_password_hash(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    hash: &str,
) -> Result<bool, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    sqlx::query("SELECT set_config('meerkateer.tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
    let user_id = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT users.id FROM users JOIN memberships ON memberships.user_id = users.id \
         WHERE memberships.tenant_id = $1 AND memberships.role = 'owner' \
           AND users.disabled_at IS NULL ORDER BY memberships.created_at, users.id LIMIT 1",
    )
    .bind(tenant_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ())?;
    let Some(user_id) = user_id else {
        return Ok(false);
    };
    sqlx::query("UPDATE users SET local_password_hash = $1 WHERE id = $2")
        .bind(hash)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
    sqlx::query(
        "INSERT INTO audit_events (tenant_id, actor_type, actor_id, action, target_type, target_id) \
         VALUES ($1, 'user', $2, 'user.local_password_set', 'user', $2)",
    )
    .bind(tenant_id).bind(user_id).execute(&mut *transaction).await.map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(true)
}

async fn select_bootstrap_tenant(database: &PgPool) -> Result<Option<uuid::Uuid>, ()> {
    sqlx::query_scalar::<_, Option<uuid::Uuid>>(
        "SELECT bootstrap_tenant_id FROM system_state WHERE singleton_id = 1",
    )
    .fetch_one(database)
    .await
    .map_err(|_| ())
}

async fn insert_local_owner_session(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    session: &IssuedSession,
    expires_at: chrono::DateTime<Utc>,
    audit_action: &'static str,
) -> Result<Option<(uuid::Uuid, String, String)>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    sqlx::query("SELECT set_config('meerkateer.tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
    let identity = sqlx::query_as::<_, (uuid::Uuid, String, String)>(
        "SELECT users.id, users.email, users.display_name FROM users \
         JOIN memberships ON memberships.user_id = users.id \
         WHERE memberships.tenant_id = $1 AND memberships.role = 'owner' \
           AND users.disabled_at IS NULL \
         ORDER BY memberships.created_at, users.id LIMIT 1",
    )
    .bind(tenant_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ())?;
    let Some((user_id, _, _)) = identity.as_ref() else {
        transaction.rollback().await.map_err(|_| ())?;
        return Ok(None);
    };
    sqlx::query(
        "INSERT INTO sessions \
         (id, tenant_id, user_id, token_digest, csrf_digest, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(session.session_id)
    .bind(tenant_id)
    .bind(user_id)
    .bind(session.digest.as_slice())
    .bind(session.csrf_digest.as_slice())
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ())?;
    sqlx::query(
        "INSERT INTO audit_events \
         (tenant_id, actor_type, actor_id, action, target_type, target_id) \
         VALUES ($1, 'user', $2, $4, 'session', $3)",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(session.session_id)
    .bind(audit_action)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(identity)
}

fn authenticated_session_response(
    state: &AppState,
    session: &IssuedSession,
    identity: (uuid::Uuid, String, String),
    expires_at: chrono::DateTime<Utc>,
) -> HttpResponse {
    let max_age = (expires_at - Utc::now()).num_seconds().max(1);
    let session_cookie = Cookie::build(
        "meerkateer_session",
        session.secret.expose_secret().to_owned(),
    )
    .http_only(true)
    .secure(state.config.environment == "production")
    .same_site(SameSite::Lax)
    .path("/")
    .max_age(CookieDuration::seconds(max_age))
    .finish();
    let csrf_cookie = Cookie::build(
        "meerkateer_csrf",
        session.csrf_secret.expose_secret().to_owned(),
    )
    .http_only(false)
    .secure(state.config.environment == "production")
    .same_site(SameSite::Lax)
    .path("/")
    .max_age(CookieDuration::seconds(max_age))
    .finish();
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(session_cookie)
        .cookie(csrf_cookie)
        .json(SessionResponse {
            tenant_id: session.tenant_id,
            user_id: identity.0,
            role: "owner".to_owned(),
            email: identity.1,
            display_name: identity.2,
        })
}

async fn authenticate_session(
    request: &HttpRequest,
    state: &AppState,
) -> Result<AuthenticatedSession, AuthenticationFailure> {
    require_rate_limit(request, state, RateLimitScope::Authentication).await?;
    let Some(cookie) = request.cookie("meerkateer_session") else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let presented = secrecy::SecretString::from(cookie.value().to_owned());
    let Some(tenant_id) = IssuedSession::presented_tenant(&presented) else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let Some(database) = &state.database else {
        return Err(AuthenticationFailure::Database);
    };
    let digest = IssuedSession::digest(&presented);
    let Some((session_id, user_id, role, email, display_name, csrf_digest)) =
        find_session(database, tenant_id, &digest)
            .await
            .map_err(|()| AuthenticationFailure::Database)?
    else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let role = role
        .parse::<Role>()
        .map_err(|_| AuthenticationFailure::Unauthorized)?;
    let csrf_digest = csrf_digest
        .try_into()
        .map_err(|_| AuthenticationFailure::Unauthorized)?;
    Ok(AuthenticatedSession {
        session_id,
        principal: Principal {
            user_id,
            tenant_id,
            role,
        },
        csrf_digest,
        email,
        display_name,
    })
}

fn unauthorized() -> HttpResponse {
    HttpResponse::Unauthorized().json(ErrorResponse {
        code: "authentication_required",
    })
}

async fn require_rate_limit(
    request: &HttpRequest,
    state: &AppState,
    scope: RateLimitScope,
) -> Result<(), AuthenticationFailure> {
    let address = request.peer_addr().map_or(
        std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
        |peer| peer.ip(),
    );
    let limit = match scope {
        RateLimitScope::Authentication => state.config.authentication_rate_limit_per_minute,
        RateLimitScope::PasswordLogin | RateLimitScope::GameProbe => 10,
        RateLimitScope::Ingestion => state.config.ingestion_rate_limit_per_minute,
    };
    if state.rate_limiter.allow(scope, address, limit).await {
        Ok(())
    } else {
        Err(AuthenticationFailure::RateLimited)
    }
}

fn rate_limited() -> HttpResponse {
    HttpResponse::TooManyRequests()
        .insert_header((header::RETRY_AFTER, "60"))
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(ErrorResponse {
            code: "rate_limited",
        })
}

async fn find_session(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    digest: &[u8; 32],
) -> Result<Option<(uuid::Uuid, uuid::Uuid, String, String, String, Vec<u8>)>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    sqlx::query("SELECT set_config('meerkateer.tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
    let session = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, String, String, String, Vec<u8>)>(
        "SELECT sessions.id, sessions.user_id, memberships.role, users.email, users.display_name, \
           sessions.csrf_digest \
         FROM sessions \
         JOIN memberships ON memberships.tenant_id = sessions.tenant_id \
           AND memberships.user_id = sessions.user_id \
         JOIN users ON users.id = sessions.user_id \
         WHERE sessions.tenant_id = $1 AND sessions.token_digest = $2 \
           AND sessions.revoked_at IS NULL AND sessions.expires_at > now() \
           AND users.disabled_at IS NULL",
    )
    .bind(tenant_id)
    .bind(digest.as_slice())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(session)
}

fn csrf_is_valid(request: &HttpRequest, session: &AuthenticatedSession) -> bool {
    let Some(cookie) = request.cookie("meerkateer_csrf") else {
        return false;
    };
    let Some(header) = request
        .headers()
        .get("X-Meerkateer-CSRF")
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    if cookie.value().len() > 128 || !constant_time_secret_eq(cookie.value(), header) {
        return false;
    }
    let presented = secrecy::SecretString::from(header.to_owned());
    let digest = IssuedSession::digest(&presented);
    bool::from(digest.ct_eq(&session.csrf_digest))
}

fn authentication_error(failure: AuthenticationFailure) -> HttpResponse {
    match failure {
        AuthenticationFailure::Unauthorized => unauthorized(),
        AuthenticationFailure::RateLimited => rate_limited(),
        AuthenticationFailure::Database => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InventoryWriteError {
    NotFound,
    Conflict,
    Database,
}

fn inventory_error(failure: InventoryWriteError) -> HttpResponse {
    match failure {
        InventoryWriteError::NotFound => {
            HttpResponse::NotFound().json(ErrorResponse { code: "not_found" })
        }
        InventoryWriteError::Conflict => HttpResponse::Conflict().json(ErrorResponse {
            code: "already_exists",
        }),
        InventoryWriteError::Database => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
    }
}

async fn list_projects(request: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    match select_projects(database, session.principal.tenant_id).await {
        Ok(items) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(ProjectListResponse { items }),
        Err(()) => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
    }
}

async fn create_project(
    request: HttpRequest,
    body: web::Json<CreateProjectRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::ServiceWrite)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    if !valid_project_slug(&body.slug) || !valid_display_name(&body.display_name) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    match insert_project(database, session.principal, &body).await {
        Ok(project) => HttpResponse::Created().json(project),
        Err(failure) => inventory_error(failure),
    }
}

async fn list_services(
    request: HttpRequest,
    project_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    match select_services(
        database,
        session.principal.tenant_id,
        project_id.into_inner(),
        state.config.heartbeat_stale_after_seconds,
    )
    .await
    {
        Ok(Some(items)) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(ServiceListResponse { items }),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse { code: "not_found" }),
        Err(()) => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
    }
}

async fn create_service(
    request: HttpRequest,
    project_id: web::Path<uuid::Uuid>,
    body: web::Json<CreateServiceRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::ServiceWrite)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    if !valid_service_slug(&body.slug)
        || !valid_environment(&body.environment)
        || body.game.as_ref().is_some_and(|game| {
            game.kind != "minecraft_java"
                || game.port == 0
                || !minecraft_probe::valid_host(&game.host)
        })
    {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    match insert_service(database, session.principal, project_id.into_inner(), &body).await {
        Ok(service) => HttpResponse::Created().json(service),
        Err(failure) => inventory_error(failure),
    }
}

async fn get_service_timeline(
    request: HttpRequest,
    service_id: web::Path<uuid::Uuid>,
    query: web::Query<TimelineQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    match select_service_timeline(
        database,
        session.principal.tenant_id,
        service_id.into_inner(),
        i64::from(limit),
    )
    .await
    {
        Ok(Some(items)) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(TimelineResponse { items }),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse { code: "not_found" }),
        Err(()) => database_unavailable(),
    }
}

async fn list_incidents(
    request: HttpRequest,
    query: web::Query<ProjectListQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let rows = sqlx::query_as::<_, IncidentRow>(
        "SELECT incidents.id, incidents.project_id, incidents.service_id, services.slug, \
         incidents.status, incidents.severity, incidents.title, incidents.cause, \
         incidents.started_at, incidents.last_observed_at, incidents.resolved_at, \
         incidents.acknowledged_at, acknowledger.display_name AS acknowledged_by, \
         incidents.assigned_to, assignee.display_name AS assignee \
         FROM incidents JOIN services ON services.tenant_id = incidents.tenant_id \
          AND services.id = incidents.service_id \
         LEFT JOIN users AS acknowledger ON acknowledger.id = incidents.acknowledged_by \
         LEFT JOIN users AS assignee ON assignee.id = incidents.assigned_to \
         WHERE incidents.tenant_id = $1 AND incidents.project_id = $2 \
         ORDER BY (incidents.status = 'open') DESC, incidents.started_at DESC, incidents.id \
         LIMIT $3",
    )
    .bind(session.principal.tenant_id)
    .bind(query.project_id)
    .bind(i64::from(limit))
    .fetch_all(&mut *transaction)
    .await;
    let Ok(rows) = rows else {
        return database_unavailable();
    };
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    let items = rows
        .into_iter()
        .map(|row| IncidentResponse {
            id: row.0,
            project_id: row.1,
            service_id: row.2,
            service: row.3,
            status: row.4,
            severity: row.5,
            title: row.6,
            cause: row.7,
            started_at: row.8,
            last_observed_at: row.9,
            resolved_at: row.10,
            acknowledged_at: row.11,
            acknowledged_by: row.12,
            assigned_to: row.13,
            assignee: row.14,
        })
        .collect();
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(IncidentListResponse { items })
}

async fn list_incident_activity(
    request: HttpRequest,
    query: web::Query<ProjectListQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let limit = query.limit.unwrap_or(200);
    if !(1..=200).contains(&limit) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let rows = sqlx::query_as::<
        _,
        (
            uuid::Uuid,
            uuid::Uuid,
            String,
            String,
            Option<String>,
            chrono::DateTime<Utc>,
        ),
    >(
        "SELECT activity.id, activity.incident_id, activity.kind, users.display_name AS actor, \
         activity.note, activity.created_at FROM incident_activity AS activity \
         JOIN incidents ON incidents.tenant_id = activity.tenant_id \
          AND incidents.id = activity.incident_id \
         JOIN users ON users.id = activity.actor_id \
         WHERE activity.tenant_id = $1 AND incidents.project_id = $2 \
         ORDER BY activity.created_at DESC, activity.id LIMIT $3",
    )
    .bind(session.principal.tenant_id)
    .bind(query.project_id)
    .bind(i64::from(limit))
    .fetch_all(&mut *transaction)
    .await;
    let Ok(rows) = rows else {
        return database_unavailable();
    };
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    let items = rows
        .into_iter()
        .map(|row| IncidentActivityResponse {
            id: row.0,
            incident_id: row.1,
            kind: row.2,
            actor: row.3,
            note: row.4,
            created_at: row.5,
        })
        .collect();
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(IncidentActivityListResponse { items })
}

async fn acknowledge_incident(
    request: HttpRequest,
    incident_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::IncidentWrite)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let incident_id = incident_id.into_inner();
    let changed = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE incidents SET acknowledged_at = now(), acknowledged_by = $3, updated_at = now() \
         WHERE tenant_id = $1 AND id = $2 AND acknowledged_at IS NULL RETURNING id",
    )
    .bind(session.principal.tenant_id)
    .bind(incident_id)
    .bind(session.principal.user_id)
    .fetch_optional(&mut *transaction)
    .await;
    let Ok(changed) = changed else {
        return database_unavailable();
    };
    if changed.is_some() {
        if record_incident_activity(
            &mut transaction,
            session.principal,
            incident_id,
            "acknowledged",
            None,
        )
        .await
        .is_err()
            || insert_inventory_audit(
                &mut transaction,
                session.principal,
                "incident.acknowledge",
                "incident",
                incident_id,
            )
            .await
            .is_err()
        {
            return database_unavailable();
        }
    } else {
        match incident_exists(&mut transaction, session.principal.tenant_id, incident_id).await {
            Ok(true) => {}
            Ok(false) => {
                return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
            }
            Err(()) => return database_unavailable(),
        }
    }
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    HttpResponse::NoContent().finish()
}

async fn update_incident_assignment(
    request: HttpRequest,
    incident_id: web::Path<uuid::Uuid>,
    body: web::Json<UpdateIncidentAssignmentRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::IncidentWrite)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let incident_id = incident_id.into_inner();
    let assignee = body.assigned.then_some(session.principal.user_id);
    let changed = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE incidents SET assigned_to = $3, updated_at = now() \
         WHERE tenant_id = $1 AND id = $2 AND assigned_to IS DISTINCT FROM $3 RETURNING id",
    )
    .bind(session.principal.tenant_id)
    .bind(incident_id)
    .bind(assignee)
    .fetch_optional(&mut *transaction)
    .await;
    let Ok(changed) = changed else {
        return database_unavailable();
    };
    if changed.is_some() {
        let kind = if body.assigned {
            "assigned"
        } else {
            "unassigned"
        };
        if record_incident_activity(&mut transaction, session.principal, incident_id, kind, None)
            .await
            .is_err()
            || insert_inventory_audit(
                &mut transaction,
                session.principal,
                if body.assigned {
                    "incident.assign"
                } else {
                    "incident.unassign"
                },
                "incident",
                incident_id,
            )
            .await
            .is_err()
        {
            return database_unavailable();
        }
    } else {
        match incident_exists(&mut transaction, session.principal.tenant_id, incident_id).await {
            Ok(true) => {}
            Ok(false) => {
                return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
            }
            Err(()) => return database_unavailable(),
        }
    }
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    HttpResponse::NoContent().finish()
}

async fn add_incident_note(
    request: HttpRequest,
    incident_id: web::Path<uuid::Uuid>,
    body: web::Json<AddIncidentNoteRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::IncidentWrite)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let note = body.note.trim();
    if note.is_empty() || note.chars().count() > 2000 {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let incident_id = incident_id.into_inner();
    match incident_exists(&mut transaction, session.principal.tenant_id, incident_id).await {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
        }
        Err(()) => return database_unavailable(),
    }
    if record_incident_activity(
        &mut transaction,
        session.principal,
        incident_id,
        "note",
        Some(note),
    )
    .await
    .is_err()
        || insert_inventory_audit(
            &mut transaction,
            session.principal,
            "incident.note",
            "incident",
            incident_id,
        )
        .await
        .is_err()
    {
        return database_unavailable();
    }
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    HttpResponse::NoContent().finish()
}

async fn incident_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: uuid::Uuid,
    incident_id: uuid::Uuid,
) -> Result<bool, ()> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM incidents WHERE tenant_id = $1 AND id = $2)",
    )
    .bind(tenant_id)
    .bind(incident_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| ())
}

async fn record_incident_activity(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    principal: Principal,
    incident_id: uuid::Uuid,
    kind: &'static str,
    note: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO incident_activity (tenant_id, incident_id, kind, actor_id, note) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(principal.tenant_id)
    .bind(incident_id)
    .bind(kind)
    .bind(principal.user_id)
    .bind(note)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn list_alert_deliveries(
    request: HttpRequest,
    query: web::Query<ProjectListQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let rows = sqlx::query_as::<_, (
        uuid::Uuid, uuid::Uuid, uuid::Uuid, String, Option<uuid::Uuid>, Option<uuid::Uuid>, String, String,
        chrono::DateTime<Utc>, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>, i32,
        Option<String>, Option<String>,
    )>(
        "SELECT deliveries.id, deliveries.project_id, deliveries.service_id, services.slug, \
         deliveries.incident_id, deliveries.replay_of, deliveries.transition, deliveries.status, deliveries.observed_at, \
         deliveries.created_at, deliveries.delivered_at, deliveries.attempts, \
         deliveries.last_error, deliveries.suppression_reason \
         FROM alert_deliveries AS deliveries \
         JOIN services ON services.tenant_id = deliveries.tenant_id AND services.id = deliveries.service_id \
         WHERE deliveries.tenant_id = $1 AND deliveries.project_id = $2 \
         ORDER BY deliveries.created_at DESC, deliveries.id LIMIT $3",
    )
    .bind(session.principal.tenant_id)
    .bind(query.project_id)
    .bind(i64::from(limit))
    .fetch_all(&mut *transaction)
    .await;
    let Ok(rows) = rows else {
        return database_unavailable();
    };
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    let items = rows
        .into_iter()
        .map(|row| AlertDeliveryResponse {
            id: row.0,
            project_id: row.1,
            service_id: row.2,
            service: row.3,
            incident_id: row.4,
            replay_of: row.5,
            transition: row.6,
            status: row.7,
            observed_at: row.8,
            created_at: row.9,
            delivered_at: row.10,
            attempts: row.11,
            last_error: row.12,
            suppression_reason: row.13,
        })
        .collect();
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(AlertDeliveryListResponse { items })
}

async fn replay_alert_delivery(
    request: HttpRequest,
    delivery_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::AlertWrite)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    if !alert_webhook_url().is_ok_and(|url| url.is_some()) {
        return HttpResponse::Conflict().json(ErrorResponse {
            code: "alert_webhook_not_configured",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let original_id = delivery_id.into_inner();
    match queue_alert_replay(database, session.principal, original_id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(AlertReplayError::NotFound) => {
            HttpResponse::NotFound().json(ErrorResponse { code: "not_found" })
        }
        Err(AlertReplayError::NotReplayable) => HttpResponse::Conflict().json(ErrorResponse {
            code: "alert_not_replayable",
        }),
        Err(AlertReplayError::Database) => database_unavailable(),
    }
}

#[derive(Debug, Clone, Copy)]
enum AlertReplayError {
    NotFound,
    NotReplayable,
    Database,
}

type AlertReplaySource = (
    uuid::Uuid,
    uuid::Uuid,
    Option<uuid::Uuid>,
    String,
    chrono::DateTime<Utc>,
    String,
    String,
    String,
    String,
);

async fn queue_alert_replay(
    database: &PgPool,
    principal: Principal,
    original_id: uuid::Uuid,
) -> Result<(), AlertReplayError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(|_| AlertReplayError::Database)?;
    set_tenant_context(&mut transaction, principal.tenant_id)
        .await
        .map_err(|_| AlertReplayError::Database)?;
    let original = sqlx::query_as::<_, AlertReplaySource>(
        "SELECT deliveries.project_id, deliveries.service_id, deliveries.incident_id, \
         deliveries.transition, deliveries.observed_at, deliveries.status, projects.slug, \
         services.slug, services.environment FROM alert_deliveries AS deliveries \
         JOIN projects ON projects.tenant_id = deliveries.tenant_id \
          AND projects.id = deliveries.project_id \
         JOIN services ON services.tenant_id = deliveries.tenant_id \
          AND services.id = deliveries.service_id \
         WHERE deliveries.tenant_id = $1 AND deliveries.id = $2",
    )
    .bind(principal.tenant_id)
    .bind(original_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AlertReplayError::Database)?
    .ok_or(AlertReplayError::NotFound)?;
    if original.5 != "dead_lettered" {
        return Err(AlertReplayError::NotReplayable);
    }
    let already_replayed = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM alert_deliveries \
         WHERE tenant_id = $1 AND replay_of = $2)",
    )
    .bind(principal.tenant_id)
    .bind(original_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| AlertReplayError::Database)?;
    if already_replayed {
        return Err(AlertReplayError::NotReplayable);
    }
    insert_alert_replay(&mut transaction, principal, original_id, &original).await?;
    insert_inventory_audit(
        &mut transaction,
        principal,
        "alert.delivery.replay",
        "alert_delivery",
        original_id,
    )
    .await
    .map_err(|_| AlertReplayError::Database)?;
    transaction
        .commit()
        .await
        .map_err(|_| AlertReplayError::Database)
}

async fn insert_alert_replay(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    principal: Principal,
    original_id: uuid::Uuid,
    original: &AlertReplaySource,
) -> Result<(), AlertReplayError> {
    let replay_id = uuid::Uuid::new_v4();
    let outbox_id = uuid::Uuid::new_v4();
    let payload = serde_json::json!({
        "delivery_id": replay_id,
        "replay_of": original_id,
        "service_id": original.1,
        "project": original.6,
        "service": original.7,
        "environment": original.8,
        "transition": original.3,
        "observed_at": original.4,
    });
    sqlx::query(
        "INSERT INTO outbox (id, tenant_id, topic, payload) \
         VALUES ($1, $2, 'alert.transition', $3)",
    )
    .bind(outbox_id)
    .bind(principal.tenant_id)
    .bind(payload)
    .execute(&mut **transaction)
    .await
    .map_err(|_| AlertReplayError::Database)?;
    sqlx::query(
        "INSERT INTO alert_deliveries \
         (tenant_id, id, project_id, service_id, incident_id, replay_of, outbox_id, \
          transition, status, observed_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'queued', $9)",
    )
    .bind(principal.tenant_id)
    .bind(replay_id)
    .bind(original.0)
    .bind(original.1)
    .bind(original.2)
    .bind(original_id)
    .bind(outbox_id)
    .bind(&original.3)
    .bind(original.4)
    .execute(&mut **transaction)
    .await
    .map_err(|_| AlertReplayError::Database)?;
    Ok(())
}

async fn get_alert_policy(request: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let row = sqlx::query_as::<_, (bool, bool, bool, i32, chrono::DateTime<Utc>)>(
        "SELECT enabled, notify_down, notify_recovered, cooldown_seconds, updated_at \
         FROM alert_policies WHERE tenant_id = $1",
    )
    .bind(session.principal.tenant_id)
    .fetch_optional(&mut *transaction)
    .await;
    let Ok(row) = row else {
        return database_unavailable();
    };
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    let (enabled, notify_down, notify_recovered, cooldown_seconds, updated_at) = row
        .map_or((true, true, true, 0, None), |row| {
            (row.0, row.1, row.2, row.3, Some(row.4))
        });
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(AlertPolicyResponse {
            enabled,
            notify_down,
            notify_recovered,
            cooldown_seconds,
            webhook_configured: WebhookConfigured(
                alert_webhook_url().is_ok_and(|url| url.is_some()),
            ),
            updated_at,
        })
}

async fn update_alert_policy(
    request: HttpRequest,
    body: web::Json<UpdateAlertPolicyRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::TenantManage)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    if !(0..=86_400).contains(&body.cooldown_seconds) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let updated_at = Utc::now();
    let result = sqlx::query(
        "INSERT INTO alert_policies (tenant_id, enabled, notify_down, notify_recovered, cooldown_seconds, updated_by, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (tenant_id) DO UPDATE SET \
         enabled = EXCLUDED.enabled, notify_down = EXCLUDED.notify_down, \
         notify_recovered = EXCLUDED.notify_recovered, cooldown_seconds = EXCLUDED.cooldown_seconds, \
         updated_by = EXCLUDED.updated_by, \
         updated_at = EXCLUDED.updated_at",
    ).bind(session.principal.tenant_id).bind(body.enabled).bind(body.notify_down)
        .bind(body.notify_recovered).bind(body.cooldown_seconds)
        .bind(session.principal.user_id).bind(updated_at)
        .execute(&mut *transaction).await;
    if result.is_err() {
        return database_unavailable();
    }
    if insert_inventory_audit(
        &mut transaction,
        session.principal,
        "alert.policy.update",
        "tenant",
        session.principal.tenant_id,
    )
    .await
    .is_err()
    {
        return database_unavailable();
    }
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    HttpResponse::Ok().json(AlertPolicyResponse {
        enabled: body.enabled,
        notify_down: body.notify_down,
        notify_recovered: body.notify_recovered,
        cooldown_seconds: body.cooldown_seconds,
        webhook_configured: WebhookConfigured(alert_webhook_url().is_ok_and(|url| url.is_some())),
        updated_at: Some(updated_at),
    })
}

async fn list_maintenance_windows(
    request: HttpRequest,
    query: web::Query<ProjectListQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let rows = sqlx::query_as::<_, (
        uuid::Uuid, uuid::Uuid, uuid::Uuid, String, String, String, chrono::DateTime<Utc>,
        chrono::DateTime<Utc>, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>,
    )>(
        "SELECT windows.id, windows.project_id, windows.service_id, services.slug, windows.title, \
         windows.reason, windows.starts_at, windows.ends_at, windows.created_at, windows.cancelled_at \
         FROM maintenance_windows AS windows JOIN services \
          ON services.tenant_id = windows.tenant_id AND services.id = windows.service_id \
         WHERE windows.tenant_id = $1 AND windows.project_id = $2 \
         ORDER BY windows.starts_at DESC, windows.id LIMIT $3",
    ).bind(session.principal.tenant_id).bind(query.project_id).bind(i64::from(limit))
        .fetch_all(&mut *transaction).await;
    let Ok(rows) = rows else {
        return database_unavailable();
    };
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    let items = rows
        .into_iter()
        .map(|row| MaintenanceWindowResponse {
            id: row.0,
            project_id: row.1,
            service_id: row.2,
            service: row.3,
            title: row.4,
            reason: row.5,
            starts_at: row.6,
            ends_at: row.7,
            created_at: row.8,
            cancelled_at: row.9,
        })
        .collect();
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(MaintenanceWindowListResponse { items })
}

async fn create_maintenance_window(
    request: HttpRequest,
    body: web::Json<CreateMaintenanceWindowRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::MaintenanceWrite)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let title = body.title.trim();
    let reason = body.reason.trim();
    if title.is_empty()
        || title.len() > 128
        || reason.is_empty()
        || reason.len() > 1024
        || body.ends_at <= body.starts_at
        || body.ends_at > body.starts_at + ChronoDuration::days(90)
        || body.ends_at < Utc::now()
    {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let id = uuid::Uuid::new_v4();
    let created_at = Utc::now();
    let inserted = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "WITH selected AS ( \
             SELECT project_id, id, slug FROM services \
             WHERE tenant_id = $1 AND project_id = $3 AND id = $4 \
         ), inserted AS ( \
             INSERT INTO maintenance_windows \
             (tenant_id, id, project_id, service_id, title, reason, starts_at, ends_at, created_by, created_at) \
             SELECT $1, $2, selected.project_id, selected.id, $5, $6, $7, $8, $9, $10 \
             FROM selected RETURNING id, service_id \
         ) \
         SELECT inserted.id, selected.slug FROM inserted \
         JOIN selected ON selected.id = inserted.service_id",
    ).bind(session.principal.tenant_id).bind(id).bind(body.project_id).bind(body.service_id)
        .bind(title).bind(reason).bind(body.starts_at).bind(body.ends_at)
        .bind(session.principal.user_id).bind(created_at).fetch_optional(&mut *transaction).await;
    let Ok(inserted) = inserted else {
        return database_unavailable();
    };
    let Some((_inserted_id, service_slug)) = inserted else {
        return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
    };
    if insert_inventory_audit(
        &mut transaction,
        session.principal,
        "maintenance.create",
        "maintenance_window",
        id,
    )
    .await
    .is_err()
    {
        return database_unavailable();
    }
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    HttpResponse::Created().json(MaintenanceWindowResponse {
        id,
        project_id: body.project_id,
        service_id: body.service_id,
        service: service_slug,
        title: title.to_owned(),
        reason: reason.to_owned(),
        starts_at: body.starts_at,
        ends_at: body.ends_at,
        created_at,
        cancelled_at: None,
    })
}

async fn cancel_maintenance_window(
    request: HttpRequest,
    window_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::MaintenanceWrite)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let window_id = window_id.into_inner();
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let changed = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE maintenance_windows SET cancelled_at = now() \
         WHERE tenant_id = $1 AND id = $2 AND cancelled_at IS NULL RETURNING id",
    )
    .bind(session.principal.tenant_id)
    .bind(window_id)
    .fetch_optional(&mut *transaction)
    .await;
    let Ok(changed) = changed else {
        return database_unavailable();
    };
    if changed.is_none() {
        return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
    }
    if insert_inventory_audit(
        &mut transaction,
        session.principal,
        "maintenance.cancel",
        "maintenance_window",
        window_id,
    )
    .await
    .is_err()
    {
        return database_unavailable();
    }
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    HttpResponse::NoContent().finish()
}

async fn list_audit_events(
    request: HttpRequest,
    query: web::Query<LimitQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if !matches!(session.principal.role, Role::Owner | Role::Admin) {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let rows = sqlx::query_as::<
        _,
        (
            uuid::Uuid,
            String,
            Option<uuid::Uuid>,
            String,
            String,
            Option<uuid::Uuid>,
            serde_json::Value,
            chrono::DateTime<Utc>,
        ),
    >(
        "SELECT id, actor_type, actor_id, action, target_type, target_id, details, occurred_at \
         FROM audit_events WHERE tenant_id = $1 ORDER BY occurred_at DESC, id LIMIT $2",
    )
    .bind(session.principal.tenant_id)
    .bind(i64::from(limit))
    .fetch_all(&mut *transaction)
    .await;
    let Ok(rows) = rows else {
        return database_unavailable();
    };
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    let items = rows
        .into_iter()
        .map(|row| AuditEventResponse {
            id: row.0,
            actor_type: row.1,
            actor_id: row.2,
            action: row.3,
            target_type: row.4,
            target_id: row.5,
            details: row.6,
            occurred_at: row.7,
        })
        .collect();
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(AuditEventListResponse { items })
}

async fn admin_summary(request: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if !matches!(session.principal.role, Role::Owner | Role::Admin) {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let counts = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64)>(
        "SELECT \
         (SELECT count(*) FROM projects WHERE tenant_id = $1), \
         (SELECT count(*) FROM services WHERE tenant_id = $1), \
         (SELECT count(*) FROM agents WHERE tenant_id = $1), \
         (SELECT count(*) FROM incidents WHERE tenant_id = $1 AND status = 'open'), \
         (SELECT count(*) FROM alert_deliveries WHERE tenant_id = $1 AND status IN ('queued', 'retrying')), \
         (SELECT count(*) FROM alert_deliveries WHERE tenant_id = $1 AND status = 'dead_lettered'), \
         (SELECT count(*) FROM maintenance_windows WHERE tenant_id = $1 AND cancelled_at IS NULL \
          AND starts_at <= now() AND ends_at > now())",
    ).bind(session.principal.tenant_id).fetch_one(&mut *transaction).await;
    let Ok(counts) = counts else {
        return database_unavailable();
    };
    let oldest_pending_alert_at = sqlx::query_scalar::<_, Option<chrono::DateTime<Utc>>>(
        "SELECT min(created_at) FROM alert_deliveries \
         WHERE tenant_id = $1 AND status IN ('queued', 'retrying')",
    )
    .bind(session.principal.tenant_id)
    .fetch_one(&mut *transaction)
    .await;
    let Ok(oldest_pending_alert_at) = oldest_pending_alert_at else {
        return database_unavailable();
    };
    let worker = sqlx::query_as::<
        _,
        (
            uuid::Uuid,
            chrono::DateTime<Utc>,
            chrono::DateTime<Utc>,
            i32,
            i32,
            i32,
            i32,
        ),
    >(
        "SELECT worker_id, started_at, last_cycle_at, claimed, completed, retried, dead_lettered \
         FROM worker_runtime ORDER BY last_cycle_at DESC LIMIT 1",
    )
    .fetch_optional(&mut *transaction)
    .await;
    let Ok(worker) = worker else {
        return database_unavailable();
    };
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    let worker_status = worker_runtime_status(worker.as_ref().map(|row| row.2), Utc::now());
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(AdminSummaryResponse {
            tenant_id: session.principal.tenant_id,
            deployment_mode: state.config.deployment_mode.as_str(),
            projects: counts.0,
            services: counts.1,
            agents: counts.2,
            open_incidents: counts.3,
            pending_alerts: counts.4,
            dead_lettered_alerts: counts.5,
            active_maintenance_windows: counts.6,
            oldest_pending_alert_at,
            worker_status,
            worker_started_at: worker.as_ref().map(|row| row.1),
            worker_last_cycle_at: worker.as_ref().map(|row| row.2),
            worker_last_cycle_claimed: worker.as_ref().map_or(0, |row| row.3),
            worker_last_cycle_completed: worker.as_ref().map_or(0, |row| row.4),
            worker_last_cycle_retried: worker.as_ref().map_or(0, |row| row.5),
            worker_last_cycle_dead_lettered: worker.as_ref().map_or(0, |row| row.6),
        })
}

fn worker_runtime_status(
    last_cycle_at: Option<chrono::DateTime<Utc>>,
    current_time: chrono::DateTime<Utc>,
) -> &'static str {
    match last_cycle_at {
        Some(last_cycle)
            if last_cycle >= current_time - ChronoDuration::seconds(30)
                && last_cycle <= current_time + ChronoDuration::minutes(5) =>
        {
            "healthy"
        }
        Some(_) => "stalled",
        None => "never_seen",
    }
}

async fn test_game_probe(
    request: HttpRequest,
    service_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    if state.config.deployment_mode != DeploymentMode::Community {
        return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
    }
    if require_rate_limit(&request, &state, RateLimitScope::GameProbe)
        .await
        .is_err()
    {
        return rate_limited();
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let Ok(mut transaction) = database.begin().await else {
        return database_unavailable();
    };
    if set_tenant_context(&mut transaction, session.principal.tenant_id)
        .await
        .is_err()
    {
        return database_unavailable();
    }
    let config = sqlx::query_as::<_, (Option<String>, Option<i32>)>(
        "SELECT game_host, game_port FROM services \
         WHERE tenant_id = $1 AND id = $2 AND game_kind = 'minecraft_java'",
    )
    .bind(session.principal.tenant_id)
    .bind(service_id.into_inner())
    .fetch_optional(&mut *transaction)
    .await;
    if transaction.commit().await.is_err() {
        return database_unavailable();
    }
    let (Some(host), Some(port)) = (match config {
        Ok(Some(config)) => config,
        Ok(None) => return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" }),
        Err(_) => return database_unavailable(),
    }) else {
        return HttpResponse::Conflict().json(ErrorResponse {
            code: "game_config_incomplete",
        });
    };
    let Ok(port) = u16::try_from(port) else {
        return HttpResponse::Conflict().json(ErrorResponse {
            code: "game_config_incomplete",
        });
    };
    match minecraft_probe::probe(&host, port).await {
        Ok(result) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(result),
        Err(minecraft_probe::Failure::UnsafeDestination) => {
            HttpResponse::BadRequest().json(ErrorResponse {
                code: "unsafe_probe_destination",
            })
        }
        Err(failure) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(minecraft_probe::ResultData::failed(&failure)),
    }
}

async fn select_projects(
    database: &PgPool,
    tenant_id: uuid::Uuid,
) -> Result<Vec<ProjectResponse>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| ())?;
    let rows = sqlx::query_as::<_, (uuid::Uuid, String, String, chrono::DateTime<Utc>)>(
        "SELECT id, slug, display_name, created_at FROM projects \
         WHERE tenant_id = $1 ORDER BY created_at, id LIMIT 200",
    )
    .bind(tenant_id)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(rows
        .into_iter()
        .map(|(id, slug, display_name, created_at)| ProjectResponse {
            id,
            slug,
            display_name,
            created_at,
        })
        .collect())
}

async fn insert_project(
    database: &PgPool,
    principal: Principal,
    body: &CreateProjectRequest,
) -> Result<ProjectResponse, InventoryWriteError> {
    let mut transaction = begin_inventory_transaction(database, principal.tenant_id).await?;
    let id = uuid::Uuid::new_v4();
    let created_at = Utc::now();
    let result = sqlx::query(
        "INSERT INTO projects (tenant_id, id, slug, display_name, created_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(principal.tenant_id)
    .bind(id)
    .bind(&body.slug)
    .bind(&body.display_name)
    .bind(created_at)
    .execute(&mut *transaction)
    .await;
    if let Err(error) = result {
        return Err(map_inventory_database_error(&error));
    }
    insert_inventory_audit(&mut transaction, principal, "project.create", "project", id).await?;
    transaction
        .commit()
        .await
        .map_err(|_| InventoryWriteError::Database)?;
    Ok(ProjectResponse {
        id,
        slug: body.slug.clone(),
        display_name: body.display_name.clone(),
        created_at,
    })
}

type ServiceRow = (
    uuid::Uuid,
    uuid::Uuid,
    String,
    String,
    chrono::DateTime<Utc>,
    Option<String>,
    Option<i64>,
    Option<chrono::DateTime<Utc>>,
    Option<chrono::DateTime<Utc>>,
    Option<String>,
    Option<String>,
    Option<i32>,
);

fn service_from_row(row: ServiceRow, stale_before: chrono::DateTime<Utc>) -> ServiceResponse {
    let (
        id,
        project_id,
        slug,
        environment,
        created_at,
        reported_state,
        last_sequence,
        observed_at,
        updated_at,
        game_kind,
        game_host,
        game_port,
    ) = row;
    let stale = observed_at.is_some_and(|value| value < stale_before);
    let effective_state = if stale {
        "unknown".to_owned()
    } else {
        reported_state
            .clone()
            .unwrap_or_else(|| "unknown".to_owned())
    };
    ServiceResponse {
        id,
        project_id,
        slug,
        environment,
        created_at,
        game: match (game_kind, game_host, game_port) {
            (Some(kind), Some(host), Some(port)) => u16::try_from(port)
                .ok()
                .map(|port| GameConfig { kind, host, port }),
            _ => None,
        },
        status: ServiceStatusResponse {
            state: effective_state,
            reported_state,
            stale,
            last_sequence,
            observed_at,
            updated_at,
        },
    }
}

async fn select_services(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    project_id: uuid::Uuid,
    stale_after_seconds: u32,
) -> Result<Option<Vec<ServiceResponse>>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| ())?;
    let project_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE tenant_id = $1 AND id = $2)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| ())?;
    if !project_exists {
        transaction.rollback().await.map_err(|_| ())?;
        return Ok(None);
    }
    let rows = sqlx::query_as::<_, ServiceRow>(
        "SELECT services.id, services.project_id, services.slug, services.environment, \
           services.created_at, service_snapshots.state, service_snapshots.last_sequence, \
           service_snapshots.observed_at, service_snapshots.updated_at, \
           services.game_kind, services.game_host, services.game_port \
         FROM services LEFT JOIN service_snapshots \
           ON service_snapshots.tenant_id = services.tenant_id \
          AND service_snapshots.service_id = services.id \
         WHERE services.tenant_id = $1 AND services.project_id = $2 \
         ORDER BY services.created_at, services.id LIMIT 500",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    let stale_before = Utc::now() - ChronoDuration::seconds(i64::from(stale_after_seconds));
    Ok(Some(
        rows.into_iter()
            .map(|row| service_from_row(row, stale_before))
            .collect(),
    ))
}

async fn insert_service(
    database: &PgPool,
    principal: Principal,
    project_id: uuid::Uuid,
    body: &CreateServiceRequest,
) -> Result<ServiceResponse, InventoryWriteError> {
    let mut transaction = begin_inventory_transaction(database, principal.tenant_id).await?;
    let id = uuid::Uuid::new_v4();
    let created_at = Utc::now();
    let result = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO services (tenant_id, project_id, id, slug, environment, created_at, game_kind, game_host, game_port) \
         SELECT $1, projects.id, $3, $4, $5, $6, $7, $8, $9 FROM projects \
         WHERE projects.tenant_id = $1 AND projects.id = $2 RETURNING id",
    )
    .bind(principal.tenant_id)
    .bind(project_id)
    .bind(id)
    .bind(&body.slug)
    .bind(&body.environment)
    .bind(created_at)
    .bind(body.game.as_ref().map(|game| game.kind.as_str()))
    .bind(body.game.as_ref().map(|game| game.host.as_str()))
    .bind(body.game.as_ref().map(|game| i32::from(game.port)))
    .fetch_optional(&mut *transaction)
    .await;
    let inserted = match result {
        Ok(inserted) => inserted,
        Err(error) => return Err(map_inventory_database_error(&error)),
    };
    if inserted.is_none() {
        transaction
            .rollback()
            .await
            .map_err(|_| InventoryWriteError::Database)?;
        return Err(InventoryWriteError::NotFound);
    }
    insert_inventory_audit(&mut transaction, principal, "service.create", "service", id).await?;
    transaction
        .commit()
        .await
        .map_err(|_| InventoryWriteError::Database)?;
    Ok(ServiceResponse {
        id,
        project_id,
        slug: body.slug.clone(),
        environment: body.environment.clone(),
        created_at,
        game: body.game.clone(),
        status: ServiceStatusResponse::unknown(),
    })
}

async fn select_service_timeline(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    service_id: uuid::Uuid,
    limit: i64,
) -> Result<Option<Vec<TimelineItemResponse>>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| ())?;
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM services WHERE tenant_id = $1 AND id = $2)",
    )
    .bind(tenant_id)
    .bind(service_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| ())?;
    if !exists {
        transaction.rollback().await.map_err(|_| ())?;
        return Ok(None);
    }
    let rows = sqlx::query_as::<
        _,
        (
            uuid::Uuid,
            String,
            serde_json::Value,
            chrono::DateTime<Utc>,
            chrono::DateTime<Utc>,
        ),
    >(
        "SELECT idempotency_key, message_kind, payload, observed_at, received_at \
         FROM ingest_messages WHERE tenant_id = $1 AND service_id = $2 \
         ORDER BY observed_at DESC, received_at DESC, idempotency_key DESC LIMIT $3",
    )
    .bind(tenant_id)
    .bind(service_id)
    .bind(limit)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(Some(
        rows.into_iter()
            .map(
                |(idempotency_key, kind, payload, observed_at, received_at)| {
                    timeline_item(idempotency_key, kind, &payload, observed_at, received_at)
                },
            )
            .collect(),
    ))
}

fn timeline_item(
    idempotency_key: uuid::Uuid,
    kind: String,
    payload: &serde_json::Value,
    observed_at: chrono::DateTime<Utc>,
    received_at: chrono::DateTime<Utc>,
) -> TimelineItemResponse {
    let message = payload
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let (state, severity, title) = match kind.as_str() {
        "heartbeat" => {
            let reported = payload
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let effective = match reported {
                "ok" => "online",
                "degraded" => "degraded",
                "down" => "offline",
                _ => "unknown",
            };
            (
                Some(effective.to_owned()),
                None,
                format!("Service reported {effective}"),
            )
        }
        "event" => {
            let event_kind = payload
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("operational_event");
            let level = payload
                .get("level")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("info");
            (None, Some(level.to_owned()), event_kind.replace('_', " "))
        }
        "deploy" => {
            let version = payload
                .get("version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let status = payload
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            (None, None, format!("Deploy {version} {status}"))
        }
        _ => (None, None, "Operational update".to_owned()),
    };
    TimelineItemResponse {
        idempotency_key,
        kind,
        state,
        severity,
        title,
        message,
        observed_at,
        received_at,
    }
}

async fn begin_inventory_transaction(
    database: &PgPool,
    tenant_id: uuid::Uuid,
) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, InventoryWriteError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(|_| InventoryWriteError::Database)?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| InventoryWriteError::Database)?;
    Ok(transaction)
}

async fn set_tenant_context(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_config('meerkateer.tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn insert_inventory_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    principal: Principal,
    action: &'static str,
    target_type: &'static str,
    target_id: uuid::Uuid,
) -> Result<(), InventoryWriteError> {
    sqlx::query(
        "INSERT INTO audit_events \
         (tenant_id, actor_type, actor_id, action, target_type, target_id) \
         VALUES ($1, 'user', $2, $3, $4, $5)",
    )
    .bind(principal.tenant_id)
    .bind(principal.user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .execute(&mut **transaction)
    .await
    .map_err(|_| InventoryWriteError::Database)?;
    Ok(())
}

fn map_inventory_database_error(error: &sqlx::Error) -> InventoryWriteError {
    if error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "23505")
    {
        InventoryWriteError::Conflict
    } else {
        InventoryWriteError::Database
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentWriteError {
    InvalidToken,
    NotFound,
    Conflict,
    Database,
}

async fn issue_enrollment_token(
    request: HttpRequest,
    body: web::Json<IssueEnrollmentTokenRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    issue_enrollment_token_for_project(request, body.expires_in_seconds, state, None).await
}

async fn issue_project_enrollment_token(
    request: HttpRequest,
    project_id: web::Path<uuid::Uuid>,
    body: web::Json<IssueEnrollmentTokenRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    issue_enrollment_token_for_project(
        request,
        body.expires_in_seconds,
        state,
        Some(project_id.into_inner()),
    )
    .await
}

async fn issue_enrollment_token_for_project(
    request: HttpRequest,
    expires_in_seconds: u32,
    state: web::Data<AppState>,
    project_id: Option<uuid::Uuid>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::AgentManage)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    if !(300..=3_600).contains(&expires_in_seconds) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_expiry",
        });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    let tenant_id = session.principal.tenant_id;
    let Ok(Ok(token)) =
        tokio::task::spawn_blocking(move || IssuedEnrollmentToken::issue(tenant_id)).await
    else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "credential_generation_failed",
        });
    };
    let expires_at = Utc::now() + ChronoDuration::seconds(i64::from(expires_in_seconds));
    match insert_enrollment_token(database, session.principal, &token, project_id, expires_at).await
    {
        Ok(()) => HttpResponse::Created()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(IssuedEnrollmentTokenResponse {
                token_id: token.token_id,
                project_id,
                prefix: token.prefix,
                secret: token.secret.expose_secret().to_owned(),
                expires_at,
            }),
        Err(AgentWriteError::NotFound) => {
            HttpResponse::NotFound().json(ErrorResponse { code: "not_found" })
        }
        Err(AgentWriteError::Database) => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
        Err(_) => HttpResponse::Conflict().json(ErrorResponse {
            code: "credential_conflict",
        }),
    }
}

async fn enroll_agent(
    request: HttpRequest,
    body: web::Json<EnrollAgentRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    if require_rate_limit(&request, &state, RateLimitScope::Authentication)
        .await
        .is_err()
    {
        return rate_limited();
    }
    if !valid_display_name(&body.display_name) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(presented_value) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| value.len() <= 192)
    else {
        return invalid_enrollment_token();
    };
    let Some((tenant_id, prefix)) = IssuedEnrollmentToken::presented_scope(presented_value) else {
        return invalid_enrollment_token();
    };
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    let stored = match find_enrollment_token(database, tenant_id, prefix).await {
        Ok(Some(stored)) => stored,
        Ok(None) => return invalid_enrollment_token(),
        Err(()) => {
            return HttpResponse::ServiceUnavailable().json(ErrorResponse {
                code: "database_unavailable",
            });
        }
    };
    let Ok(permit) = Arc::clone(&state.credential_verifier).acquire_owned().await else {
        return database_unavailable();
    };
    let presented = secrecy::SecretString::from(presented_value.to_owned());
    let token_id = stored.0;
    let password_hash = stored.1;
    let project_id = stored.2;
    let Ok(Ok(())) = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        IssuedEnrollmentToken::verify(&presented, &password_hash)
    })
    .await
    else {
        return invalid_enrollment_token();
    };
    let agent_id = uuid::Uuid::new_v4();
    let Ok(Ok(credential)) =
        tokio::task::spawn_blocking(move || IssuedAgentCredential::issue(tenant_id, agent_id))
            .await
    else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "credential_generation_failed",
        });
    };
    let expires_at = Utc::now() + ChronoDuration::days(90);
    let fingerprint_digest: [u8; 32] = Sha256::digest(body.installation_id.as_bytes()).into();
    match consume_token_and_insert_agent(
        database,
        tenant_id,
        token_id,
        project_id,
        agent_id,
        fingerprint_digest,
        &body.display_name,
        &credential,
        expires_at,
    )
    .await
    {
        Ok(()) => HttpResponse::Created()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(EnrolledAgentResponse {
                agent_id,
                project_id,
                credential_id: credential.credential_id,
                secret: credential.secret.expose_secret().to_owned(),
                expires_at,
            }),
        Err(AgentWriteError::InvalidToken) => invalid_enrollment_token(),
        Err(AgentWriteError::Conflict) => HttpResponse::Conflict().json(ErrorResponse {
            code: "installation_already_enrolled",
        }),
        Err(_) => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
    }
}

async fn list_agents(request: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    match select_agents(
        database,
        session.principal.tenant_id,
        state.config.heartbeat_stale_after_seconds,
    )
    .await
    {
        Ok(items) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(AgentListResponse { items }),
        Err(()) => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
    }
}

async fn get_agent_telemetry(
    request: HttpRequest,
    agent_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    match select_agent_telemetry(
        database,
        session.principal.tenant_id,
        agent_id.into_inner(),
        state.config.heartbeat_stale_after_seconds,
    )
    .await
    {
        Ok(Some(snapshot)) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(snapshot),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse { code: "not_found" }),
        Err(()) => database_unavailable(),
    }
}

async fn list_project_agents(
    request: HttpRequest,
    project_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::Read)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    match select_project_agents(
        database,
        session.principal.tenant_id,
        project_id.into_inner(),
        state.config.heartbeat_stale_after_seconds,
    )
    .await
    {
        Ok(Some(items)) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(AgentListResponse { items }),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse { code: "not_found" }),
        Err(()) => database_unavailable(),
    }
}

async fn assign_project_agent(
    request: HttpRequest,
    path: web::Path<(uuid::Uuid, uuid::Uuid)>,
    state: web::Data<AppState>,
) -> HttpResponse {
    update_project_agent_assignment(request, path.into_inner(), state, true).await
}

async fn unassign_project_agent(
    request: HttpRequest,
    path: web::Path<(uuid::Uuid, uuid::Uuid)>,
    state: web::Data<AppState>,
) -> HttpResponse {
    update_project_agent_assignment(request, path.into_inner(), state, false).await
}

async fn update_project_agent_assignment(
    request: HttpRequest,
    (project_id, agent_id): (uuid::Uuid, uuid::Uuid),
    state: web::Data<AppState>,
    assign: bool,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::AgentManage)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let result = if assign {
        set_project_agent_assignment(database, session.principal, project_id, agent_id).await
    } else {
        remove_project_agent_assignment(database, session.principal, project_id, agent_id).await
    };
    match result {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(failure) => inventory_error(failure),
    }
}

async fn revoke_agent(
    request: HttpRequest,
    agent_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::AgentManage)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    match mark_agent_revoked(database, session.principal, agent_id.into_inner()).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(AgentWriteError::NotFound) => {
            HttpResponse::NotFound().json(ErrorResponse { code: "not_found" })
        }
        Err(_) => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
    }
}

fn invalid_enrollment_token() -> HttpResponse {
    HttpResponse::Unauthorized().json(ErrorResponse {
        code: "invalid_enrollment_token",
    })
}

#[derive(Debug, Clone, Copy)]
struct AuthenticatedAgent {
    tenant: uuid::Uuid,
    agent: uuid::Uuid,
    credential: uuid::Uuid,
}

async fn rotate_agent_credential(
    request: HttpRequest,
    path_agent_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    let agent = match authenticate_agent(&request, &state).await {
        Ok(agent) => agent,
        Err(AuthenticationFailure::Unauthorized) => return agent_unauthorized(),
        Err(AuthenticationFailure::RateLimited) => return rate_limited(),
        Err(AuthenticationFailure::Database) => {
            return HttpResponse::ServiceUnavailable().json(ErrorResponse {
                code: "database_unavailable",
            });
        }
    };
    if agent.agent != path_agent_id.into_inner() {
        return agent_unauthorized();
    }
    let tenant_id = agent.tenant;
    let agent_id = agent.agent;
    let Ok(Ok(replacement)) =
        tokio::task::spawn_blocking(move || IssuedAgentCredential::issue(tenant_id, agent_id))
            .await
    else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "credential_generation_failed",
        });
    };
    let previous_valid_until = Utc::now() + ChronoDuration::minutes(5);
    let expires_at = Utc::now() + ChronoDuration::days(90);
    match replace_agent_credential(
        database,
        agent,
        &replacement,
        previous_valid_until,
        expires_at,
    )
    .await
    {
        Ok(()) => HttpResponse::Created()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(RotatedAgentCredentialResponse {
                credential_id: replacement.credential_id,
                secret: replacement.secret.expose_secret().to_owned(),
                expires_at,
                previous_valid_until,
            }),
        Err(AgentWriteError::InvalidToken | AgentWriteError::NotFound) => agent_unauthorized(),
        Err(_) => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
    }
}

async fn authenticate_agent(
    request: &HttpRequest,
    state: &AppState,
) -> Result<AuthenticatedAgent, AuthenticationFailure> {
    require_rate_limit(request, state, RateLimitScope::Authentication).await?;
    let Some(presented_value) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| value.len() <= 256)
    else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let Some((tenant_id, agent_id, prefix)) =
        IssuedAgentCredential::presented_scope(presented_value)
    else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let Some(database) = &state.database else {
        return Err(AuthenticationFailure::Database);
    };
    let Some((credential_id, password_hash)) =
        find_agent_credential(database, tenant_id, agent_id, prefix)
            .await
            .map_err(|()| AuthenticationFailure::Database)?
    else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let permit = Arc::clone(&state.credential_verifier)
        .acquire_owned()
        .await
        .map_err(|_| AuthenticationFailure::Database)?;
    let presented = secrecy::SecretString::from(presented_value.to_owned());
    let verified = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        IssuedAgentCredential::verify(&presented, &password_hash)
    })
    .await
    .map_err(|_| AuthenticationFailure::Database)?;
    verified.map_err(|_| AuthenticationFailure::Unauthorized)?;
    Ok(AuthenticatedAgent {
        tenant: tenant_id,
        agent: agent_id,
        credential: credential_id,
    })
}

fn agent_unauthorized() -> HttpResponse {
    HttpResponse::Unauthorized().json(ErrorResponse {
        code: "unauthorized",
    })
}

async fn find_agent_credential(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    agent_id: uuid::Uuid,
    prefix: &str,
) -> Result<Option<(uuid::Uuid, String)>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| ())?;
    let result = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "SELECT agent_credentials.id, agent_credentials.password_hash \
         FROM agent_credentials JOIN agents \
           ON agents.tenant_id = agent_credentials.tenant_id \
          AND agents.id = agent_credentials.agent_id \
         WHERE agent_credentials.tenant_id = $1 AND agent_credentials.agent_id = $2 \
           AND agent_credentials.prefix = $3 AND agent_credentials.revoked_at IS NULL \
           AND agent_credentials.valid_after <= now() AND agent_credentials.expires_at > now() \
           AND agents.status = 'active'",
    )
    .bind(tenant_id)
    .bind(agent_id)
    .bind(prefix)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(result)
}

async fn replace_agent_credential(
    database: &PgPool,
    agent: AuthenticatedAgent,
    replacement: &IssuedAgentCredential,
    previous_valid_until: chrono::DateTime<Utc>,
    expires_at: chrono::DateTime<Utc>,
) -> Result<(), AgentWriteError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(|_| AgentWriteError::Database)?;
    set_tenant_context(&mut transaction, agent.tenant)
        .await
        .map_err(|_| AgentWriteError::Database)?;
    let previous = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE agent_credentials SET \
           expires_at = CASE WHEN expires_at > $4 THEN $4 ELSE expires_at END, \
           last_used_at = now() \
         WHERE tenant_id = $1 AND agent_id = $2 AND id = $3 \
           AND revoked_at IS NULL AND valid_after <= now() AND expires_at > now() \
         RETURNING id",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .bind(agent.credential)
    .bind(previous_valid_until)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    if previous.is_none() {
        transaction
            .rollback()
            .await
            .map_err(|_| AgentWriteError::Database)?;
        return Err(AgentWriteError::InvalidToken);
    }
    sqlx::query(
        "INSERT INTO agent_credentials \
         (tenant_id, agent_id, id, prefix, password_hash, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .bind(replacement.credential_id)
    .bind(&replacement.prefix)
    .bind(&replacement.password_hash)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    sqlx::query(
        "UPDATE agents SET last_seen_at = now() \
         WHERE tenant_id = $1 AND id = $2 AND status = 'active'",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    sqlx::query(
        "INSERT INTO audit_events \
         (tenant_id, actor_type, actor_id, action, target_type, target_id) \
         VALUES ($1, 'agent', $2, 'agent_credential.rotate', 'agent_credential', $3)",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .bind(replacement.credential_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    transaction
        .commit()
        .await
        .map_err(|_| AgentWriteError::Database)
}

async fn insert_enrollment_token(
    database: &PgPool,
    principal: Principal,
    token: &IssuedEnrollmentToken,
    project_id: Option<uuid::Uuid>,
    expires_at: chrono::DateTime<Utc>,
) -> Result<(), AgentWriteError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(|_| AgentWriteError::Database)?;
    set_tenant_context(&mut transaction, principal.tenant_id)
        .await
        .map_err(|_| AgentWriteError::Database)?;
    let inserted = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO enrollment_tokens \
         (tenant_id, id, project_id, prefix, password_hash, expires_at, created_by) \
         SELECT $1, $2, $3, $4, $5, $6, $7 \
         WHERE $3::uuid IS NULL OR EXISTS (\
           SELECT 1 FROM projects WHERE tenant_id = $1 AND id = $3\
         ) RETURNING id",
    )
    .bind(principal.tenant_id)
    .bind(token.token_id)
    .bind(project_id)
    .bind(&token.prefix)
    .bind(&token.password_hash)
    .bind(expires_at)
    .bind(principal.user_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    if inserted.is_none() {
        transaction
            .rollback()
            .await
            .map_err(|_| AgentWriteError::Database)?;
        return Err(AgentWriteError::NotFound);
    }
    sqlx::query(
        "INSERT INTO audit_events \
         (tenant_id, actor_type, actor_id, action, target_type, target_id) \
         VALUES ($1, 'user', $2, 'agent.enrollment_token.issue', 'enrollment_token', $3)",
    )
    .bind(principal.tenant_id)
    .bind(principal.user_id)
    .bind(token.token_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    transaction
        .commit()
        .await
        .map_err(|_| AgentWriteError::Database)
}

async fn find_enrollment_token(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    prefix: &str,
) -> Result<Option<(uuid::Uuid, String, Option<uuid::Uuid>)>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| ())?;
    let result = sqlx::query_as::<_, (uuid::Uuid, String, Option<uuid::Uuid>)>(
        "SELECT id, password_hash, project_id FROM enrollment_tokens \
         WHERE tenant_id = $1 AND prefix = $2 AND consumed_at IS NULL AND expires_at > now()",
    )
    .bind(tenant_id)
    .bind(prefix)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
async fn consume_token_and_insert_agent(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    token_id: uuid::Uuid,
    project_id: Option<uuid::Uuid>,
    agent_id: uuid::Uuid,
    fingerprint_digest: [u8; 32],
    display_name: &str,
    credential: &IssuedAgentCredential,
    expires_at: chrono::DateTime<Utc>,
) -> Result<(), AgentWriteError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(|_| AgentWriteError::Database)?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| AgentWriteError::Database)?;
    let consumed = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE enrollment_tokens SET consumed_at = now() \
         WHERE tenant_id = $1 AND id = $2 AND project_id IS NOT DISTINCT FROM $3 \
           AND consumed_at IS NULL AND expires_at > now() \
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(token_id)
    .bind(project_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    if consumed.is_none() {
        transaction
            .rollback()
            .await
            .map_err(|_| AgentWriteError::Database)?;
        return Err(AgentWriteError::InvalidToken);
    }
    let inserted = sqlx::query(
        "INSERT INTO agents \
         (tenant_id, id, machine_fingerprint_digest, display_name, status) \
         VALUES ($1, $2, $3, $4, 'active')",
    )
    .bind(tenant_id)
    .bind(agent_id)
    .bind(fingerprint_digest.as_slice())
    .bind(display_name)
    .execute(&mut *transaction)
    .await;
    if let Err(error) = inserted {
        if error
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .is_some_and(|code| code == "23505")
        {
            return Err(AgentWriteError::Conflict);
        }
        return Err(AgentWriteError::Database);
    }
    sqlx::query(
        "INSERT INTO agent_credentials \
         (tenant_id, agent_id, id, prefix, password_hash, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(tenant_id)
    .bind(agent_id)
    .bind(credential.credential_id)
    .bind(&credential.prefix)
    .bind(&credential.password_hash)
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    if let Some(project_id) = project_id {
        sqlx::query(
            "INSERT INTO project_agents (tenant_id, project_id, agent_id) \
             VALUES ($1, $2, $3)",
        )
        .bind(tenant_id)
        .bind(project_id)
        .bind(agent_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| AgentWriteError::Database)?;
    }
    sqlx::query(
        "INSERT INTO audit_events \
         (tenant_id, actor_type, actor_id, action, target_type, target_id) \
         VALUES ($1, 'agent', $2, 'agent.enroll', 'agent', $2)",
    )
    .bind(tenant_id)
    .bind(agent_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    transaction
        .commit()
        .await
        .map_err(|_| AgentWriteError::Database)
}

type AgentRow = (
    uuid::Uuid,
    String,
    String,
    chrono::DateTime<Utc>,
    Option<chrono::DateTime<Utc>>,
);

type AgentTelemetryHeaderRow = (
    String,
    Option<chrono::DateTime<Utc>>,
    Option<i64>,
    Option<i64>,
    Option<chrono::DateTime<Utc>>,
);

type AgentTelemetryMetricRow = (String, f64, chrono::DateTime<Utc>, serde_json::Value);

async fn select_agents(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    stale_after_seconds: u32,
) -> Result<Vec<AgentResponse>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| ())?;
    let rows = sqlx::query_as::<_, AgentRow>(
        "SELECT id, display_name, status, enrolled_at, last_seen_at FROM agents \
         WHERE tenant_id = $1 ORDER BY enrolled_at, id LIMIT 500",
    )
    .bind(tenant_id)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(agent_responses(rows, stale_after_seconds))
}

async fn select_agent_telemetry(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    agent_id: uuid::Uuid,
    stale_after_seconds: u32,
) -> Result<Option<AgentTelemetrySnapshotResponse>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| ())?;
    let header = sqlx::query_as::<_, AgentTelemetryHeaderRow>(
        "SELECT agents.status, agents.last_seen_at, latest.first_sequence, \
           latest.last_sequence, latest.received_at FROM agents LEFT JOIN LATERAL ( \
             SELECT first_sequence, last_sequence, received_at FROM agent_batches \
              WHERE tenant_id = agents.tenant_id AND agent_id = agents.id \
              ORDER BY last_sequence DESC, received_at DESC LIMIT 1 \
           ) latest ON true WHERE agents.tenant_id = $1 AND agents.id = $2",
    )
    .bind(tenant_id)
    .bind(agent_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ())?;
    let Some((status, last_seen_at, first_sequence, last_sequence, received_at)) = header else {
        transaction.rollback().await.map_err(|_| ())?;
        return Ok(None);
    };
    let rows = if let (Some(first_sequence), Some(last_sequence)) = (first_sequence, last_sequence)
    {
        sqlx::query_as::<_, AgentTelemetryMetricRow>(
            "SELECT name, value, observed_at, attributes FROM agent_telemetry_records \
             WHERE tenant_id = $1 AND agent_id = $2 \
               AND sequence BETWEEN $3 AND $4 AND record_type = 'sample' \
               AND name IN ('agent.heartbeat', 'host.cpu.utilization', \
                 'host.memory.used_bytes', 'host.memory.total_bytes', \
                 'host.disk.used_bytes', 'host.disk.total_bytes', \
                 'host.disk.inodes_used', 'host.disk.inodes_total', 'process.running', \
                 'os.service.running') \
             ORDER BY sequence LIMIT 64",
        )
        .bind(tenant_id)
        .bind(agent_id)
        .bind(first_sequence)
        .bind(last_sequence)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| ())?
    } else {
        Vec::new()
    };
    transaction.commit().await.map_err(|_| ())?;
    Ok(Some(agent_telemetry_snapshot(
        agent_id,
        &status,
        last_seen_at,
        received_at,
        rows,
        stale_after_seconds,
    )))
}

fn agent_telemetry_snapshot(
    agent_id: uuid::Uuid,
    agent_status: &str,
    last_seen_at: Option<chrono::DateTime<Utc>>,
    received_at: Option<chrono::DateTime<Utc>>,
    rows: Vec<AgentTelemetryMetricRow>,
    stale_after_seconds: u32,
) -> AgentTelemetrySnapshotResponse {
    let stale_before = Utc::now() - ChronoDuration::seconds(i64::from(stale_after_seconds));
    let connection_state = agent_connection_state(agent_status, last_seen_at, stale_before);
    let snapshot_stale = received_at.is_none_or(|value| value < stale_before);
    let mut values = BTreeMap::new();
    let mut observed_at = None;
    let mut platform = None;
    let mut architecture = None;
    let mut processes = BTreeMap::new();
    let mut services = BTreeMap::new();
    for (name, value, row_observed_at, attributes) in rows {
        if name == "agent.heartbeat" {
            observed_at = Some(row_observed_at);
            platform = safe_agent_attribute(&attributes, "os");
            architecture = safe_agent_attribute(&attributes, "arch");
        } else if name == "process.running" {
            insert_agent_process(&mut processes, value, &attributes);
        } else if name == "os.service.running" {
            insert_agent_service(&mut services, value, &attributes);
        } else {
            values.insert(name, value);
        }
    }

    let cpu_usage_percent = values
        .get("host.cpu.utilization")
        .copied()
        .filter(|value| (0.0..=1.0).contains(value))
        .map(|value| value * 100.0);
    let memory = capacity_response(
        values.get("host.memory.used_bytes").copied(),
        values.get("host.memory.total_bytes").copied(),
    );
    let disk = capacity_response(
        values.get("host.disk.used_bytes").copied(),
        values.get("host.disk.total_bytes").copied(),
    );
    let inodes = count_capacity_response(
        values.get("host.disk.inodes_used").copied(),
        values.get("host.disk.inodes_total").copied(),
    );
    let mut missing_metrics = Vec::new();
    if observed_at.is_none() {
        missing_metrics.push("agent.heartbeat");
    }
    if cpu_usage_percent.is_none() {
        missing_metrics.push("host.cpu.utilization");
    }
    if memory.is_none() {
        missing_metrics.extend(["host.memory.used_bytes", "host.memory.total_bytes"]);
    }
    if disk.is_none() {
        missing_metrics.extend(["host.disk.used_bytes", "host.disk.total_bytes"]);
    }
    let collection_state = if received_at.is_none() {
        "unavailable"
    } else if missing_metrics.is_empty() {
        "complete"
    } else {
        "partial"
    };
    AgentTelemetrySnapshotResponse {
        agent_id,
        connection_state,
        collection_state,
        observed_at,
        received_at,
        snapshot_stale,
        platform,
        architecture,
        cpu_usage_percent,
        memory,
        disk,
        inodes,
        processes: processes
            .into_iter()
            .map(|(name, instances)| AgentProcessResponse {
                name,
                running: instances > 0,
                instances,
            })
            .collect(),
        services: services
            .into_iter()
            .map(|(name, (running, state))| AgentServiceResponse {
                name,
                running,
                state,
            })
            .collect(),
        missing_metrics,
    }
}

fn insert_agent_process(
    processes: &mut BTreeMap<String, u32>,
    value: f64,
    attributes: &serde_json::Value,
) {
    if processes.len() < 16
        && let (Some(process_name), Some(instances)) = (
            safe_agent_attribute(attributes, "process"),
            process_instances(value),
        )
    {
        processes.insert(process_name, instances);
    }
}

fn insert_agent_service(
    services: &mut BTreeMap<String, (Option<bool>, String)>,
    value: f64,
    attributes: &serde_json::Value,
) {
    if services.len() < 16
        && let (Some(service_name), Some(state)) = (
            safe_agent_attribute(attributes, "service"),
            safe_service_state(attributes),
        )
    {
        let running = match safe_agent_attribute(attributes, "known").as_deref() {
            Some("true") => Some(value >= 1.0),
            _ => None,
        };
        services.insert(service_name, (running, state));
    }
}

fn safe_agent_attribute(attributes: &serde_json::Value, key: &str) -> Option<String> {
    let value = attributes.get(key)?.as_str()?;
    let length = value.chars().count();
    ((1..=64).contains(&length) && !value.chars().any(char::is_control)).then(|| value.to_owned())
}

fn safe_service_state(attributes: &serde_json::Value) -> Option<String> {
    let state = safe_agent_attribute(attributes, "state")?;
    [
        "active",
        "reloading",
        "activating",
        "deactivating",
        "inactive",
        "failed",
        "not_found",
        "permission_denied",
        "manager_unavailable",
        "query_timeout",
        "unsupported_platform",
        "query_failed",
        "unknown",
    ]
    .contains(&state.as_str())
    .then_some(state)
}

fn process_instances(value: f64) -> Option<u32> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(value as u32)
}

fn capacity_response(
    used_bytes: Option<f64>,
    total_bytes: Option<f64>,
) -> Option<AgentCapacityResponse> {
    const MAX_EXACT_BYTES: f64 = 9_007_199_254_740_992.0;
    let used_bytes = used_bytes.filter(|value| (0.0..=MAX_EXACT_BYTES).contains(value))?;
    let total_bytes = total_bytes.filter(|value| (0.0..=MAX_EXACT_BYTES).contains(value))?;
    if total_bytes <= 0.0 || used_bytes > total_bytes {
        return None;
    }
    Some(AgentCapacityResponse {
        used_bytes,
        total_bytes,
        utilization_percent: (used_bytes / total_bytes) * 100.0,
    })
}

fn count_capacity_response(
    used: Option<f64>,
    total: Option<f64>,
) -> Option<AgentCountCapacityResponse> {
    const MAX_EXACT_COUNT: f64 = 9_007_199_254_740_992.0;
    let used =
        used.filter(|value| (0.0..=MAX_EXACT_COUNT).contains(value) && value.fract() == 0.0)?;
    let total =
        total.filter(|value| (0.0..=MAX_EXACT_COUNT).contains(value) && value.fract() == 0.0)?;
    if total <= 0.0 || used > total {
        return None;
    }
    Some(AgentCountCapacityResponse {
        used,
        total,
        utilization_percent: (used / total) * 100.0,
    })
}

async fn select_project_agents(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    project_id: uuid::Uuid,
    stale_after_seconds: u32,
) -> Result<Option<Vec<AgentResponse>>, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| ())?;
    let project_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE tenant_id = $1 AND id = $2)",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| ())?;
    if !project_exists {
        transaction.rollback().await.map_err(|_| ())?;
        return Ok(None);
    }
    let rows = sqlx::query_as::<_, AgentRow>(
        "SELECT agents.id, agents.display_name, agents.status, agents.enrolled_at, \
           agents.last_seen_at FROM project_agents JOIN agents \
           ON agents.tenant_id = project_agents.tenant_id \
          AND agents.id = project_agents.agent_id \
         WHERE project_agents.tenant_id = $1 AND project_agents.project_id = $2 \
         ORDER BY project_agents.assigned_at, agents.id LIMIT 500",
    )
    .bind(tenant_id)
    .bind(project_id)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(Some(agent_responses(rows, stale_after_seconds)))
}

fn agent_responses(rows: Vec<AgentRow>, stale_after_seconds: u32) -> Vec<AgentResponse> {
    let stale_before = Utc::now() - ChronoDuration::seconds(i64::from(stale_after_seconds));
    rows.into_iter()
        .map(
            |(id, display_name, status, enrolled_at, last_seen_at)| AgentResponse {
                id,
                display_name,
                connection_state: agent_connection_state(&status, last_seen_at, stale_before),
                status,
                enrolled_at,
                last_seen_at,
            },
        )
        .collect()
}

fn agent_connection_state(
    status: &str,
    last_seen_at: Option<chrono::DateTime<Utc>>,
    stale_before: chrono::DateTime<Utc>,
) -> String {
    match (status, last_seen_at) {
        ("revoked", _) => "revoked",
        ("quarantined", _) => "quarantined",
        ("active", None) => "never_seen",
        ("active", Some(last_seen)) if last_seen < stale_before => "stale",
        ("active", Some(_)) => "online",
        _ => "unknown",
    }
    .to_owned()
}

async fn set_project_agent_assignment(
    database: &PgPool,
    principal: Principal,
    project_id: uuid::Uuid,
    agent_id: uuid::Uuid,
) -> Result<(), InventoryWriteError> {
    let mut transaction = begin_inventory_transaction(database, principal.tenant_id).await?;
    let (project_exists, agent_exists) = sqlx::query_as::<_, (bool, bool)>(
        "SELECT \
           EXISTS(SELECT 1 FROM projects WHERE tenant_id = $1 AND id = $2), \
           EXISTS(SELECT 1 FROM agents WHERE tenant_id = $1 AND id = $3)",
    )
    .bind(principal.tenant_id)
    .bind(project_id)
    .bind(agent_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| InventoryWriteError::Database)?;
    if !project_exists || !agent_exists {
        transaction
            .rollback()
            .await
            .map_err(|_| InventoryWriteError::Database)?;
        return Err(InventoryWriteError::NotFound);
    }
    let inserted = sqlx::query(
        "INSERT INTO project_agents (tenant_id, project_id, agent_id) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(principal.tenant_id)
    .bind(project_id)
    .bind(agent_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| InventoryWriteError::Database)?;
    if inserted.rows_affected() > 0 {
        insert_project_agent_audit(
            &mut transaction,
            principal,
            "workspace.agent.assign",
            project_id,
            agent_id,
        )
        .await?;
    }
    transaction
        .commit()
        .await
        .map_err(|_| InventoryWriteError::Database)
}

async fn remove_project_agent_assignment(
    database: &PgPool,
    principal: Principal,
    project_id: uuid::Uuid,
    agent_id: uuid::Uuid,
) -> Result<(), InventoryWriteError> {
    let mut transaction = begin_inventory_transaction(database, principal.tenant_id).await?;
    let removed = sqlx::query(
        "DELETE FROM project_agents \
         WHERE tenant_id = $1 AND project_id = $2 AND agent_id = $3",
    )
    .bind(principal.tenant_id)
    .bind(project_id)
    .bind(agent_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| InventoryWriteError::Database)?;
    if removed.rows_affected() == 0 {
        transaction
            .rollback()
            .await
            .map_err(|_| InventoryWriteError::Database)?;
        return Err(InventoryWriteError::NotFound);
    }
    insert_project_agent_audit(
        &mut transaction,
        principal,
        "workspace.agent.unassign",
        project_id,
        agent_id,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| InventoryWriteError::Database)
}

async fn insert_project_agent_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    principal: Principal,
    action: &'static str,
    project_id: uuid::Uuid,
    agent_id: uuid::Uuid,
) -> Result<(), InventoryWriteError> {
    sqlx::query(
        "INSERT INTO audit_events \
         (tenant_id, actor_type, actor_id, action, target_type, target_id, details) \
         VALUES ($1, 'user', $2, $3, 'agent', $4, jsonb_build_object('project_id', $5))",
    )
    .bind(principal.tenant_id)
    .bind(principal.user_id)
    .bind(action)
    .bind(agent_id)
    .bind(project_id)
    .execute(&mut **transaction)
    .await
    .map_err(|_| InventoryWriteError::Database)?;
    Ok(())
}

async fn mark_agent_revoked(
    database: &PgPool,
    principal: Principal,
    agent_id: uuid::Uuid,
) -> Result<(), AgentWriteError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(|_| AgentWriteError::Database)?;
    set_tenant_context(&mut transaction, principal.tenant_id)
        .await
        .map_err(|_| AgentWriteError::Database)?;
    let revoked = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE agents SET status = 'revoked', revoked_at = now() \
         WHERE tenant_id = $1 AND id = $2 AND status <> 'revoked' RETURNING id",
    )
    .bind(principal.tenant_id)
    .bind(agent_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    if revoked.is_none() {
        transaction
            .rollback()
            .await
            .map_err(|_| AgentWriteError::Database)?;
        return Err(AgentWriteError::NotFound);
    }
    sqlx::query(
        "UPDATE agent_credentials SET revoked_at = now() \
         WHERE tenant_id = $1 AND agent_id = $2 AND revoked_at IS NULL",
    )
    .bind(principal.tenant_id)
    .bind(agent_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    sqlx::query(
        "INSERT INTO audit_events \
         (tenant_id, actor_type, actor_id, action, target_type, target_id) \
         VALUES ($1, 'user', $2, 'agent.revoke', 'agent', $3)",
    )
    .bind(principal.tenant_id)
    .bind(principal.user_id)
    .bind(agent_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentWriteError::Database)?;
    transaction
        .commit()
        .await
        .map_err(|_| AgentWriteError::Database)
}

async fn ingest_agent_telemetry(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> HttpResponse {
    let agent = match authenticate_agent(&request, &state).await {
        Ok(agent) => agent,
        Err(AuthenticationFailure::Unauthorized) => return agent_unauthorized(),
        Err(AuthenticationFailure::RateLimited) => return rate_limited(),
        Err(AuthenticationFailure::Database) => return database_unavailable(),
    };
    if require_rate_limit(&request, &state, RateLimitScope::Ingestion)
        .await
        .is_err()
    {
        return rate_limited();
    }
    let Ok(batch) = decode_ingest::<TelemetryBatch>(&request, &body) else {
        return invalid_agent_batch();
    };
    let Some(idempotency_key) = request
        .headers()
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
    else {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "idempotency_key_required",
        });
    };
    if !batch.has_valid_shape()
        || batch.agent_id != agent.agent
        || batch.batch_id != idempotency_key
        || !valid_agent_batch_timestamps(&batch)
    {
        return invalid_agent_batch();
    }
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let payload_digest: [u8; 32] = Sha256::digest(&body).into();
    match persist_agent_batch(database, agent, &batch, &payload_digest).await {
        Ok(AgentBatchResult::Accepted { gap_detected }) => {
            let mut response = HttpResponse::Accepted();
            if gap_detected {
                response.insert_header(("X-Meerkateer-Sequence-Gap", "true"));
            }
            response
                .insert_header((header::CACHE_CONTROL, "no-store"))
                .json(agent_telemetry_ack(&batch))
        }
        Ok(AgentBatchResult::Duplicate) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(agent_telemetry_ack(&batch)),
        Err(AgentBatchWriteError::Unauthorized) => agent_unauthorized(),
        Err(AgentBatchWriteError::Conflict) => HttpResponse::Conflict().json(ErrorResponse {
            code: "sequence_or_idempotency_conflict",
        }),
        Err(AgentBatchWriteError::Database) => database_unavailable(),
    }
}

fn agent_telemetry_ack(batch: &TelemetryBatch) -> AgentTelemetryAcknowledgement {
    AgentTelemetryAcknowledgement {
        protocol_version: PROTOCOL_VERSION,
        agent_id: batch.agent_id,
        batch_id: batch.batch_id,
        status: "accepted",
        accepted_through_sequence: batch.last_sequence,
        rejected: Vec::new(),
        server_time: now(),
    }
}

fn valid_agent_batch_timestamps(batch: &TelemetryBatch) -> bool {
    validate_bounded_timestamp(&batch.sent_at, ChronoDuration::hours(24)).is_some()
        && batch.records.iter().all(|record| {
            let timestamp = match record {
                TelemetryRecord::Sample { observed_at, .. }
                | TelemetryRecord::Event { observed_at, .. } => observed_at,
            };
            validate_bounded_timestamp(timestamp, ChronoDuration::days(7)).is_some()
        })
}

fn validate_bounded_timestamp(
    value: &str,
    maximum_age: ChronoDuration,
) -> Option<chrono::DateTime<Utc>> {
    if value.len() > 35 || !value.ends_with('Z') {
        return None;
    }
    let timestamp = chrono::DateTime::parse_from_rfc3339(value)
        .ok()?
        .with_timezone(&Utc);
    let now = Utc::now();
    (timestamp <= now + ChronoDuration::minutes(5) && timestamp >= now - maximum_age)
        .then_some(timestamp)
}

#[derive(Debug, Clone, Copy)]
enum AgentBatchResult {
    Accepted { gap_detected: bool },
    Duplicate,
}

#[derive(Debug, Clone, Copy)]
enum AgentBatchWriteError {
    Unauthorized,
    Conflict,
    Database,
}

async fn persist_agent_batch(
    database: &PgPool,
    agent: AuthenticatedAgent,
    batch: &TelemetryBatch,
    payload_digest: &[u8; 32],
) -> Result<AgentBatchResult, AgentBatchWriteError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(|_| AgentBatchWriteError::Database)?;
    set_tenant_context(&mut transaction, agent.tenant)
        .await
        .map_err(|_| AgentBatchWriteError::Database)?;
    require_active_agent_credential(&mut transaction, agent).await?;
    if let Some((stored_digest, accepted_through)) =
        find_agent_batch(&mut transaction, agent, batch.batch_id).await?
    {
        let digest_matches = stored_digest.as_slice() == payload_digest;
        let sequence_matches =
            u64::try_from(accepted_through).is_ok_and(|sequence| sequence == batch.last_sequence);
        transaction
            .commit()
            .await
            .map_err(|_| AgentBatchWriteError::Database)?;
        return if digest_matches && sequence_matches {
            Ok(AgentBatchResult::Duplicate)
        } else {
            Err(AgentBatchWriteError::Conflict)
        };
    }
    let previous_sequence = lock_agent_sequence(&mut transaction, agent).await?;
    let first_sequence =
        i64::try_from(batch.first_sequence).map_err(|_| AgentBatchWriteError::Conflict)?;
    let last_sequence =
        i64::try_from(batch.last_sequence).map_err(|_| AgentBatchWriteError::Conflict)?;
    if first_sequence <= previous_sequence {
        return Err(AgentBatchWriteError::Conflict);
    }
    let gap_detected = first_sequence > previous_sequence.saturating_add(1);
    insert_agent_records(&mut transaction, agent, batch).await?;
    sqlx::query(
        "INSERT INTO agent_batches \
         (tenant_id, agent_id, batch_id, first_sequence, last_sequence, payload_digest, gap_detected) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .bind(batch.batch_id)
    .bind(first_sequence)
    .bind(last_sequence)
    .bind(payload_digest.as_slice())
    .bind(gap_detected)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentBatchWriteError::Conflict)?;
    sqlx::query(
        "UPDATE agent_sequence_state SET last_sequence = $3, updated_at = now() \
         WHERE tenant_id = $1 AND agent_id = $2",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .bind(last_sequence)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentBatchWriteError::Database)?;
    let outbox_payload = serde_json::json!({
        "agent_id": agent.agent,
        "batch_id": batch.batch_id,
        "first_sequence": batch.first_sequence,
        "last_sequence": batch.last_sequence,
        "gap_detected": gap_detected
    });
    sqlx::query(
        "INSERT INTO outbox (tenant_id, topic, payload) VALUES ($1, 'agent.telemetry', $2)",
    )
    .bind(agent.tenant)
    .bind(outbox_payload)
    .execute(&mut *transaction)
    .await
    .map_err(|_| AgentBatchWriteError::Database)?;
    transaction
        .commit()
        .await
        .map_err(|_| AgentBatchWriteError::Database)?;
    Ok(AgentBatchResult::Accepted { gap_detected })
}

async fn require_active_agent_credential(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent: AuthenticatedAgent,
) -> Result<(), AgentBatchWriteError> {
    let active = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE agent_credentials SET last_used_at = now() \
         WHERE tenant_id = $1 AND agent_id = $2 AND id = $3 \
           AND revoked_at IS NULL AND valid_after <= now() AND expires_at > now() \
         RETURNING id",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .bind(agent.credential)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AgentBatchWriteError::Database)?;
    if active.is_none() {
        return Err(AgentBatchWriteError::Unauthorized);
    }
    sqlx::query(
        "UPDATE agents SET last_seen_at = now() \
         WHERE tenant_id = $1 AND id = $2 AND status = 'active'",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .execute(&mut **transaction)
    .await
    .map_err(|_| AgentBatchWriteError::Database)?;
    Ok(())
}

async fn find_agent_batch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent: AuthenticatedAgent,
    batch_id: uuid::Uuid,
) -> Result<Option<(Vec<u8>, i64)>, AgentBatchWriteError> {
    sqlx::query_as(
        "SELECT payload_digest, last_sequence FROM agent_batches \
         WHERE tenant_id = $1 AND agent_id = $2 AND batch_id = $3",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .bind(batch_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| AgentBatchWriteError::Database)
}

async fn lock_agent_sequence(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent: AuthenticatedAgent,
) -> Result<i64, AgentBatchWriteError> {
    sqlx::query(
        "INSERT INTO agent_sequence_state (tenant_id, agent_id, last_sequence) \
         VALUES ($1, $2, 0) ON CONFLICT (tenant_id, agent_id) DO NOTHING",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .execute(&mut **transaction)
    .await
    .map_err(|_| AgentBatchWriteError::Database)?;
    sqlx::query_scalar(
        "SELECT last_sequence FROM agent_sequence_state \
         WHERE tenant_id = $1 AND agent_id = $2 FOR UPDATE",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| AgentBatchWriteError::Database)
}

async fn insert_agent_records(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent: AuthenticatedAgent,
    batch: &TelemetryBatch,
) -> Result<(), AgentBatchWriteError> {
    let records = batch
        .records
        .iter()
        .map(normalize_agent_record)
        .collect::<Result<Vec<_>, _>>()?;
    sqlx::query(
        "INSERT INTO agent_telemetry_records \
         (tenant_id, agent_id, record_id, sequence, record_type, observed_at, name, \
          value, severity, message, attributes) \
         SELECT $1, $2, record_id, sequence, record_type, observed_at, name, \
           value, severity, message, attributes \
         FROM jsonb_to_recordset($3::jsonb) AS record( \
           record_id uuid, sequence bigint, record_type text, observed_at timestamptz, \
           name text, value double precision, severity text, message text, attributes jsonb)",
    )
    .bind(agent.tenant)
    .bind(agent.agent)
    .bind(serde_json::Value::Array(records))
    .execute(&mut **transaction)
    .await
    .map_err(|_| AgentBatchWriteError::Conflict)?;
    Ok(())
}

fn normalize_agent_record(
    record: &TelemetryRecord,
) -> Result<serde_json::Value, AgentBatchWriteError> {
    match record {
        TelemetryRecord::Sample {
            sequence,
            record_id,
            observed_at,
            metric,
            value,
            attributes,
        } => Ok(serde_json::json!({
            "record_id": record_id,
            "sequence": i64::try_from(*sequence).map_err(|_| AgentBatchWriteError::Conflict)?,
            "record_type": "sample",
            "observed_at": observed_at,
            "name": metric,
            "value": value,
            "severity": null,
            "message": null,
            "attributes": attributes
        })),
        TelemetryRecord::Event {
            sequence,
            record_id,
            observed_at,
            kind,
            severity,
            message,
            attributes,
        } => Ok(serde_json::json!({
            "record_id": record_id,
            "sequence": i64::try_from(*sequence).map_err(|_| AgentBatchWriteError::Conflict)?,
            "record_type": "event",
            "observed_at": observed_at,
            "name": kind,
            "value": null,
            "severity": severity,
            "message": message,
            "attributes": attributes
        })),
    }
}

fn invalid_agent_batch() -> HttpResponse {
    HttpResponse::BadRequest().json(ErrorResponse {
        code: "invalid_batch",
    })
}

async fn ingest_heartbeat(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> HttpResponse {
    let service = match authenticate_service(&request, &state).await {
        Ok(service) => service,
        Err(AuthenticationFailure::Unauthorized) => return service_unauthorized(),
        Err(AuthenticationFailure::RateLimited) => return rate_limited(),
        Err(AuthenticationFailure::Database) => return database_unavailable(),
    };
    if require_rate_limit(&request, &state, RateLimitScope::Ingestion)
        .await
        .is_err()
    {
        return rate_limited();
    }
    let Ok(payload) = decode_ingest::<HeartbeatIngest>(&request, &body) else {
        return invalid_ingest();
    };
    if !payload.has_valid_shape()
        || !ingest_identity_matches(
            &service,
            &payload.service,
            &payload.project,
            &payload.environment,
        )
    {
        return invalid_ingest();
    }
    let Ok(observed_at) = validate_ingest_timestamp(&payload.timestamp, IngestKind::Heartbeat)
    else {
        return invalid_ingest();
    };
    let snapshot_state = match payload.status {
        HeartbeatStatus::Ok => "online",
        HeartbeatStatus::Degraded => "degraded",
        HeartbeatStatus::Down => "offline",
    };
    accept_ingest(
        &request,
        &state,
        service,
        IngestKind::Heartbeat,
        observed_at,
        &payload,
        Some(snapshot_state),
    )
    .await
}

async fn ingest_event(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> HttpResponse {
    let service = match authenticate_service(&request, &state).await {
        Ok(service) => service,
        Err(AuthenticationFailure::Unauthorized) => return service_unauthorized(),
        Err(AuthenticationFailure::RateLimited) => return rate_limited(),
        Err(AuthenticationFailure::Database) => return database_unavailable(),
    };
    if require_rate_limit(&request, &state, RateLimitScope::Ingestion)
        .await
        .is_err()
    {
        return rate_limited();
    }
    let Ok(payload) = decode_ingest::<EventIngest>(&request, &body) else {
        return invalid_ingest();
    };
    if !payload.has_valid_shape()
        || !ingest_identity_matches(
            &service,
            &payload.service,
            &payload.project,
            &payload.environment,
        )
    {
        return invalid_ingest();
    }
    let Ok(observed_at) = validate_ingest_timestamp(&payload.timestamp, IngestKind::Event) else {
        return invalid_ingest();
    };
    accept_ingest(
        &request,
        &state,
        service,
        IngestKind::Event,
        observed_at,
        &payload,
        None,
    )
    .await
}

async fn ingest_deploy(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> HttpResponse {
    let service = match authenticate_service(&request, &state).await {
        Ok(service) => service,
        Err(AuthenticationFailure::Unauthorized) => return service_unauthorized(),
        Err(AuthenticationFailure::RateLimited) => return rate_limited(),
        Err(AuthenticationFailure::Database) => return database_unavailable(),
    };
    if require_rate_limit(&request, &state, RateLimitScope::Ingestion)
        .await
        .is_err()
    {
        return rate_limited();
    }
    let Ok(payload) = decode_ingest::<DeployIngest>(&request, &body) else {
        return invalid_ingest();
    };
    if !payload.has_valid_shape()
        || !ingest_identity_matches(
            &service,
            &payload.service,
            &payload.project,
            &payload.environment,
        )
    {
        return invalid_ingest();
    }
    let Ok(observed_at) = validate_ingest_timestamp(&payload.timestamp, IngestKind::Deploy) else {
        return invalid_ingest();
    };
    accept_ingest(
        &request,
        &state,
        service,
        IngestKind::Deploy,
        observed_at,
        &payload,
        None,
    )
    .await
}

fn decode_ingest<T: serde::de::DeserializeOwned>(
    request: &HttpRequest,
    body: &[u8],
) -> Result<T, ()> {
    let is_json = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("application/json"));
    if !is_json || body.is_empty() {
        return Err(());
    }
    serde_json::from_slice(body).map_err(|_| ())
}

fn ingest_identity_matches(
    authenticated: &AuthenticatedService,
    service: &str,
    project: &str,
    environment: &str,
) -> bool {
    constant_time_secret_eq(&authenticated.service_slug, service)
        && constant_time_secret_eq(&authenticated.project_slug, project)
        && constant_time_secret_eq(&authenticated.environment, environment)
}

fn validate_ingest_timestamp(value: &str, kind: IngestKind) -> Result<chrono::DateTime<Utc>, ()> {
    if value.len() > 35 || !value.ends_with('Z') {
        return Err(());
    }
    let observed = chrono::DateTime::parse_from_rfc3339(value)
        .map_err(|_| ())?
        .with_timezone(&Utc);
    let now = Utc::now();
    if observed > now + ChronoDuration::minutes(5) || observed < now - kind.maximum_age() {
        return Err(());
    }
    Ok(observed)
}

async fn accept_ingest<T: Serialize>(
    request: &HttpRequest,
    state: &AppState,
    service: AuthenticatedService,
    kind: IngestKind,
    observed_at: chrono::DateTime<Utc>,
    payload: &T,
    snapshot_state: Option<&'static str>,
) -> HttpResponse {
    let Some(idempotency_key) = request
        .headers()
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
    else {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "idempotency_key_required",
        });
    };
    let Ok(payload) = serde_json::to_value(payload) else {
        return invalid_ingest();
    };
    let Some(database) = &state.database else {
        return database_unavailable();
    };
    let received_at = Utc::now();
    match persist_ingest(
        database,
        &service,
        idempotency_key,
        kind,
        observed_at,
        received_at,
        payload,
        snapshot_state,
        state.config.deployment_mode == DeploymentMode::Community
            && alert_webhook_url().is_ok_and(|url| url.is_some()),
    )
    .await
    {
        Ok(true) => HttpResponse::Accepted()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(IngestAcknowledgement {
                status: "accepted",
                idempotency_key,
                received_at,
            }),
        Ok(false) => HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(IngestAcknowledgement {
                status: "duplicate",
                idempotency_key,
                received_at,
            }),
        Err(IngestWriteError::IdempotencyConflict) => {
            HttpResponse::Conflict().json(ErrorResponse {
                code: "idempotency_conflict",
            })
        }
        Err(IngestWriteError::Unauthorized) => service_unauthorized(),
        Err(IngestWriteError::Database) => database_unavailable(),
    }
}

#[derive(Debug, Clone, Copy)]
enum IngestWriteError {
    Unauthorized,
    IdempotencyConflict,
    Database,
}

struct AcceptedIngest<'a> {
    service: &'a AuthenticatedService,
    idempotency_key: uuid::Uuid,
    kind: IngestKind,
    observed_at: chrono::DateTime<Utc>,
    payload: &'a serde_json::Value,
    snapshot_state: Option<&'static str>,
    webhook_configured: bool,
}

#[derive(Clone, Copy)]
struct IncidentTransition<'a> {
    service: &'a AuthenticatedService,
    idempotency_key: uuid::Uuid,
    transition: &'static str,
    observed_at: chrono::DateTime<Utc>,
    reported_cause: Option<&'a str>,
    webhook_configured: bool,
}

#[allow(clippy::too_many_arguments)]
async fn persist_ingest(
    database: &PgPool,
    service: &AuthenticatedService,
    idempotency_key: uuid::Uuid,
    kind: IngestKind,
    observed_at: chrono::DateTime<Utc>,
    received_at: chrono::DateTime<Utc>,
    payload: serde_json::Value,
    snapshot_state: Option<&'static str>,
    webhook_configured: bool,
) -> Result<bool, IngestWriteError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(|_| IngestWriteError::Database)?;
    set_tenant_context(&mut transaction, service.tenant)
        .await
        .map_err(|_| IngestWriteError::Database)?;
    let active = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE service_credentials SET last_used_at = now() \
         WHERE tenant_id = $1 AND service_id = $2 AND id = $3 \
           AND revoked_at IS NULL AND valid_after <= now() \
           AND (expires_at IS NULL OR expires_at > now()) RETURNING id",
    )
    .bind(service.tenant)
    .bind(service.service)
    .bind(service.credential)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| IngestWriteError::Database)?;
    if active.is_none() {
        transaction
            .rollback()
            .await
            .map_err(|_| IngestWriteError::Database)?;
        return Err(IngestWriteError::Unauthorized);
    }
    let inserted = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO ingest_messages \
         (tenant_id, service_id, idempotency_key, message_kind, observed_at, payload, received_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (tenant_id, service_id, idempotency_key) DO NOTHING \
         RETURNING idempotency_key",
    )
    .bind(service.tenant)
    .bind(service.service)
    .bind(idempotency_key)
    .bind(kind.as_str())
    .bind(observed_at)
    .bind(&payload)
    .bind(received_at)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| IngestWriteError::Database)?;
    if inserted.is_some() {
        project_accepted_ingest(
            &mut transaction,
            AcceptedIngest {
                service,
                idempotency_key,
                kind,
                observed_at,
                payload: &payload,
                snapshot_state,
                webhook_configured,
            },
        )
        .await?;
    } else if !duplicate_matches(&mut transaction, service, idempotency_key, kind, &payload).await?
    {
        transaction
            .rollback()
            .await
            .map_err(|_| IngestWriteError::Database)?;
        return Err(IngestWriteError::IdempotencyConflict);
    }
    transaction
        .commit()
        .await
        .map_err(|_| IngestWriteError::Database)?;
    Ok(inserted.is_some())
}

async fn project_accepted_ingest(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ingest: AcceptedIngest<'_>,
) -> Result<(), IngestWriteError> {
    let AcceptedIngest {
        service,
        idempotency_key,
        kind,
        observed_at,
        payload,
        snapshot_state,
        webhook_configured,
    } = ingest;
    if let Some(state) = snapshot_state {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
            .bind(service.service)
            .execute(&mut **transaction)
            .await
            .map_err(|_| IngestWriteError::Database)?;
        let previous: Option<(String, chrono::DateTime<Utc>)> = sqlx::query_as(
            "SELECT state, observed_at FROM service_snapshots \
             WHERE tenant_id = $1 AND service_id = $2 FOR UPDATE",
        )
        .bind(service.tenant)
        .bind(service.service)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| IngestWriteError::Database)?;
        let is_newer = previous
            .as_ref()
            .is_none_or(|(_, previous_at)| observed_at > *previous_at);
        sqlx::query(
            "INSERT INTO service_snapshots \
             (tenant_id, service_id, state, last_sequence, observed_at) \
             VALUES ($1, $2, $3, 0, $4) \
             ON CONFLICT (tenant_id, service_id) DO UPDATE SET \
               state = EXCLUDED.state, observed_at = EXCLUDED.observed_at, updated_at = now() \
             WHERE service_snapshots.observed_at < EXCLUDED.observed_at",
        )
        .bind(service.tenant)
        .bind(service.service)
        .bind(state)
        .bind(observed_at)
        .execute(&mut **transaction)
        .await
        .map_err(|_| IngestWriteError::Database)?;
        if is_newer {
            let previous_state = previous.as_ref().map(|(state, _)| state.as_str());
            let transition = if state == "offline" && previous_state != Some("offline") {
                Some("down")
            } else if state == "online" && previous_state == Some("offline") {
                Some("recovered")
            } else {
                None
            };
            if let Some(transition) = transition {
                record_incident_transition(
                    transaction,
                    IncidentTransition {
                        service,
                        idempotency_key,
                        transition,
                        observed_at,
                        reported_cause: payload.get("message").and_then(serde_json::Value::as_str),
                        webhook_configured,
                    },
                )
                .await?;
            }
        }
    }
    let outbox_payload = serde_json::json!({
        "service_id": service.service,
        "idempotency_key": idempotency_key,
        "message_kind": kind.as_str()
    });
    sqlx::query("INSERT INTO outbox (tenant_id, topic, payload) VALUES ($1, $2, $3)")
        .bind(service.tenant)
        .bind(format!("ingest.{}", kind.as_str()))
        .bind(outbox_payload)
        .execute(&mut **transaction)
        .await
        .map_err(|_| IngestWriteError::Database)?;
    Ok(())
}

async fn record_incident_transition(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    change: IncidentTransition<'_>,
) -> Result<(), IngestWriteError> {
    let incident_id = update_incident_for_transition(transaction, change).await?;
    create_alert_delivery(transaction, change, incident_id).await
}

async fn update_incident_for_transition(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    change: IncidentTransition<'_>,
) -> Result<Option<uuid::Uuid>, IngestWriteError> {
    let service = change.service;
    if change.transition == "down" {
        let incident_id = uuid::Uuid::new_v4();
        let title = format!("{} is offline", service.service_slug);
        let cause = change.reported_cause.unwrap_or("heartbeat reported down");
        let id = sqlx::query_scalar::<_, uuid::Uuid>(
            "INSERT INTO incidents \
             (tenant_id, id, project_id, service_id, status, severity, title, cause, \
              started_at, last_observed_at, opened_by_ingest_key) \
             VALUES ($1, $2, $3, $4, 'open', 'critical', $5, $6, $7, $7, $8) \
             ON CONFLICT (tenant_id, service_id) WHERE status = 'open' DO UPDATE SET \
               last_observed_at = GREATEST(incidents.last_observed_at, EXCLUDED.last_observed_at), \
               updated_at = now() RETURNING id",
        )
        .bind(service.tenant)
        .bind(incident_id)
        .bind(service.project)
        .bind(service.service)
        .bind(title)
        .bind(cause)
        .bind(change.observed_at)
        .bind(change.idempotency_key)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|_| IngestWriteError::Database)?;
        sqlx::query(
            "INSERT INTO incident_events \
             (tenant_id, incident_id, ingest_key, kind, state, observed_at) \
             VALUES ($1, $2, $3, 'opened', 'offline', $4) ON CONFLICT DO NOTHING",
        )
        .bind(service.tenant)
        .bind(id)
        .bind(change.idempotency_key)
        .bind(change.observed_at)
        .execute(&mut **transaction)
        .await
        .map_err(|_| IngestWriteError::Database)?;
        Ok(Some(id))
    } else {
        let id = sqlx::query_scalar::<_, uuid::Uuid>(
            "UPDATE incidents SET status = 'resolved', last_observed_at = $4, resolved_at = $4, \
             resolved_by_ingest_key = $3, updated_at = now() \
             WHERE tenant_id = $1 AND service_id = $2 AND status = 'open' RETURNING id",
        )
        .bind(service.tenant)
        .bind(service.service)
        .bind(change.idempotency_key)
        .bind(change.observed_at)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| IngestWriteError::Database)?;
        if let Some(id) = id {
            sqlx::query(
                "INSERT INTO incident_events \
                 (tenant_id, incident_id, ingest_key, kind, state, observed_at) \
                 VALUES ($1, $2, $3, 'resolved', 'online', $4) ON CONFLICT DO NOTHING",
            )
            .bind(service.tenant)
            .bind(id)
            .bind(change.idempotency_key)
            .bind(change.observed_at)
            .execute(&mut **transaction)
            .await
            .map_err(|_| IngestWriteError::Database)?;
        }
        Ok(id)
    }
}

async fn create_alert_delivery(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    change: IncidentTransition<'_>,
    incident_id: Option<uuid::Uuid>,
) -> Result<(), IngestWriteError> {
    let delivery_id = uuid::Uuid::new_v4();
    let (status, suppression_reason, outbox_id) =
        classify_and_enqueue_alert(transaction, change, delivery_id).await?;
    sqlx::query(
        "INSERT INTO alert_deliveries \
         (tenant_id, id, project_id, service_id, incident_id, outbox_id, transition, status, \
          observed_at, suppression_reason) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(change.service.tenant)
    .bind(delivery_id)
    .bind(change.service.project)
    .bind(change.service.service)
    .bind(incident_id)
    .bind(outbox_id)
    .bind(change.transition)
    .bind(status)
    .bind(change.observed_at)
    .bind(suppression_reason)
    .execute(&mut **transaction)
    .await
    .map_err(|_| IngestWriteError::Database)?;
    Ok(())
}

async fn classify_and_enqueue_alert(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    change: IncidentTransition<'_>,
    delivery_id: uuid::Uuid,
) -> Result<(&'static str, Option<String>, Option<uuid::Uuid>), IngestWriteError> {
    let service = change.service;
    let policy = sqlx::query_as::<_, (bool, bool, bool, i32)>(
        "SELECT enabled, notify_down, notify_recovered, cooldown_seconds \
         FROM alert_policies WHERE tenant_id = $1",
    )
    .bind(service.tenant)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| IngestWriteError::Database)?
    .unwrap_or((true, true, true, 0));
    let transition_enabled = if change.transition == "down" {
        policy.1
    } else {
        policy.2
    };
    let maintenance_reason = sqlx::query_scalar::<_, String>(
        "SELECT reason FROM maintenance_windows \
         WHERE tenant_id = $1 AND service_id = $2 AND cancelled_at IS NULL \
           AND starts_at <= $3 AND ends_at > $3 \
         ORDER BY starts_at DESC, id LIMIT 1",
    )
    .bind(service.tenant)
    .bind(service.service)
    .bind(change.observed_at)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| IngestWriteError::Database)?;
    let cooldown_active = if change.transition == "down" && policy.3 > 0 {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM alert_deliveries \
             WHERE tenant_id = $1 AND service_id = $2 AND transition = 'down' \
               AND status IN ('queued', 'retrying', 'delivered', 'dead_lettered') \
               AND observed_at < $3 \
               AND observed_at >= $3 - make_interval(secs => $4))",
        )
        .bind(service.tenant)
        .bind(service.service)
        .bind(change.observed_at)
        .bind(policy.3)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|_| IngestWriteError::Database)?
    } else {
        false
    };
    if let Some(reason) = maintenance_reason {
        Ok(("suppressed", Some(reason), None))
    } else if !policy.0 || !transition_enabled {
        Ok(("skipped_disabled", None, None))
    } else if cooldown_active {
        Ok((
            "suppressed",
            Some(format!("Alert cooldown active ({} seconds)", policy.3)),
            None,
        ))
    } else if !change.webhook_configured {
        Ok(("skipped_unconfigured", None, None))
    } else {
        let outbox_id = uuid::Uuid::new_v4();
        let alert = serde_json::json!({
            "delivery_id": delivery_id,
            "service_id": service.service,
            "project": service.project_slug,
            "service": service.service_slug,
            "environment": service.environment,
            "transition": change.transition,
            "observed_at": change.observed_at,
        });
        sqlx::query(
            "INSERT INTO outbox (id, tenant_id, topic, payload) VALUES ($1, $2, 'alert.transition', $3)",
        )
        .bind(outbox_id)
        .bind(service.tenant)
        .bind(alert)
        .execute(&mut **transaction)
        .await
        .map_err(|_| IngestWriteError::Database)?;
        Ok(("queued", None, Some(outbox_id)))
    }
}

async fn duplicate_matches(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    service: &AuthenticatedService,
    idempotency_key: uuid::Uuid,
    kind: IngestKind,
    payload: &serde_json::Value,
) -> Result<bool, IngestWriteError> {
    let existing = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT message_kind, payload FROM ingest_messages \
         WHERE tenant_id = $1 AND service_id = $2 AND idempotency_key = $3",
    )
    .bind(service.tenant)
    .bind(service.service)
    .bind(idempotency_key)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| IngestWriteError::Database)?;
    Ok(existing.is_some_and(|(existing_kind, existing_payload)| {
        existing_kind == kind.as_str() && existing_payload == *payload
    }))
}

async fn authenticate_service(
    request: &HttpRequest,
    state: &AppState,
) -> Result<AuthenticatedService, AuthenticationFailure> {
    require_rate_limit(request, state, RateLimitScope::Authentication).await?;
    let Some(presented_value) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| value.len() <= 192)
    else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let Some(tenant_id) = IssuedServiceKey::presented_tenant(presented_value) else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let Some(prefix) = IssuedServiceKey::presented_prefix(presented_value) else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let Some(database) = &state.database else {
        return Err(AuthenticationFailure::Database);
    };
    let Some((
        credential,
        project,
        service,
        password_hash,
        service_slug,
        project_slug,
        environment,
    )) = find_service_credential(database, tenant_id, prefix)
        .await
        .map_err(|()| AuthenticationFailure::Database)?
    else {
        return Err(AuthenticationFailure::Unauthorized);
    };
    let permit = Arc::clone(&state.credential_verifier)
        .acquire_owned()
        .await
        .map_err(|_| AuthenticationFailure::Database)?;
    let presented = secrecy::SecretString::from(presented_value.to_owned());
    let verified = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        IssuedServiceKey::verify(&presented, &password_hash)
    })
    .await
    .map_err(|_| AuthenticationFailure::Database)?;
    verified.map_err(|_| AuthenticationFailure::Unauthorized)?;
    Ok(AuthenticatedService {
        tenant: tenant_id,
        project,
        service,
        credential,
        service_slug,
        project_slug,
        environment,
    })
}

async fn find_service_credential(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    prefix: &str,
) -> Result<
    Option<(
        uuid::Uuid,
        uuid::Uuid,
        uuid::Uuid,
        String,
        String,
        String,
        String,
    )>,
    (),
> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    set_tenant_context(&mut transaction, tenant_id)
        .await
        .map_err(|_| ())?;
    let result = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, uuid::Uuid, String, String, String, String)>(
        "SELECT service_credentials.id, projects.id, services.id, service_credentials.password_hash, \
           services.slug, projects.slug, services.environment \
         FROM service_credentials \
         JOIN services ON services.tenant_id = service_credentials.tenant_id \
           AND services.id = service_credentials.service_id \
         JOIN projects ON projects.tenant_id = services.tenant_id \
           AND projects.id = services.project_id \
         WHERE service_credentials.tenant_id = $1 AND service_credentials.prefix = $2 \
           AND service_credentials.revoked_at IS NULL \
           AND service_credentials.valid_after <= now() \
           AND (service_credentials.expires_at IS NULL OR service_credentials.expires_at > now())",
    )
    .bind(tenant_id)
    .bind(prefix)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(result)
}

fn service_unauthorized() -> HttpResponse {
    HttpResponse::Unauthorized().json(ErrorResponse {
        code: "unauthorized",
    })
}

fn invalid_ingest() -> HttpResponse {
    HttpResponse::BadRequest().json(ErrorResponse {
        code: "invalid_payload",
    })
}

fn database_unavailable() -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(ErrorResponse {
        code: "database_unavailable",
    })
}

#[derive(Debug, Serialize)]
struct IssuedCredentialResponse {
    credential_id: uuid::Uuid,
    prefix: String,
    secret: String,
}

#[derive(Debug, Deserialize)]
struct RotateCredentialRequest {
    overlap_seconds: u32,
}

#[derive(Debug, Serialize)]
struct RotatedCredentialResponse {
    credential_id: uuid::Uuid,
    prefix: String,
    secret: String,
    previous_valid_until: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialWriteError {
    NotFound,
    Database,
}

async fn issue_service_credential(
    request: HttpRequest,
    service_id: web::Path<uuid::Uuid>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::CredentialIssue)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    let tenant_id = session.principal.tenant_id;
    let Ok(Ok(key)) = tokio::task::spawn_blocking(move || IssuedServiceKey::issue(tenant_id)).await
    else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "credential_generation_failed",
        });
    };
    match insert_service_credential(database, session.principal, service_id.into_inner(), &key)
        .await
    {
        Ok(()) => HttpResponse::Created()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(IssuedCredentialResponse {
                credential_id: key.key_id,
                prefix: key.prefix,
                secret: key.secret.expose_secret().to_owned(),
            }),
        Err(CredentialWriteError::NotFound) => {
            HttpResponse::NotFound().json(ErrorResponse { code: "not_found" })
        }
        Err(CredentialWriteError::Database) => {
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                code: "database_unavailable",
            })
        }
    }
}

async fn rotate_service_credential(
    request: HttpRequest,
    path: web::Path<(uuid::Uuid, uuid::Uuid)>,
    body: web::Json<RotateCredentialRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::CredentialRotate)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    if body.overlap_seconds > 3_600 {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_overlap",
        });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    let tenant_id = session.principal.tenant_id;
    let Ok(Ok(key)) = tokio::task::spawn_blocking(move || IssuedServiceKey::issue(tenant_id)).await
    else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "credential_generation_failed",
        });
    };
    let (service_id, credential_id) = path.into_inner();
    let previous_valid_until =
        Utc::now() + ChronoDuration::seconds(i64::from(body.overlap_seconds));
    match replace_service_credential(
        database,
        session.principal,
        service_id,
        credential_id,
        previous_valid_until,
        &key,
    )
    .await
    {
        Ok(()) => HttpResponse::Created()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(RotatedCredentialResponse {
                credential_id: key.key_id,
                prefix: key.prefix,
                secret: key.secret.expose_secret().to_owned(),
                previous_valid_until: previous_valid_until
                    .to_rfc3339_opts(SecondsFormat::Secs, true),
            }),
        Err(CredentialWriteError::NotFound) => {
            HttpResponse::NotFound().json(ErrorResponse { code: "not_found" })
        }
        Err(CredentialWriteError::Database) => {
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                code: "database_unavailable",
            })
        }
    }
}

async fn revoke_service_credential(
    request: HttpRequest,
    path: web::Path<(uuid::Uuid, uuid::Uuid)>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let session = match authenticate_session(&request, &state).await {
        Ok(session) => session,
        Err(failure) => return authentication_error(failure),
    };
    if session
        .principal
        .authorize(session.principal.tenant_id, Action::CredentialRotate)
        .is_err()
    {
        return HttpResponse::Forbidden().json(ErrorResponse { code: "forbidden" });
    }
    if !csrf_is_valid(&request, &session) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            code: "csrf_failed",
        });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };
    let (service_id, credential_id) = path.into_inner();
    match mark_service_credential_revoked(database, session.principal, service_id, credential_id)
        .await
    {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(CredentialWriteError::NotFound) => {
            HttpResponse::NotFound().json(ErrorResponse { code: "not_found" })
        }
        Err(CredentialWriteError::Database) => {
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                code: "database_unavailable",
            })
        }
    }
}

async fn insert_service_credential(
    database: &PgPool,
    principal: Principal,
    service_id: uuid::Uuid,
    key: &IssuedServiceKey,
) -> Result<(), CredentialWriteError> {
    let mut transaction = begin_tenant_transaction(database, principal.tenant_id).await?;
    let inserted = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO service_credentials \
         (tenant_id, service_id, id, prefix, password_hash, created_by) \
         SELECT $1, services.id, $3, $4, $5, $6 FROM services \
         WHERE services.tenant_id = $1 AND services.id = $2 RETURNING id",
    )
    .bind(principal.tenant_id)
    .bind(service_id)
    .bind(key.key_id)
    .bind(&key.prefix)
    .bind(&key.password_hash)
    .bind(principal.user_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| CredentialWriteError::Database)?;
    if inserted.is_none() {
        transaction
            .rollback()
            .await
            .map_err(|_| CredentialWriteError::Database)?;
        return Err(CredentialWriteError::NotFound);
    }
    insert_credential_audit(
        &mut transaction,
        principal,
        "service_credential.issue",
        key.key_id,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| CredentialWriteError::Database)
}

async fn replace_service_credential(
    database: &PgPool,
    principal: Principal,
    service_id: uuid::Uuid,
    credential_id: uuid::Uuid,
    previous_valid_until: chrono::DateTime<Utc>,
    key: &IssuedServiceKey,
) -> Result<(), CredentialWriteError> {
    let mut transaction = begin_tenant_transaction(database, principal.tenant_id).await?;
    let previous = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE service_credentials SET expires_at = CASE \
           WHEN expires_at IS NULL OR expires_at > $4 THEN $4 ELSE expires_at END \
         WHERE tenant_id = $1 AND service_id = $2 AND id = $3 \
           AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now()) \
         RETURNING id",
    )
    .bind(principal.tenant_id)
    .bind(service_id)
    .bind(credential_id)
    .bind(previous_valid_until)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| CredentialWriteError::Database)?;
    if previous.is_none() {
        transaction
            .rollback()
            .await
            .map_err(|_| CredentialWriteError::Database)?;
        return Err(CredentialWriteError::NotFound);
    }
    sqlx::query(
        "INSERT INTO service_credentials \
         (tenant_id, service_id, id, prefix, password_hash, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(principal.tenant_id)
    .bind(service_id)
    .bind(key.key_id)
    .bind(&key.prefix)
    .bind(&key.password_hash)
    .bind(principal.user_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| CredentialWriteError::Database)?;
    insert_credential_audit(
        &mut transaction,
        principal,
        "service_credential.rotate",
        key.key_id,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| CredentialWriteError::Database)
}

async fn mark_service_credential_revoked(
    database: &PgPool,
    principal: Principal,
    service_id: uuid::Uuid,
    credential_id: uuid::Uuid,
) -> Result<(), CredentialWriteError> {
    let mut transaction = begin_tenant_transaction(database, principal.tenant_id).await?;
    let revoked = sqlx::query_scalar::<_, uuid::Uuid>(
        "UPDATE service_credentials SET revoked_at = now() \
         WHERE tenant_id = $1 AND service_id = $2 AND id = $3 AND revoked_at IS NULL \
         RETURNING id",
    )
    .bind(principal.tenant_id)
    .bind(service_id)
    .bind(credential_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| CredentialWriteError::Database)?;
    if revoked.is_none() {
        transaction
            .rollback()
            .await
            .map_err(|_| CredentialWriteError::Database)?;
        return Err(CredentialWriteError::NotFound);
    }
    insert_credential_audit(
        &mut transaction,
        principal,
        "service_credential.revoke",
        credential_id,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| CredentialWriteError::Database)
}

async fn begin_tenant_transaction(
    database: &PgPool,
    tenant_id: uuid::Uuid,
) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, CredentialWriteError> {
    let mut transaction = database
        .begin()
        .await
        .map_err(|_| CredentialWriteError::Database)?;
    sqlx::query("SELECT set_config('meerkateer.tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(|_| CredentialWriteError::Database)?;
    Ok(transaction)
}

async fn insert_credential_audit(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    principal: Principal,
    action: &'static str,
    target_id: uuid::Uuid,
) -> Result<(), CredentialWriteError> {
    sqlx::query(
        "INSERT INTO audit_events \
         (tenant_id, actor_type, actor_id, action, target_type, target_id) \
         VALUES ($1, 'user', $2, $3, 'service_credential', $4)",
    )
    .bind(principal.tenant_id)
    .bind(principal.user_id)
    .bind(action)
    .bind(target_id)
    .execute(&mut **transaction)
    .await
    .map_err(|_| CredentialWriteError::Database)?;
    Ok(())
}

async fn bootstrap(
    request: HttpRequest,
    body: web::Json<BootstrapRequest>,
    state: web::Data<AppState>,
) -> HttpResponse {
    if require_rate_limit(&request, &state, RateLimitScope::Authentication)
        .await
        .is_err()
    {
        return rate_limited();
    }
    let Some(expected_token) = &state.config.bootstrap_token else {
        return HttpResponse::NotFound().json(ErrorResponse { code: "not_found" });
    };
    let supplied_token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if !supplied_token
        .is_some_and(|value| constant_time_secret_eq(value, expected_token.expose_secret()))
    {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            code: "invalid_bootstrap_token",
        });
    }
    if !valid_project_slug(&body.tenant_slug)
        || !valid_display_name(&body.tenant_name)
        || !valid_display_name(&body.owner_name)
        || !valid_email(&body.owner_email)
        || !valid_local_password(&body.owner_password)
    {
        return HttpResponse::BadRequest().json(ErrorResponse {
            code: "invalid_request",
        });
    }
    let Some(database) = &state.database else {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        });
    };

    let Ok(owner_password_hash) = hash_owner_password(&state, &body.owner_password).await else {
        return database_unavailable();
    };

    let tenant_id = uuid::Uuid::new_v4();
    let user_id = uuid::Uuid::new_v4();
    let session = IssuedSession::issue(tenant_id);
    let expires_at = Utc::now() + ChronoDuration::hours(12);
    let result = create_bootstrap_identity(
        database,
        tenant_id,
        user_id,
        &body,
        &owner_password_hash,
        &session,
        expires_at,
    )
    .await;
    match result {
        Ok(true) => bootstrap_success_response(&state, &session, tenant_id, user_id),
        Ok(false) => HttpResponse::Conflict().json(ErrorResponse {
            code: "bootstrap_already_completed",
        }),
        Err(()) => HttpResponse::ServiceUnavailable().json(ErrorResponse {
            code: "database_unavailable",
        }),
    }
}

fn bootstrap_success_response(
    state: &AppState,
    session: &IssuedSession,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> HttpResponse {
    let cookie = Cookie::build(
        "meerkateer_session",
        session.secret.expose_secret().to_owned(),
    )
    .http_only(true)
    .secure(state.config.environment == "production")
    .same_site(SameSite::Lax)
    .path("/")
    .max_age(CookieDuration::hours(12))
    .finish();
    let csrf_cookie = Cookie::build(
        "meerkateer_csrf",
        session.csrf_secret.expose_secret().to_owned(),
    )
    .http_only(false)
    .secure(state.config.environment == "production")
    .same_site(SameSite::Lax)
    .path("/")
    .max_age(CookieDuration::hours(12))
    .finish();
    HttpResponse::Created()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(cookie)
        .cookie(csrf_cookie)
        .json(BootstrapResponse {
            tenant_id,
            user_id,
            role: "owner",
        })
}

async fn create_bootstrap_identity(
    database: &PgPool,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
    request: &BootstrapRequest,
    owner_password_hash: &str,
    session: &IssuedSession,
    expires_at: chrono::DateTime<Utc>,
) -> Result<bool, ()> {
    let mut transaction = database.begin().await.map_err(|_| ())?;
    let available = sqlx::query_scalar::<_, bool>(
        "SELECT bootstrap_completed_at IS NULL FROM system_state \
         WHERE singleton_id = 1 FOR UPDATE",
    )
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| ())?;
    if !available {
        transaction.rollback().await.map_err(|_| ())?;
        return Ok(false);
    }
    sqlx::query("SELECT set_config('meerkateer.tenant_id', $1, true)")
        .bind(tenant_id.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
    sqlx::query("INSERT INTO tenants (id, slug, display_name) VALUES ($1, $2, $3)")
        .bind(tenant_id)
        .bind(&request.tenant_slug)
        .bind(&request.tenant_name)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
    sqlx::query(
        "INSERT INTO users (id, email, display_name, local_password_hash) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(request.owner_email.to_ascii_lowercase())
    .bind(&request.owner_name)
    .bind(owner_password_hash)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ())?;
    sqlx::query("INSERT INTO memberships (tenant_id, user_id, role) VALUES ($1, $2, 'owner')")
        .bind(tenant_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
    let claimed = sqlx::query_scalar::<_, i16>(
        "UPDATE system_state SET bootstrap_completed_at = now(), bootstrap_tenant_id = $1 \
         WHERE singleton_id = 1 AND bootstrap_completed_at IS NULL RETURNING singleton_id",
    )
    .bind(tenant_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| ())?;
    if claimed.is_none() {
        transaction.rollback().await.map_err(|_| ())?;
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO sessions \
         (id, tenant_id, user_id, token_digest, csrf_digest, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(session.session_id)
    .bind(tenant_id)
    .bind(user_id)
    .bind(session.digest.as_slice())
    .bind(session.csrf_digest.as_slice())
    .bind(expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ())?;
    sqlx::query(
        "INSERT INTO audit_events \
         (tenant_id, actor_type, actor_id, action, target_type, target_id) \
         VALUES ($1, 'user', $2, 'tenant.bootstrap', 'tenant', $1)",
    )
    .bind(tenant_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(true)
}

fn valid_project_slug(value: &str) -> bool {
    (2..=64).contains(&value.len())
        && value.chars().enumerate().all(|(index, character)| {
            (character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-')
                && (index > 0 || character.is_ascii_alphanumeric())
        })
        && value
            .chars()
            .last()
            .is_some_and(|character| character.is_ascii_alphanumeric())
}

fn valid_service_slug(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character == '.'
                || character == '_'
                || character == '-'
        })
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric())
}

fn valid_environment(value: &str) -> bool {
    matches!(
        value,
        "development" | "staging" | "production" | "test" | "local"
    )
}

fn valid_display_name(value: &str) -> bool {
    let length = value.chars().count();
    (1..=128).contains(&length) && value.trim() == value
}

fn valid_email(value: &str) -> bool {
    (3..=254).contains(&value.len())
        && value == value.to_ascii_lowercase()
        && value.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && value.is_ascii()
        })
}

async fn health(state: web::Data<AppState>) -> HttpResponse {
    let checks = process_checks(&state).await;
    let is_healthy = checks.values().all(|check| check.ok);
    let response = HealthResponse {
        status: if is_healthy { "ok" } else { "degraded" }.to_owned(),
        service: state.config.service_name.clone(),
        project: state.config.project.clone(),
        environment: state.config.environment.clone(),
        interface: INTERFACE.to_owned(),
        interface_version: INTERFACE_VERSION.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        build: state.build.clone(),
        checks,
        timestamp: now(),
    };
    HttpResponse::build(if is_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    })
    .json(response)
}

async fn ready(state: web::Data<AppState>) -> HttpResponse {
    let checks = process_checks(&state).await;
    let is_ready = checks.values().all(|check| check.ok);
    let response = ReadyResponse {
        ready: is_ready,
        service: state.config.service_name.clone(),
        project: state.config.project.clone(),
        environment: state.config.environment.clone(),
        interface: INTERFACE.to_owned(),
        interface_version: INTERFACE_VERSION.to_owned(),
        checks,
        timestamp: now(),
    };
    HttpResponse::build(if is_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    })
    .json(response)
}

async fn metrics(request: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    if !state.config.metrics_enabled {
        return HttpResponse::NotFound().finish();
    }
    if let Some(expected) = &state.config.metrics_token {
        let supplied = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "));
        if !supplied.is_some_and(|value| constant_time_eq(value, expected.expose_secret())) {
            return HttpResponse::Unauthorized().finish();
        }
    }

    let namespace = state.config.project.replace('-', "_");
    let version = escape_prometheus_label(env!("CARGO_PKG_VERSION"));
    let git_sha = escape_prometheus_label(&state.build.git_sha);
    let git_branch = escape_prometheus_label(&state.build.git_branch);
    let uptime = state.started_at.elapsed().as_secs_f64();
    let body = format!(
        "# HELP {namespace}_build_info Build and source information.\n\
         # TYPE {namespace}_build_info gauge\n\
         {namespace}_build_info{{version=\"{version}\",git_sha=\"{git_sha}\",git_branch=\"{git_branch}\"}} 1\n\
         # HELP {namespace}_uptime_seconds Seconds since process start.\n\
         # TYPE {namespace}_uptime_seconds gauge\n\
         {namespace}_uptime_seconds {uptime}\n\
         # HELP {namespace}_dependency_up Whether a required dependency is reachable.\n\
         # TYPE {namespace}_dependency_up gauge\n\
         {namespace}_dependency_up{{dependency=\"config\"}} 1\n"
    );
    HttpResponse::Ok()
        .insert_header((
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        ))
        .body(body)
}

async fn server_info(state: web::Data<AppState>) -> impl Responder {
    web::Json(ServerInfo {
        name: state.config.service_name.clone(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        mode: state.config.deployment_mode.as_str().to_owned(),
        build: state.build.clone(),
    })
}

async fn metadata(state: web::Data<AppState>) -> impl Responder {
    web::Json(MetadataResponse {
        interface: INTERFACE.to_owned(),
        interface_version: INTERFACE_VERSION.to_owned(),
        service: state.config.service_name.clone(),
        project: state.config.project.clone(),
        environment: state.config.environment.clone(),
        runtime: "rust".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        build: state.build.clone(),
        endpoints: MetadataEndpoints {
            health: "/health".to_owned(),
            ready: "/ready".to_owned(),
            metrics: "/metrics".to_owned(),
            server_info: "/server-info".to_owned(),
        },
        capabilities: MetadataCapabilities {
            health: true,
            readiness: true,
            prometheus_metrics: state.config.metrics_enabled,
            server_info: true,
            push_heartbeat: false,
            push_event: false,
            push_deploy: false,
        },
    })
}

async fn process_checks(state: &AppState) -> BTreeMap<String, CheckResult> {
    let mut checks = BTreeMap::from([("config".to_owned(), CheckResult::ok())]);
    if let Some(database) = &state.database {
        let started = Instant::now();
        let result = timeout(
            Duration::from_secs(2),
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(database),
        )
        .await;
        let latency = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let check = match result {
            Ok(Ok(1)) => CheckResult::ok_with_latency(latency),
            Ok(Ok(_) | Err(_)) | Err(_) => CheckResult::failed(SafeCheckError::Unavailable),
        };
        checks.insert("database".to_owned(), check);
    }
    checks
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn escape_prometheus_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let max_len = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

#[must_use]
pub const fn graceful_shutdown_timeout() -> Duration {
    Duration::from_secs(20)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use actix_web::{
        App,
        cookie::Cookie,
        http::{StatusCode, header},
        test, web,
    };
    use chrono::{Duration as ChronoDuration, Utc};
    use meerkateer_config::ServerConfig;
    use meerkateer_identity::{IssuedSession, Principal, Role};
    use secrecy::ExposeSecret;
    use serde_json::Value;

    use super::{
        AppState, AuthenticatedSession, agent_telemetry_snapshot, configure_routes, csrf_is_valid,
        worker_runtime_status,
    };

    #[actix_web::test]
    async fn worker_progress_distinguishes_fresh_stalled_and_missing_cycles() {
        let current_time = Utc::now();
        assert_eq!(worker_runtime_status(None, current_time), "never_seen");
        assert_eq!(
            worker_runtime_status(
                Some(current_time - ChronoDuration::seconds(30)),
                current_time
            ),
            "healthy"
        );
        assert_eq!(
            worker_runtime_status(
                Some(current_time - ChronoDuration::seconds(31)),
                current_time
            ),
            "stalled"
        );
        assert_eq!(
            worker_runtime_status(
                Some(current_time + ChronoDuration::minutes(6)),
                current_time
            ),
            "stalled"
        );
    }

    fn config(values: &[(&str, &str)]) -> Arc<ServerConfig> {
        let result = ServerConfig::from_source(|name| {
            values
                .iter()
                .find_map(|(key, value)| (*key == name).then(|| (*value).to_owned()))
        });
        let Ok(config) = result else {
            std::process::abort();
        };
        Arc::new(config)
    }

    #[actix_web::test]
    async fn health_is_mks1_conformant_at_startup() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState::new(config(&[]), None)))
                .configure(configure_routes),
        )
        .await;
        let response =
            test::call_service(&app, test::TestRequest::get().uri("/health").to_request()).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["interface"], "meerkateer");
        assert_eq!(body["interface_version"], "1");
        assert_eq!(body["status"], "ok");
        assert_eq!(body["checks"]["config"]["ok"], true);
    }

    #[actix_web::test]
    async fn metrics_token_is_enforced_without_leaking_it() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState::new(
                    config(&[("MEERKATEER_METRICS_TOKEN", "fixture-metrics-token")]),
                    None,
                )))
                .configure(configure_routes),
        )
        .await;
        let unauthorized =
            test::call_service(&app, test::TestRequest::get().uri("/metrics").to_request()).await;
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let authorized = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/metrics")
                .insert_header(("Authorization", "Bearer fixture-metrics-token"))
                .to_request(),
        )
        .await;
        assert_eq!(authorized.status(), StatusCode::OK);
        let body = test::read_body(authorized).await;
        assert!(
            !body
                .windows("fixture-metrics-token".len())
                .any(|window| window == b"fixture-metrics-token")
        );
    }

    #[actix_web::test]
    async fn publishes_parseable_openapi() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState::new(config(&[]), None)))
                .configure(configure_routes),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::get().uri("/openapi.json").to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["openapi"], "3.1.0");
        assert!(body["paths"]["/health"].is_object());
    }

    #[actix_web::test]
    async fn bootstrap_is_hidden_when_not_configured() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState::new(config(&[]), None)))
                .configure(configure_routes),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/bootstrap")
                .set_json(serde_json::json!({
                    "tenant_slug": "arena-ops",
                    "tenant_name": "Arena Ops",
                    "owner_email": "owner@example.com",
                    "owner_name": "Owner",
                    "owner_password": "fixture-owner-password-123"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn bootstrap_token_fails_closed_before_database_access() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState::new(
                    config(&[("MEERKATEER_BOOTSTRAP_TOKEN", "fixture-bootstrap-token")]),
                    None,
                )))
                .configure(configure_routes),
        )
        .await;
        let request_body = serde_json::json!({
            "tenant_slug": "arena-ops",
            "tenant_name": "Arena Ops",
            "owner_email": "owner@example.com",
            "owner_name": "Owner",
            "owner_password": "fixture-owner-password-123"
        });
        let unauthorized = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/bootstrap")
                .insert_header(("Authorization", "Bearer wrong-token"))
                .set_json(&request_body)
                .to_request(),
        )
        .await;
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        let body = test::read_body(unauthorized).await;
        assert!(
            !body
                .windows(23)
                .any(|window| window == b"fixture-bootstrap-token")
        );

        let unavailable = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/bootstrap")
                .insert_header(("Authorization", "Bearer fixture-bootstrap-token"))
                .set_json(&request_body)
                .to_request(),
        )
        .await;
        assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[actix_web::test]
    async fn community_setup_key_cannot_create_a_session() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState::new(
                    config(&[("MEERKATEER_BOOTSTRAP_TOKEN", "fixture-admin-token")]),
                    None,
                )))
                .configure(configure_routes),
        )
        .await;
        let rejected = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/session/local-login")
                .insert_header(("Authorization", "Bearer incorrect-token"))
                .to_request(),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::NOT_FOUND);

        let accepted_auth = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/session/local-login")
                .insert_header(("Authorization", "Bearer fixture-admin-token"))
                .to_request(),
        )
        .await;
        assert_eq!(accepted_auth.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn authentication_rate_limit_runs_before_secret_verification() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState::new(
                    config(&[
                        ("MEERKATEER_BOOTSTRAP_TOKEN", "fixture-bootstrap-token"),
                        ("MEERKATEER_AUTH_RATE_LIMIT_PER_MINUTE", "2"),
                    ]),
                    None,
                )))
                .configure(configure_routes),
        )
        .await;
        let request_body = serde_json::json!({
            "tenant_slug": "arena-ops",
            "tenant_name": "Arena Ops",
            "owner_email": "owner@example.com",
            "owner_name": "Owner",
            "owner_password": "fixture-owner-password-123"
        });
        for expected in [
            StatusCode::UNAUTHORIZED,
            StatusCode::UNAUTHORIZED,
            StatusCode::TOO_MANY_REQUESTS,
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/bootstrap")
                    .peer_addr(std::net::SocketAddr::from(([192, 0, 2, 20], 4_242)))
                    .insert_header(("Authorization", "Bearer wrong-token"))
                    .set_json(&request_body)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), expected);
            if expected == StatusCode::TOO_MANY_REQUESTS {
                assert_eq!(
                    response.headers().get(header::RETRY_AFTER),
                    Some(&header::HeaderValue::from_static("60"))
                );
            }
        }
    }

    #[actix_web::test]
    async fn session_lookup_rejects_missing_and_malformed_cookies() {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState::new(config(&[]), None)))
                .configure(configure_routes),
        )
        .await;
        let missing = test::call_service(
            &app,
            test::TestRequest::get().uri("/v1/session").to_request(),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let malformed = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/session")
                .insert_header(("Cookie", "meerkateer_session=not-a-session"))
                .to_request(),
        )
        .await;
        assert_eq!(malformed.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn csrf_requires_matching_cookie_header_and_session_digest() {
        let tenant_id = uuid::Uuid::new_v4();
        let issued = IssuedSession::issue(tenant_id);
        let authenticated = AuthenticatedSession {
            session_id: issued.session_id,
            principal: Principal {
                user_id: uuid::Uuid::new_v4(),
                tenant_id,
                role: Role::Owner,
            },
            csrf_digest: issued.csrf_digest,
            email: "owner@example.com".to_owned(),
            display_name: "Owner".to_owned(),
        };
        let value = issued.csrf_secret.expose_secret();
        let valid = test::TestRequest::default()
            .cookie(Cookie::new("meerkateer_csrf", value.to_owned()))
            .insert_header(("X-Meerkateer-CSRF", value))
            .to_http_request();
        assert!(csrf_is_valid(&valid, &authenticated));

        let missing_header = test::TestRequest::default()
            .cookie(Cookie::new("meerkateer_csrf", value.to_owned()))
            .to_http_request();
        assert!(!csrf_is_valid(&missing_header, &authenticated));

        let mismatch = test::TestRequest::default()
            .cookie(Cookie::new("meerkateer_csrf", value.to_owned()))
            .insert_header(("X-Meerkateer-CSRF", "mks_csrf_wrong"))
            .to_http_request();
        assert!(!csrf_is_valid(&mismatch, &authenticated));
    }

    #[actix_web::test]
    async fn agent_snapshot_reports_process_failure_and_capacity() {
        let now = Utc::now();
        let rows = vec![
            (
                "agent.heartbeat".to_owned(),
                1.0,
                now,
                serde_json::json!({"os": "linux", "arch": "x86_64"}),
            ),
            (
                "host.cpu.utilization".to_owned(),
                0.25,
                now,
                serde_json::json!({}),
            ),
            (
                "host.memory.used_bytes".to_owned(),
                50.0,
                now,
                serde_json::json!({}),
            ),
            (
                "host.memory.total_bytes".to_owned(),
                100.0,
                now,
                serde_json::json!({}),
            ),
            (
                "host.disk.used_bytes".to_owned(),
                75.0,
                now,
                serde_json::json!({}),
            ),
            (
                "host.disk.total_bytes".to_owned(),
                100.0,
                now,
                serde_json::json!({}),
            ),
            (
                "host.disk.inodes_used".to_owned(),
                25.0,
                now,
                serde_json::json!({}),
            ),
            (
                "host.disk.inodes_total".to_owned(),
                100.0,
                now,
                serde_json::json!({}),
            ),
            (
                "process.running".to_owned(),
                0.0,
                now,
                serde_json::json!({"process": "game-server"}),
            ),
            (
                "os.service.running".to_owned(),
                0.0,
                now,
                serde_json::json!({"service": "minecraft.service", "state": "failed", "known": "true"}),
            ),
            (
                "os.service.running".to_owned(),
                0.0,
                now,
                serde_json::json!({"service": "private.service", "state": "permission_denied", "known": "false"}),
            ),
        ];
        let result = agent_telemetry_snapshot(
            uuid::Uuid::new_v4(),
            "active",
            Some(now),
            Some(now),
            rows,
            180,
        );
        assert_eq!(result.collection_state, "complete");
        assert!(!result.snapshot_stale);
        assert_eq!(result.cpu_usage_percent, Some(25.0));
        assert_eq!(
            result.memory.map(|value| value.utilization_percent),
            Some(50.0)
        );
        assert_eq!(
            result.disk.map(|value| value.utilization_percent),
            Some(75.0)
        );
        assert_eq!(
            result.inodes.map(|value| value.utilization_percent),
            Some(25.0)
        );
        assert_eq!(result.processes.len(), 1);
        assert!(!result.processes[0].running);
        assert_eq!(result.services.len(), 2);
        assert!(
            result
                .services
                .iter()
                .any(|service| service.name == "minecraft.service"
                    && service.running == Some(false)
                    && service.state == "failed")
        );
        assert!(
            result
                .services
                .iter()
                .any(|service| service.name == "private.service"
                    && service.running.is_none()
                    && service.state == "permission_denied")
        );
    }

    #[actix_web::test]
    async fn agent_snapshot_fails_partial_for_invalid_or_missing_metrics() {
        let now = Utc::now();
        let result = agent_telemetry_snapshot(
            uuid::Uuid::new_v4(),
            "active",
            Some(now),
            Some(now - ChronoDuration::seconds(181)),
            vec![
                (
                    "agent.heartbeat".to_owned(),
                    1.0,
                    now,
                    serde_json::json!({"os": "linux"}),
                ),
                (
                    "host.cpu.utilization".to_owned(),
                    4.0,
                    now,
                    serde_json::json!({}),
                ),
                (
                    "host.memory.used_bytes".to_owned(),
                    200.0,
                    now,
                    serde_json::json!({}),
                ),
                (
                    "host.memory.total_bytes".to_owned(),
                    100.0,
                    now,
                    serde_json::json!({}),
                ),
                (
                    "process.running".to_owned(),
                    -1.0,
                    now,
                    serde_json::json!({"process": "invalid"}),
                ),
            ],
            180,
        );
        assert_eq!(result.collection_state, "partial");
        assert!(result.snapshot_stale);
        assert!(result.cpu_usage_percent.is_none());
        assert!(result.memory.is_none());
        assert!(result.disk.is_none());
        assert!(result.processes.is_empty());
        assert!(result.missing_metrics.contains(&"host.cpu.utilization"));
    }
}
