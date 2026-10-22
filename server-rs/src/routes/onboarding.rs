// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/onboarding.js` (all routes require auth).
//!
//! POST /api/onboarding/seed      — { starter_library?: bool, sample_client?: bool }
//! POST /api/onboarding/complete  — marks onboarding finished

use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::Tenant;
use crate::error::DbResult;
use crate::models::{SUCCESS, Success};
use crate::routes::JsonBody;
use crate::seed;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/seed", post(seed_account))
        .route("/complete", post(complete))
}

/// Counts of starter content added.
#[derive(Debug, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SeedLibrary {
    pub recipes: u32,
    pub templates: u32,
}

/// POST /api/onboarding/seed.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct OnboardingSeedResponse {
    pub library: SeedLibrary,
    pub sample_client: bool,
}

async fn seed_account(mut t: Tenant, body: JsonBody) -> DbResult<Json<OnboardingSeedResponse>> {
    let id = t.account.id;
    let library = if body.truthy("starter_library") {
        seed::seed_starter_library(&mut t.tx, id).await?
    } else {
        SeedLibrary::default()
    };
    let sample_client =
        body.truthy("sample_client") && seed::seed_sample_client(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(OnboardingSeedResponse {
        library,
        sample_client,
    }))
}

async fn complete(mut t: Tenant) -> DbResult<Json<Success>> {
    sqlx::query(
        "UPDATE accounts SET onboarding_completed_at = COALESCE(onboarding_completed_at, now())
         WHERE id = $1",
    )
    .bind(t.account.id)
    .execute(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok(Json(SUCCESS))
}
