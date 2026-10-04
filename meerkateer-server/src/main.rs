use std::sync::Arc;

use actix_web::{App, HttpServer, middleware::DefaultHeaders, web};
use anyhow::Context;
use meerkateer_config::ServerConfig;
use meerkateer_server::{
    AppState, configure_routes, graceful_shutdown_timeout, run_scheduled_probe_loop,
};
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Arc::new(ServerConfig::load().context("invalid server configuration")?);
    let bind_address = config.bind_address;
    let database = if let Some(url) = &config.database_url {
        Some(
            PgPoolOptions::new()
                .max_connections(config.database_max_connections)
                .acquire_timeout(std::time::Duration::from_secs(5))
                .connect(url.expose_secret())
                .await
                .context("failed to connect to the control database")?,
        )
    } else {
        None
    };
    let state = web::Data::new(AppState::new(Arc::clone(&config), database));

    info!(
        %bind_address,
        deployment_mode = config.deployment_mode.as_str(),
        service = %config.service_name,
        environment = %config.environment,
        "starting Meerkateer server"
    );

    let scheduler = tokio::spawn(run_scheduled_probe_loop(state.get_ref().clone()));
    let server_result = HttpServer::new(move || {
        App::new()
            .wrap(
                DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("Referrer-Policy", "no-referrer"))
                    .add((
                        "Permissions-Policy",
                        "camera=(), microphone=(), geolocation=()",
                    )),
            )
            .app_data(state.clone())
            .configure(configure_routes)
    })
    .shutdown_timeout(graceful_shutdown_timeout().as_secs())
    .bind(bind_address)
    .with_context(|| format!("failed to bind {bind_address}"))?
    .run()
    .await;
    scheduler.abort();
    server_result.context("server stopped with an error")
}
