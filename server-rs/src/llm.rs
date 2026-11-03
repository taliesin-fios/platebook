// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Minimal AI client over reqwest, used by recipe text import, AI menu
//! suggestions and the connection test.
//!
//! Each account picks its own provider, model and API key in Settings
//! (`settings.ai_*`, the key encrypted by [`crate::ai_keys`]). An account
//! that hasn't set one up falls back to the server's `ANTHROPIC_API_KEY`, if
//! there is one, as before.
//!
//! Two wire formats cover the providers:
//! - Anthropic's Messages API (`/v1/messages`);
//! - OpenAI-style Chat Completions (`/chat/completions`), which OpenAI,
//!   Google Gemini, OpenRouter, Mistral, Groq and most other services (and
//!   local model servers) accept. "Custom" is any of those at a chef-supplied
//!   base URL.
//!
//! Claude defaults: Sonnet 5.5 (`claude-sonnet-5-5`) for parsing and menus,
//! Haiku 5.5 (`claude-haiku-5-5`) for the connection test; a model chosen in
//! Settings, or `ANTHROPIC_MODEL` for the server key, overrides them. Other
//! providers need a model chosen in Settings. Both 5.5 models think by
//! default, which would spend the small `max_tokens` budgets here, so
//! `thinking_for` turns it off.
//!
//! A chef's own settings are called through `AppState::ai_http` (public
//! addresses only, no redirects) unless the operator sets
//! `AI_ALLOW_PRIVATE_URLS`.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;
use ts_rs::TS;
use url::Url;

use crate::state::AppState;

pub const API_VERSION: &str = "2023-06-01";
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-5";
pub const TEST_MODEL: &str = "claude-haiku-5-5";

const TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    OpenAi,
    Gemini,
    OpenRouter,
    Mistral,
    Groq,
    Custom,
}

impl Provider {
    pub const ALL: [Provider; 7] = [
        Provider::Anthropic,
        Provider::OpenAi,
        Provider::Gemini,
        Provider::OpenRouter,
        Provider::Mistral,
        Provider::Groq,
        Provider::Custom,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAi => "openai",
            Provider::Gemini => "gemini",
            Provider::OpenRouter => "openrouter",
            Provider::Mistral => "mistral",
            Provider::Groq => "groq",
            Provider::Custom => "custom",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic (Claude)",
            Provider::OpenAi => "OpenAI",
            Provider::Gemini => "Google Gemini",
            Provider::OpenRouter => "OpenRouter",
            Provider::Mistral => "Mistral",
            Provider::Groq => "Groq",
            Provider::Custom => "Custom (OpenAI-compatible)",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == s)
    }

    /// `None` for Custom, which needs a base URL.
    pub fn default_base_url(self) -> Option<&'static str> {
        match self {
            Provider::Anthropic => Some("https://api.anthropic.com"),
            Provider::OpenAi => Some("https://api.openai.com/v1"),
            Provider::Gemini => Some("https://generativelanguage.googleapis.com/v1beta/openai"),
            Provider::OpenRouter => Some("https://openrouter.ai/api/v1"),
            Provider::Mistral => Some("https://api.mistral.ai/v1"),
            Provider::Groq => Some("https://api.groq.com/openai/v1"),
            Provider::Custom => None,
        }
    }

    /// Claude has defaults; every other provider needs a model chosen.
    pub fn needs_model(self) -> bool {
        self != Provider::Anthropic
    }

    /// Only Custom may run without a key (a local model server).
    pub fn needs_key(self) -> bool {
        self != Provider::Custom
    }
}

/// One provider in `GET /api/ai/status`, for the Settings form.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ProviderInfo {
    pub id: String,
    pub label: String,
    pub default_base_url: Option<String>,
    pub needs_model: bool,
    pub needs_key: bool,
}

