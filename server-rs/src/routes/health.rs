// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

use axum::Json;
use serde_json::{Value, json};

/// `GET /health` — liveness for Railway, same response as the Node server.
pub async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}
