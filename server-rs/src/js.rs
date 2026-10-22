// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! JavaScript value semantics for request bodies.
//!
//! The Node routes read `req.body` loosely (`adults || 2`, `Number(x)`,
//! `!!flag`). These helpers reproduce those coercions so the Rust routes
//! accept exactly the inputs the Express server accepted.

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use serde_json::Value;

/// JS truthiness: `null`, `false`, `0`, `NaN` and `""` are falsy.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `Number(v)`: numbers as-is, numeric strings parsed (blank is 0), booleans
/// 0/1, `null` 0, anything else `NaN`.
pub fn number(v: &Value) -> f64 {
    match v {
        Value::Null => 0.0,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                0.0
            } else {
                t.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        Value::Array(_) | Value::Object(_) => f64::NAN,
    }
}

/// `parseInt(v, 10)`: leading integer of the string form, else `None` (NaN).
pub fn parse_int(v: &Value) -> Option<i64> {
    let s = match v {
        Value::Number(n) => {
            return n
                .as_f64()
                .filter(|f| f.is_finite())
                .map(|f| f.trunc() as i64);
        }
        Value::String(s) => s.trim_start(),
        Value::Bool(_) | Value::Null | Value::Array(_) | Value::Object(_) => return None,
    };
    let (sign, digits) = match s.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, s.strip_prefix('+').unwrap_or(s)),
    };
    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    digits[..end].parse::<i64>().ok().map(|n| sign * n)
}

/// `parseFloat(v)`: leading decimal number of the string form.
pub fn parse_float(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => parse_float_str(s),
        _ => None,
    }
}

/// `parseFloat(s)` on a string.
pub fn parse_float_str(s: &str) -> Option<f64> {
    let s = s.trim_start();
    let mut end = 0;
    let mut seen_dot = false;
    let mut seen_digit = false;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            seen_digit = true;
            end = i + 1;
        } else if c == '.' && !seen_dot {
            seen_dot = true;
        } else if !((c == '-' || c == '+') && i == 0) {
            break;
        }
    }
    if !seen_digit {
        return None;
    }
    s[..end].parse::<f64>().ok()
}

/// A finite `f64` as a `Decimal` (for NUMERIC columns).
pub fn decimal(f: f64) -> Option<Decimal> {
    if f.is_finite() {
        Decimal::from_f64(f).map(|d| d.round_dp(6).normalize())
    } else {
        None
    }
}

/// `String(v)` for scalars (used when Node bound a raw value into a TEXT column).
pub fn string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        other => other.to_string(),
    }
}

/// An integer-valued JS number (`Number.isInteger(Number(v))`).
pub fn integer(v: &Value) -> Option<i64> {
    let f = number(v);
    (f.is_finite() && f.fract() == 0.0).then_some(f as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn truthiness() {
        for v in [json!(null), json!(false), json!(0), json!("")] {
            assert!(!truthy(&v));
        }
        for v in [json!(1), json!("0"), json!([]), json!({})] {
            assert!(truthy(&v));
        }
    }

    #[test]
    fn number_and_parse() {
        assert_eq!(number(&json!(" 12 ")), 12.0);
        assert!(number(&json!("12a")).is_nan());
        assert_eq!(parse_int(&json!("12a")), Some(12));
        assert_eq!(parse_int(&json!("-3 days")), Some(-3));
        assert_eq!(parse_int(&json!("x")), None);
        assert_eq!(parse_float(&json!("2.5 cups")), Some(2.5));
        assert_eq!(parse_float(&json!("abc")), None);
        assert_eq!(integer(&json!("4")), Some(4));
        assert_eq!(integer(&json!(4.5)), None);
    }
}