pub fn provider_list() -> Vec<ProviderInfo> {
    Provider::ALL
        .into_iter()
        .map(|p| ProviderInfo {
            id: p.id().into(),
            label: p.label().into(),
            default_base_url: p.default_base_url().map(Into::into),
            needs_model: p.needs_model(),
            needs_key: p.needs_key(),
        })
        .collect()
}

#[derive(Clone, Copy)]
pub enum Purpose {
    /// Recipe parsing and menu suggestions.
    Task,
    /// The connection test: a tiny request, the cheapest default model.
    Test,
}

/// The `settings.ai_*` columns.
#[derive(Debug, Default, sqlx::FromRow)]
pub struct AiSettings {
    pub ai_provider: Option<String>,
    pub ai_model: Option<String>,
    pub ai_base_url: Option<String>,
    pub ai_api_key_enc: Option<String>,
    pub ai_key_hint: Option<String>,
}

/// Reads this account's AI settings. Run it on a tenant transaction.
pub async fn load_settings(
    conn: &mut PgConnection,
    account_id: i64,
) -> Result<AiSettings, sqlx::Error> {
    Ok(sqlx::query_as::<_, AiSettings>(
        "SELECT ai_provider, ai_model, ai_base_url, ai_api_key_enc, ai_key_hint
         FROM settings WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_optional(conn)
    .await?
    .unwrap_or_default())
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Everything one request needs. The key never leaves the server.
pub struct AiConfig {
    pub provider: Provider,
    model: String,
    test_model: String,
    base_url: String,
    api_key: Option<String>,
    /// The chef's own settings (true) or the server's `ANTHROPIC_API_KEY`.
    pub own: bool,
}

/// The AI an account will use: its own settings, else the server's key.
pub fn resolve(state: &AppState, account_id: i64, s: &AiSettings) -> Result<AiConfig, AiError> {
    let config = &state.config;
    let Some(provider) = non_empty(&s.ai_provider).and_then(Provider::parse) else {
        let key = config
            .anthropic_api_key
            .clone()
            .ok_or(AiError::NotConfigured)?;
        return Ok(AiConfig {
            provider: Provider::Anthropic,
            model: config
                .anthropic_model
                .clone()
                .unwrap_or_else(|| DEFAULT_MODEL.into()),
            test_model: config
                .anthropic_model
                .clone()
                .unwrap_or_else(|| TEST_MODEL.into()),
            base_url: config.anthropic_api_base.clone(),
            api_key: Some(key),
            own: false,
        });
    };
    let api_key = match non_empty(&s.ai_api_key_enc) {
        Some(stored) => Some(
            crate::ai_keys::decrypt(&config.jwt_secret, account_id, stored)
                .ok_or(AiError::KeyUnreadable)?,
        ),
        None if provider.needs_key() => return Err(AiError::NotConfigured),
        None => None,
    };
    let base_url = non_empty(&s.ai_base_url)
        .or(provider.default_base_url())
        .ok_or(AiError::NotConfigured)?
        .to_string();
    let model = non_empty(&s.ai_model);
    if model.is_none() && provider.needs_model() {
        return Err(AiError::NotConfigured);
    }
    Ok(AiConfig {
        provider,
        model: model.unwrap_or(DEFAULT_MODEL).into(),
        test_model: model.unwrap_or(TEST_MODEL).into(),
        base_url,
        api_key,
        own: true,
    })
}

/// Checks a chef-supplied base URL. Must be HTTPS at a public address,
/// unless the operator allows private addresses. Returns it without a
/// trailing slash.
pub fn check_base_url(raw: &str, allow_private: bool) -> Result<String, &'static str> {
    let raw = raw.trim().trim_end_matches('/');
    if raw.len() > 300 {
        return Err("Base URL must be 300 characters or fewer");
    }
    let url = Url::parse(raw).map_err(|_| "Base URL isn't a valid address")?;
    let scheme_ok = match url.scheme() {
        "https" => true,
        "http" => allow_private,
        _ => false,
    };
    if !scheme_ok {
        return Err("Base URL must start with https://");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Base URL can't contain a user name or password");
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("Base URL can't have a query (?…) or fragment (#…)");
    }
    if !allow_private && crate::safe_fetch::is_url_blocked(raw) {
        return Err("Base URL must be a public address");
    }
    Ok(raw.to_string())
}

/// Thinking setting that switches reasoning off for the Claude models that
/// think by default. Sonnet 5.5 rejects `disabled` (it takes
/// `between_tools`); Haiku 5.5 accepts it at the default effort. Other models
/// are left alone.
fn thinking_for(model: &str) -> Option<Value> {
    if model.starts_with("claude-sonnet-5-5") {
        Some(json!({ "type": "between_tools" }))
    } else if model.starts_with("claude-haiku-5-5") {
        Some(json!({ "type": "disabled" }))
    } else {
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("AI isn't set up. Add a provider and API key in Settings → AI Assistant")]
    NotConfigured,
    #[error("Your saved AI key can't be read on this server. Enter it again in Settings")]
    KeyUnreadable,
    #[error("{message}")]
    Api { status: u16, message: String },
    #[error("Request timed out")]
    Timeout,
    #[error("The AI provider's address isn't allowed")]
    Disallowed,
    #[error("{0}")]
    Transport(String),
    #[error("Unexpected response from the AI provider")]
    BadResponse,
}

impl AiError {
    pub fn status(&self) -> Option<u16> {
        match self {
            AiError::Api { status, .. } => Some(*status),
            _ => None,
        }
    }
}

/// `{ "error": { "message" } }`, which Anthropic, OpenAI and most
/// OpenAI-compatible services return. Gemini wraps it in an array.
fn error_message(body: &Value) -> Option<String> {
    let body = body.as_array().and_then(|a| a.first()).unwrap_or(body);
    body.pointer("/error/message")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// One single-turn request. Returns the reply text, or `None` if the
/// response has none.
pub async fn message(
    state: &AppState,
    ai: &AiConfig,
    purpose: Purpose,
    max_tokens: u32,
    system: Option<&str>,
    user: &str,
) -> Result<Option<String>, AiError> {
    let model = match purpose {
        Purpose::Task => &ai.model,
        Purpose::Test => &ai.test_model,
    };
    let http = if ai.own && !state.config.ai_allow_private_urls {
        &state.ai_http
    } else {
        &state.http
    };
    let base = ai.base_url.trim_end_matches('/');
    let request = if ai.provider == Provider::Anthropic {
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
        http.post(format!("{base}/v1/messages"))
            .header("x-api-key", ai.api_key.as_deref().unwrap_or_default())
            .header("anthropic-version", API_VERSION)
            .json(&body)
    } else {
        let mut messages = Vec::new();
        if let Some(system) = system {
            messages.push(json!({ "role": "system", "content": system }));
        }
        messages.push(json!({ "role": "user", "content": user }));
        // OpenAI's current models refuse `max_tokens`; the rest still use it.
        let limit = if ai.provider == Provider::OpenAi {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        let mut body = json!({ "model": model, "messages": messages });
        body[limit] = json!(max_tokens);
        let req = http.post(format!("{base}/chat/completions")).json(&body);
        match &ai.api_key {
            Some(key) => req.bearer_auth(key),
            None => req,
        }
    };

    let res = request.timeout(TIMEOUT).send().await.map_err(|e| {
        if e.is_timeout() {
            AiError::Timeout
        } else if crate::safe_fetch::is_blocked_error(&e) {
            AiError::Disallowed
        } else {
            AiError::Transport(e.to_string())
        }
    })?;

    let status = res.status();
    if !status.is_success() {
        let message = res
            .json::<Value>()
            .await
            .ok()
            .as_ref()
            .and_then(error_message)
            .unwrap_or_else(|| {
                format!("{} returned HTTP {}", ai.provider.label(), status.as_u16())
            });
        return Err(AiError::Api {
            status: status.as_u16(),
            message: format!("{} {}", status.as_u16(), message),
        });
    }
    let body: Value = res.json().await.map_err(|_| AiError::BadResponse)?;
    Ok(if ai.provider == Provider::Anthropic {
        anthropic_text(&body)
    } else {
        chat_text(&body)
    })
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

/// The first text block of a Messages API response.
fn anthropic_text(body: &Value) -> Option<String> {
    let blocks: Vec<ContentBlock> = serde_json::from_value(body.get("content")?.clone()).ok()?;
    blocks
        .into_iter()
        .find(|b| b.kind == "text")
        .map(|b| b.text)
}

/// `choices[0].message.content`: a string, or (some services) a list of
/// text parts.
fn chat_text(body: &Value) -> Option<String> {
    match body.pointer("/choices/0/message/content")? {
        Value::String(s) => Some(s.clone()),
        Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect(),
        ),
        _ => None,
    }
}

/// The JSON in a reply: trimmed, with a Markdown code fence (```json … ```)
/// removed, since models other than Claude often add one.
pub fn json_text(text: &str) -> &str {
    let t = text.trim();
    let Some(rest) = t.strip_prefix("```") else {
        return t;
    };
    let rest = rest.split_once('\n').map_or("", |(_, body)| body);
    rest.trim_end().strip_suffix("```").unwrap_or(rest).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ids_round_trip() {
        for p in Provider::ALL {
            assert_eq!(Provider::parse(p.id()), Some(p));
        }
        assert_eq!(Provider::parse("nope"), None);
        assert!(Provider::Custom.default_base_url().is_none());
        assert!(!Provider::Anthropic.needs_model());
        assert!(!Provider::Custom.needs_key());
    }

    #[test]
    fn base_urls_must_be_public_https() {
        assert_eq!(
            check_base_url(" https://api.example.com/v1/ ", false),
            Ok("https://api.example.com/v1".into())
        );
        for bad in [
            "http://api.example.com/v1",
            "https://localhost:11434/v1",
            "https://127.0.0.1/v1",
            "https://10.0.0.5/v1",
            "https://169.254.169.254/latest",
            "https://[::1]/v1",
            "https://user:pw@api.example.com",
            "https://api.example.com/v1?x=1",
            "ftp://api.example.com",
            "not a url",
        ] {
            assert!(check_base_url(bad, false).is_err(), "{bad}");
        }
        // The operator can allow a model server on their own network.
        assert!(check_base_url("http://localhost:11434/v1", true).is_ok());
        assert!(check_base_url("http://192.168.1.20:1234/v1", true).is_ok());
        assert!(check_base_url("ftp://192.168.1.20", true).is_err());
    }

    #[test]
    fn json_text_strips_code_fences() {
        assert_eq!(json_text("  {\"a\":1} "), "{\"a\":1}");
        assert_eq!(json_text("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(json_text("```\n{\"a\":1}\n```\n"), "{\"a\":1}");
        assert_eq!(json_text("```json\n{\"a\":1}"), "{\"a\":1}");
    }

    #[test]
    fn reads_reply_text_from_both_formats() {
        let claude =
            json!({ "content": [{ "type": "thinking" }, { "type": "text", "text": "hi" }] });
        assert_eq!(anthropic_text(&claude).as_deref(), Some("hi"));
        let chat = json!({ "choices": [{ "message": { "content": "hi" } }] });
        assert_eq!(chat_text(&chat).as_deref(), Some("hi"));
        let parts = json!({ "choices": [{ "message": { "content": [{ "text": "h" }, { "text": "i" }] } }] });
        assert_eq!(chat_text(&parts).as_deref(), Some("hi"));
        assert_eq!(chat_text(&json!({ "choices": [] })), None);
    }

    #[test]
    fn reads_error_messages() {
        let one = json!({ "error": { "message": "bad key" } });
        assert_eq!(error_message(&one).as_deref(), Some("bad key"));
        assert_eq!(error_message(&json!([one])).as_deref(), Some("bad key"));
        assert_eq!(error_message(&json!({ "detail": "x" })), None);
    }
}
