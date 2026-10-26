// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/services/stripeProvider.js`: Stripe Checkout for booking
//! deposits and the webhook handler. Only used when `PAYMENTS_MODE=stripe`.
//!
//! Talks to the Stripe REST API with reqwest (form-encoded, like the official
//! SDKs) and verifies webhook signatures exactly as `stripe.webhooks.constructEvent`
//! does: HMAC-SHA256 of `"{t}.{raw body}"` with the endpoint secret, any `v1`
//! signature may match, 5-minute timestamp tolerance.

use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::state::AppState;

pub const SIGNATURE_TOLERANCE_SECS: i64 = 300;

#[derive(Debug, thiserror::Error)]
pub enum StripeError {
    #[error(
        "STRIPE_SECRET_KEY is not set. Add it to the environment or switch to PAYMENTS_MODE=manual."
    )]
    NotConfigured,
    #[error("{0}")]
    Api(String),
    #[error("{0}")]
    Transport(String),
}

#[derive(Debug, Deserialize)]
pub struct CheckoutSession {
    pub id: String,
    pub url: Option<String>,
    /// `open`, `complete` or `expired`.
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

/// Inputs for [`create_checkout_session`]: a booking deposit or an invoice.
pub struct CheckoutRequest<'a> {
    /// The line item's name ("Booking Fee", "Invoice INV-0003").
    pub product_name: String,
    pub description: String,
    pub amount_cents: i64,
    pub customer_email: Option<&'a str>,
    /// Copied to the session's and the PaymentIntent's metadata:
    /// payment_intent.* and charge.* events only carry the PaymentIntent's,
    /// and the webhook needs to know what each event pays for
    /// (`intake_id` or `invoice_id`).
    pub metadata: Vec<(&'static str, String)>,
    /// Where Stripe sends the client back to: their own page with its token
    /// (`/intake/:id?token=…`, `/invoice/:id?token=…`). `&paid=1` or
    /// `&cancelled=1` is added, so the page can say what happened.
    pub return_path: String,
}

pub async fn create_checkout_session(
    state: &AppState,
    r: CheckoutRequest<'_>,
) -> Result<CheckoutSession, StripeError> {
    let key = state
        .config
        .stripe_secret_key
        .as_deref()
        .ok_or(StripeError::NotConfigured)?;
    let client_url = &state.config.client_url;
    let mut form: Vec<(String, String)> = vec![
        ("mode".into(), "payment".into()),
        ("payment_method_types[0]".into(), "card".into()),
        ("line_items[0][price_data][currency]".into(), "usd".into()),
        (
            "line_items[0][price_data][product_data][name]".into(),
            r.product_name,
        ),
        (
            "line_items[0][price_data][product_data][description]".into(),
            r.description,
        ),
        (
            "line_items[0][price_data][unit_amount]".into(),
            r.amount_cents.to_string(),
        ),
        ("line_items[0][quantity]".into(), "1".into()),
        (
            "success_url".into(),
            format!("{client_url}{}&paid=1", r.return_path),
        ),
        (
            "cancel_url".into(),
            format!("{client_url}{}&cancelled=1", r.return_path),
        ),
    ];
    for (k, v) in &r.metadata {
        form.push((format!("metadata[{k}]"), v.clone()));
        form.push((format!("payment_intent_data[metadata][{k}]"), v.clone()));
    }
    if let Some(email) = r.customer_email.filter(|e| !e.is_empty()) {
        form.push(("customer_email".into(), email.to_string()));
    }

    let res = state
        .http
        .post(format!(
            "{}/v1/checkout/sessions",
            state.config.stripe_api_base.trim_end_matches('/')
        ))
        .bearer_auth(key)
        .form(&form)
        .send()
        .await
        .map_err(|e| StripeError::Transport(e.to_string()))?;
    if !res.status().is_success() {
        let status = res.status();
        let msg = res
            .json::<ErrorBody>()
            .await
            .map(|b| b.error.message)
            .unwrap_or_else(|_| format!("Stripe returned HTTP {}", status.as_u16()));
        return Err(StripeError::Api(msg));
    }
    res.json::<CheckoutSession>()
        .await
        .map_err(|e| StripeError::Transport(e.to_string()))
}

async fn checkout_session_call(
    state: &AppState,
    req: reqwest::RequestBuilder,
) -> Result<CheckoutSession, StripeError> {
    let key = state
        .config
        .stripe_secret_key
        .as_deref()
        .ok_or(StripeError::NotConfigured)?;
    let res = req
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| StripeError::Transport(e.to_string()))?;
    if !res.status().is_success() {
        let status = res.status();
        let msg = res
            .json::<ErrorBody>()
            .await
            .map(|b| b.error.message)
            .unwrap_or_else(|_| format!("Stripe returned HTTP {}", status.as_u16()));
        return Err(StripeError::Api(msg));
    }
    res.json::<CheckoutSession>()
        .await
        .map_err(|e| StripeError::Transport(e.to_string()))
}

