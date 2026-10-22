// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/utils/intakeToken.js`: the HMAC token in a client's intake
//! edit link. Format `<issued_at_hex>.<hmac(id.issued_at)[..32]>`, valid for
//! 30 days (SEC-A3). Byte-compatible with the Node implementation, so links
//! already emailed to clients keep working after cutover.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

pub const MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;

fn signature(secret: &str, id: &str, issued_at_hex: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(format!("{id}.{issued_at_hex}").as_bytes());
    let mut hex = hex::encode(mac.finalize().into_bytes());
    hex.truncate(32);
    hex
}

/// A token for intake `id` issued at `now` (unix seconds).
pub fn make_token_at(secret: &str, id: &str, now: i64) -> String {
    let issued = format!("{now:x}");
    let sig = signature(secret, id, &issued);
    format!("{issued}.{sig}")
}

pub fn make_token(secret: &str, id: i64) -> String {
    make_token_at(secret, &id.to_string(), chrono::Utc::now().timestamp())
}

/// `parseInt(hex, 16)`: leading hex digits, `None` if there are none.
fn parse_hex_prefix(s: &str) -> Option<i64> {
    let s = s.trim_start();
    let end = s.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(s.len());
    i64::from_str_radix(&s[..end], 16).ok()
}

/// Verifies `token` for the intake id as it appears in the URL.
pub fn verify_token_at(secret: &str, id: &str, token: &str, now: i64) -> bool {
    let Some((issued_hex, sig)) = token.split_once('.') else {
        return false;
    };
    let Some(issued) = parse_hex_prefix(issued_hex) else {
        return false;
    };
    let age = now.saturating_sub(issued);
    if !(0..=MAX_AGE_SECS).contains(&age) {
        return false;
    }
    let expected = signature(secret, id, issued_hex);
    sig.len() == expected.len() && bool::from(sig.as_bytes().ct_eq(expected.as_bytes()))
}

pub fn verify_token(secret: &str, id: &str, token: &str) -> bool {
    verify_token_at(secret, id, token, chrono::Utc::now().timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: &str = "test-secret";
    const DAY: i64 = 24 * 60 * 60;

    // Same cases as server/test/intakeToken.test.js.
    #[test]
    fn fresh_token_verifies_for_its_id_only() {
        let t = make_token(S, 42);
        assert!(verify_token(S, "42", &t));
        assert!(!verify_token(S, "99", &t));
        assert!(!verify_token("other-secret", "42", &t));
    }

    #[test]
    fn tampered_legacy_and_empty_tokens_fail() {
        let t = make_token(S, 42);
        let issued = t.split('.').next().unwrap();
        assert!(!verify_token(
            S,
            "42",
            &format!("{issued}.{}", "0".repeat(32))
        ));
        assert!(!verify_token(S, "42", &"a".repeat(32)));
        assert!(!verify_token(S, "42", ""));
        assert!(!verify_token(S, "42", "zz.abc"));
    }

    #[test]
    fn expiry_window_and_future_tokens() {
        let now = chrono::Utc::now().timestamp();
        assert!(!verify_token_at(
            S,
            "42",
            &make_token_at(S, "42", now - 31 * DAY),
            now
        ));
        assert!(verify_token_at(
            S,
            "42",
            &make_token_at(S, "42", now - 29 * DAY),
            now
        ));
        assert!(!verify_token_at(
            S,
            "42",
            &make_token_at(S, "42", now + 3600),
            now
        ));
        assert_ne!(make_token_at(S, "1", now), make_token_at(S, "1", now + 5));
    }

    #[test]
    fn matches_node_crypto_output() {
        // node -e "c=require('crypto');c.createHmac('sha256','test-secret').update('42.6600a000').digest('hex').slice(0,32)"
        assert_eq!(
            make_token_at(S, "42", 0x6600a000),
            "6600a000.9de251f509caa9b3527da54f24d655dd"
        );
    }
}
