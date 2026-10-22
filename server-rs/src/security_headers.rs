// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! The response headers `helmet()` sets in `server/index.js`, byte for byte,
//! including the Content-Security-Policy (S6) that allows Google Fonts.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

pub const CSP: &str = "default-src 'self';script-src 'self';style-src 'self' 'unsafe-inline' https://fonts.googleapis.com;img-src 'self' data: blob:;connect-src 'self';font-src 'self' data: https://fonts.gstatic.com;object-src 'none';frame-src 'none';worker-src 'self' blob:;base-uri 'self';form-action 'self';frame-ancestors 'self';script-src-attr 'none';upgrade-insecure-requests";

const HEADERS: &[(&str, &str)] = &[
    ("content-security-policy", CSP),
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-resource-policy", "same-origin"),
    ("origin-agent-cluster", "?1"),
    ("referrer-policy", "no-referrer"),
    (
        "strict-transport-security",
        "max-age=31536000; includeSubDomains",
    ),
    ("x-content-type-options", "nosniff"),
    ("x-dns-prefetch-control", "off"),
    ("x-download-options", "noopen"),
    ("x-frame-options", "SAMEORIGIN"),
    ("x-permitted-cross-domain-policies", "none"),
    ("x-xss-protection", "0"),
];

/// Middleware: adds the helmet headers to every response (unless a handler
/// already set one).
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    for (name, value) in HEADERS {
        h.entry(HeaderName::from_static(name))
            .or_insert(HeaderValue::from_static(value));
    }
    res
}