fn session_url(state: &AppState, id: &str, suffix: &str) -> String {
    format!(
        "{}/v1/checkout/sessions/{id}{suffix}",
        state.config.stripe_api_base.trim_end_matches('/')
    )
}

/// GET /v1/checkout/sessions/:id: is an earlier checkout still open, paid
/// (`complete`) or `expired`?
pub async fn retrieve_checkout_session(
    state: &AppState,
    id: &str,
) -> Result<CheckoutSession, StripeError> {
    checkout_session_call(state, state.http.get(session_url(state, id, ""))).await
}

/// POST /v1/checkout/sessions/:id/expire: closes an open checkout so it can't
/// be paid after a new one replaces it.
pub async fn expire_checkout_session(
    state: &AppState,
    id: &str,
) -> Result<CheckoutSession, StripeError> {
    checkout_session_call(state, state.http.post(session_url(state, id, "/expire"))).await
}

/// Why a webhook signature was rejected (the text goes in the 400 body).
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    #[error("No stripe-signature header value was provided.")]
    Missing,
    #[error("Unable to extract timestamp and signatures from header")]
    Malformed,
    #[error("No signatures found matching the expected signature for payload.")]
    NoMatch,
    #[error("Timestamp outside the tolerance zone")]
    Expired,
}

/// The `v1` signature Stripe computes for `payload` at `timestamp`.
pub fn sign(secret: &str, timestamp: i64, payload: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

/// Verifies a `Stripe-Signature` header against the raw request body.
pub fn verify_signature(
    payload: &[u8],
    header: Option<&str>,
    secret: &str,
    now: i64,
) -> Result<(), SignatureError> {
    let header = header
        .filter(|h| !h.is_empty())
        .ok_or(SignatureError::Missing)?;
    let mut timestamp: Option<i64> = None;
    let mut signatures: Vec<&str> = Vec::new();
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", v)) => timestamp = v.parse().ok(),
            Some(("v1", v)) => signatures.push(v),
            _ => {}
        }
    }
    let timestamp = timestamp.ok_or(SignatureError::Malformed)?;
    if signatures.is_empty() {
        return Err(SignatureError::NoMatch);
    }
    let expected = sign(secret, timestamp, payload);
    let matched = signatures
        .iter()
        .any(|s| s.len() == expected.len() && bool::from(s.as_bytes().ct_eq(expected.as_bytes())));
    if !matched {
        return Err(SignatureError::NoMatch);
    }
    if (now - timestamp).abs() > SIGNATURE_TOLERANCE_SECS {
        return Err(SignatureError::Expired);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_test_secret";

    #[test]
    fn accepts_a_valid_signature_among_several() {
        let body = br#"{"id":"evt_1"}"#;
        let t = 1_800_000_000;
        let header = format!("t={t},v1=deadbeef,v1={}", sign(SECRET, t, body));
        assert_eq!(
            verify_signature(body, Some(&header), SECRET, t + 10),
            Ok(())
        );
    }

    #[test]
    fn rejects_bad_missing_tampered_and_stale() {
        let body = br#"{"id":"evt_1"}"#;
        let t = 1_800_000_000;
        let good = format!("t={t},v1={}", sign(SECRET, t, body));
        assert_eq!(
            verify_signature(body, None, SECRET, t),
            Err(SignatureError::Missing)
        );
        assert_eq!(
            verify_signature(body, Some("v1=abc"), SECRET, t),
            Err(SignatureError::Malformed)
        );
        assert_eq!(
            verify_signature(b"{\"id\":\"evt_2\"}", Some(&good), SECRET, t),
            Err(SignatureError::NoMatch)
        );
        assert_eq!(
            verify_signature(body, Some(&good), "whsec_other", t),
            Err(SignatureError::NoMatch)
        );
        assert_eq!(
            verify_signature(body, Some(&good), SECRET, t + 301),
            Err(SignatureError::Expired)
        );
    }

    #[test]
    fn accepts_a_header_from_the_official_sdk() {
        // stripe-node: webhooks.generateTestHeaderString({ payload: '{"id":"evt_1"}',
        //   secret: 'whsec_test_secret', timestamp: 1800000000 })
        let header =
            "t=1800000000,v1=8a6563dbee1eccda632491deb091d19e764aadddcd0ace8d9516599526831c89";
        assert_eq!(
            verify_signature(br#"{"id":"evt_1"}"#, Some(header), SECRET, 1_800_000_000),
            Ok(())
        );
    }
}
