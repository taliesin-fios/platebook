// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Serde helpers that keep the JSON API identical to the SQLite-backed Node
//! server while the database uses proper types.

/// Serialize a BOOLEAN column as `0` / `1`, which is what SQLite returned and
/// what the client compares against (`intake.client_containers === 1`).
/// Use as `#[serde(with = "crate::serde_util::bool_as_int")]`.
pub mod bool_as_int {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &bool, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(u8::from(*value))
    }

    /// Accepts `true`/`false`, numbers (non-zero is true) and `"0"`/`"1"`.
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
        Ok(match serde_json::Value::deserialize(d)? {
            serde_json::Value::Bool(b) => b,
            serde_json::Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
            serde_json::Value::String(s) => !matches!(s.trim(), "" | "0" | "false"),
            _ => false,
        })
    }
}

/// `Option<bool>` variant of [`bool_as_int`] (`null` stays `null`).
pub mod opt_bool_as_int {
    use serde::Serializer;

    pub fn serialize<S: Serializer>(value: &Option<bool>, s: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(b) => s.serialize_u8(u8::from(*b)),
            None => s.serialize_none(),
        }
    }
}

/// `TIMESTAMPTZ` as SQLite's `datetime('now')` text, `YYYY-MM-DD HH:MM:SS`
/// (UTC), which the client formats with `fmtUtcTimestamp`.
/// Use as `#[serde(serialize_with = "crate::serde_util::ts::serialize")]`.
pub mod ts {
    use chrono::{DateTime, Utc};
    use serde::Serializer;

    pub const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

    pub fn format(value: &DateTime<Utc>) -> String {
        value.format(FORMAT).to_string()
    }

    pub fn serialize<S: Serializer>(
        value: &Option<DateTime<Utc>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(t) => s.serialize_str(&format(t)),
            None => s.serialize_none(),
        }
    }
}

/// A JSONB array column the Node routes returned parsed, with `NULL` as `[]`
/// (`JSON.parse(row.col || '[]')`).
pub mod json_array {
    use serde::{Serialize, Serializer};
    use serde_json::Value;

    pub fn serialize<S: Serializer>(value: &Option<Value>, s: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(v) if !v.is_null() => v.serialize(s),
            _ => s
                .serialize_seq(Some(0))
                .and_then(serde::ser::SerializeSeq::end),
        }
    }
}

/// A JSONB array column the Node routes returned as the raw JSON text
/// (e.g. `client_allergens` in the session list), `NULL` stays `null`.
pub mod json_text {
    use serde::Serializer;
    use serde_json::Value;

    pub fn serialize<S: Serializer>(value: &Option<Value>, s: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(v) if !v.is_null() => s.serialize_str(&v.to_string()),
            _ => s.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Row {
        #[serde(with = "super::bool_as_int")]
        flag: bool,
        #[serde(serialize_with = "super::opt_bool_as_int::serialize")]
        maybe: Option<bool>,
    }

    #[test]
    fn timestamps_and_json_columns_match_sqlite_text() {
        use chrono::TimeZone;
        #[derive(Serialize)]
        struct T {
            #[serde(serialize_with = "super::ts::serialize")]
            at: Option<chrono::DateTime<chrono::Utc>>,
            #[serde(serialize_with = "super::json_array::serialize")]
            arr: Option<serde_json::Value>,
            #[serde(serialize_with = "super::json_text::serialize")]
            raw: Option<serde_json::Value>,
        }
        let at = chrono::Utc.with_ymd_and_hms(2026, 3, 4, 5, 6, 7).unwrap();
        let json = serde_json::to_string(&T {
            at: Some(at),
            arr: None,
            raw: Some(serde_json::json!(["Nuts"])),
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"at":"2026-03-04 05:06:07","arr":[],"raw":"[\"Nuts\"]"}"#
        );
    }

    #[test]
    fn booleans_serialize_as_sqlite_integers() {
        let json = serde_json::to_string(&Row {
            flag: true,
            maybe: None,
        })
        .unwrap();
        assert_eq!(json, r#"{"flag":1,"maybe":null}"#);
        let row: Row = serde_json::from_str(r#"{"flag":0,"maybe":null}"#).unwrap();
        assert!(!row.flag);
        let row: Row = serde_json::from_str(r#"{"flag":"1","maybe":null}"#).unwrap();
        assert!(row.flag);
    }
}
