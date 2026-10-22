// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Router assembly, mirroring the middleware order in `server/index.js`:
//! /health first (no auth), then security headers (helmet), CORS, body
//! limit, `requireAuth`, routes, a JSON 404 for unknown `/api/*` paths and,
//! when `CLIENT_DIST` is set (production), the React build with an
//! `index.html` fallback for client-side routes.

use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Router, middleware};
use tower::ServiceExt;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::auth::require_auth;
use crate::error::AppError;
use crate::routes;
use crate::state::AppState;

/// `express.json({ limit: '10mb' })`
const BODY_LIMIT: usize = 10 * 1024 * 1024;

pub fn build(state: AppState) -> Router {
    crate::config::set_production(state.config.production);

    let origins: Vec<HeaderValue> = state
        .config
        .allowed_origins()
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([header::CONTENT_TYPE]);

    let api = Router::new()
        .nest("/api/auth", routes::auth::router())
        .nest("/api/clients", routes::clients::router())
        .nest("/api/templates", routes::templates::router())
        .nest("/api/settings", routes::settings::router())
        .nest("/api/public", routes::public::router())
        .nest("/api/onboarding", routes::onboarding::router())
        .nest("/api/sessions", routes::sessions::router())
        .nest("/api/grocery", routes::grocery::item_router())
        .nest("/api/costs", routes::costs::router())
        .nest("/api/planner", routes::planner::router())
        .nest("/api/recipes", routes::recipes::router())
        .nest("/api/ai", routes::ai::router())
        .nest("/api/intake", routes::intake::router())
        .nest("/api/invoices", routes::invoices::router())
        .nest("/api/payments", routes::payments::router())
        .fallback(fallback)
        .layer(middleware::from_fn_with_state(state.clone(), require_auth))
        .layer(axum::extract::DefaultBodyLimit::max(BODY_LIMIT))
        .layer(cors)
        .layer(middleware::from_fn(
            crate::security_headers::security_headers,
        ))
        .with_state(state);

    // Registered outside every layer so Railway can always reach it.
    Router::new()
        .route("/health", get(routes::health::health))
        .merge(api)
        .layer(TraceLayer::new_for_http())
}

/// Unknown `/api/*` paths get a JSON 404; anything else is the React app
/// (static file, else `index.html`) when a build directory is configured.
async fn fallback(State(state): State<AppState>, req: Request) -> Response {
    let path = req.uri().path();
    if path == "/api" || path.starts_with("/api/") {
        return AppError::not_found().into_response();
    }
    match &state.config.client_dist {
        Some(dist) => {
            let index = std::path::Path::new(dist).join("index.html");
            match ServeDir::new(dist)
                .fallback(ServeFile::new(index))
                .oneshot(req)
                .await
            {
                Ok(res) => res.into_response(),
                Err(err) => AppError::internal(err).into_response(),
            }
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
