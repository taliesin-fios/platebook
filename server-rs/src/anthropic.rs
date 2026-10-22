// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Minimal Claude Messages API client over reqwest (there is no official Rust
//! SDK). Used by recipe text import and the AI menu routes.
//!
//! Model defaults: Sonnet 5.5 (`claude-sonnet-5-5`) for parsing and menus,
//! Haiku 5.5 (`claude-haiku-5-5`) for the connection test; `ANTHROPIC_MODEL`
//! overrides all of them. Both 5.5 models think by default, which would spend
//! the small `max_tokens` budgets here, so `thinking_for` turns it off.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;

pub const API_VERSION: &str = "2023-06-01";
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-5";
pub const TEST_MODEL: &str = "claude-haiku-5-5";

/// Thinking setting that switches reasoning off for the models that think by
/// default. Sonnet 5.5 rejects `disabled` (it takes `between_tools`); Haiku 5.5
/// accepts it at the default effort. Other models (e.g. an `ANTHROPIC_MODEL`
/// override) are left alone.
fn thinking_for(model: &str) -> Option<serde_json::Value> {
    if model.starts_with("claude-sonnet-5-5") {
        Some(json!({ "type": "between_tools" }))
    } else if model.starts_with("claude-haiku-5-5") {
        Some(json!({ "type": "disabled" }))
    } else {
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClaudeError {
    #[error("Anthropic API key not configured")]
    NotConfigured,
    #[error("{message}")]
    Api { status: u16, message: String },
    #[error("Request timed out")]
    Timeout,
    #[error("{0}")]
    Transport(String),
    #[error("Unexpected response from the Anthropic API")]
    BadResponse,
}

impl ClaudeError {
    pub fn status(&self) -> Option<u16> {
        match self {
            ClaudeError::Api { status, .. } => Some(*status),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct MessageResponse {
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: String,
}

/// One single-turn request. Returns the text of the first text content block,
/// or `None` if the response has none.
pub async fn message(
    state: &AppState,
    default_model: &str,
    max_tokens: u32,
    system: Option<&str>,
    user: &str,
) -> Result<Option<String>, ClaudeError> {
    let config = &state.config;
    let key = config
        .anthropic_api_key
        .as_deref()
        .ok_or(ClaudeError::NotConfigured)?;
    let model = config.anthropic_model.as_deref().unwrap_or(default_model);
    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{ "role": "user", "content": user }],
    });
    if let Some(thinking) = thinking_for(model) {
        body["thinking"] = thinking;
    }
    if let Some(system) = system {
        body["system"] = json!(system);
    }

    let res = state
        .http
        .post(format!(
            "{}/v1/messages",
            config.anthropic_api_base.trim_end_matches('/')
        ))
        .header("x-api-key", key)
        .header("anthropic-version", API_VERSION)
        .timeout(Duration::from_secs(120))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                ClaudeError::Timeout
            } else {
                ClaudeError::Transport(e.to_string())
            }
        })?;

    let status = res.status();
    if !status.is_success() {
        let message = res
            .json::<ApiErrorBody>()
            .await
            .map(|b| b.error.message)
            .unwrap_or_else(|_| format!("Anthropic API returned HTTP {}", status.as_u16()));
        return Err(ClaudeError::Api {
            status: status.as_u16(),
            message: format!("{} {}", status.as_u16(), message),
        });
    }
    let parsed: MessageResponse = res.json().await.map_err(|_| ClaudeError::BadResponse)?;
    Ok(parsed
        .content
        .into_iter()
        .find(|b| b.kind == "text")
        .map(|b| b.text))
}
