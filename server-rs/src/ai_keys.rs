// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Encryption at rest for each account's AI API key (`settings.ai_api_key_enc`).
//!
//! XChaCha20-Poly1305 with a random 24-byte nonce. The key is derived from
//! `JWT_SECRET` (HMAC-SHA256 with a fixed label), so there is no extra secret
//! to configure; changing `JWT_SECRET` makes saved keys unreadable, and chefs
//! then re-enter theirs. The account id is bound in as associated data, so a
//! stored value copied to another account's row doesn't decrypt.
//!
//! Stored form: `v1:` + base64(nonce ‖ ciphertext).

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use sha2::Sha256;

const PREFIX: &str = "v1:";
const NONCE_LEN: usize = 24;

fn cipher(secret: &str) -> XChaCha20Poly1305 {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(b"platebook ai-api-key v1");
    XChaCha20Poly1305::new(&mac.finalize().into_bytes())
}

pub fn encrypt(secret: &str, account_id: i64, plaintext: &str) -> String {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let aad = account_id.to_be_bytes();
    let ciphertext = cipher(secret)
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext.as_bytes(),
                aad: &aad,
            },
        )
        .expect("XChaCha20-Poly1305 encryption doesn't fail for small inputs");
    let mut raw = nonce.to_vec();
    raw.extend(ciphertext);
    format!("{PREFIX}{}", STANDARD.encode(raw))
}

/// `None` when the value is malformed, belongs to another account, or was
/// written under a different `JWT_SECRET`.
pub fn decrypt(secret: &str, account_id: i64, stored: &str) -> Option<String> {
    let raw = STANDARD.decode(stored.strip_prefix(PREFIX)?).ok()?;
    if raw.len() <= NONCE_LEN {
        return None;
    }
    let (nonce, ciphertext) = raw.split_at(NONCE_LEN);
    let aad = account_id.to_be_bytes();
    let plaintext = cipher(secret)
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .ok()?;
    String::from_utf8(plaintext).ok()
}

/// The last four characters, shown in Settings so a chef can tell keys apart.
pub fn hint(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    chars[chars.len().saturating_sub(4)..].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let stored = encrypt("secret", 7, "sk-test-1234");
        assert!(stored.starts_with("v1:"));
        assert!(!stored.contains("sk-test"));
        assert_eq!(
            decrypt("secret", 7, &stored).as_deref(),
            Some("sk-test-1234")
        );
    }

    #[test]
    fn each_encryption_uses_a_fresh_nonce() {
        assert_ne!(encrypt("secret", 7, "k"), encrypt("secret", 7, "k"));
    }

    #[test]
    fn refuses_another_account_secret_or_tampering() {
        let stored = encrypt("secret", 7, "sk-test-1234");
        assert_eq!(decrypt("secret", 8, &stored), None);
        assert_eq!(decrypt("other", 7, &stored), None);
        let mut raw = STANDARD.decode(&stored[3..]).unwrap();
        *raw.last_mut().unwrap() ^= 1;
        assert_eq!(
            decrypt("secret", 7, &format!("v1:{}", STANDARD.encode(raw))),
            None
        );
        assert_eq!(decrypt("secret", 7, "sk-plain"), None);
        assert_eq!(decrypt("secret", 7, "v1:AAAA"), None);
    }

    #[test]
    fn hint_is_the_last_four_characters() {
        assert_eq!(hint("sk-ant-abcd1234"), "1234");
        assert_eq!(hint("ab"), "ab");
        assert_eq!(hint(""), "");
    }
}
