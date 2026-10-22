// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

use std::net::SocketAddr;

use chef_server::config::Config;
use chef_server::state::AppState;
use chef_server::{app, db};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info")),
        )
        .init();

    let config = Config::from_env()?;
    tracing::info!(?config, "starting");

    let pool = db::connect(&config.database_url).await?;
    db::MIGRATOR.run(&pool).await?;

    // S9: check SMTP credentials early (non-blocking), then the backup timer.
    let smtp = config.clone();
    tokio::spawn(async move { chef_server::email::verify_transporter(&smtp).await });
    chef_server::backup::start_scheduler(&config);
    start_pruning(pool.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = AppState::new(pool, config);
    chef_server::reminders::start(state.clone());
    let router = app::build(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// Hourly: drop expired rate-limit windows and used one-time-token ids, so
/// those tables stay small. Several instances doing this at once is harmless.
fn start_pruning(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
        loop {
            tick.tick().await;
            if let Err(err) = chef_server::rate_limit::prune(&pool).await {
                tracing::warn!(%err, "pruning rate_limit_hits failed");
            }
            if let Err(err) = chef_server::auth::prune_used_jtis(&pool).await {
                tracing::warn!(%err, "pruning used_jtis failed");
            }
        }
    });
}

/// Ctrl-C, or SIGTERM (what Railway sends on redeploy), lets in-flight
/// requests finish before the process exits.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
    tracing::info!("shutting down");
}
