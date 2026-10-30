// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/utils/money.js` (DI17): money arithmetic in integer cents.
//!
//! Postgres stores dollar amounts as `NUMERIC(12,2)` (`Decimal` here) and
//! `deposit_cents` as integer cents. Rounding matches JavaScript's
//! `Math.round` (half rounds toward +infinity), but on exact decimals, so
//! inputs like 1.005 round the way they read instead of the way a binary
//! float happens to store them.

use rust_decimal::Decimal;

const HUNDRED: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

/// `Math.round(x)`: nearest integer, halves toward +infinity.
fn js_round(x: Decimal) -> Decimal {
    x.checked_add(Decimal::new(5, 1)).unwrap_or(x).floor()
}

/// The largest value of `x`'s sign, for results too big for a `Decimal`.
fn saturated(negative: bool) -> Decimal {
    if negative { Decimal::MIN } else { Decimal::MAX }
}

/// `a * b`, saturating instead of panicking (user-supplied amounts can be
/// absurd; validation rejects them, but arithmetic must never crash).
fn mul(a: Decimal, b: Decimal) -> Decimal {
    a.checked_mul(b)
        .unwrap_or_else(|| saturated(a.is_sign_negative() != b.is_sign_negative()))
}

fn decimal_to_i64(x: Decimal) -> i64 {
    i64::try_from(x).unwrap_or(if x.is_sign_negative() {
        i64::MIN
    } else {
        i64::MAX
    })
}

/// Dollars → integer cents.
pub fn to_cents(dollars: Decimal) -> i64 {
    decimal_to_i64(js_round(mul(dollars, HUNDRED)))
}

/// Integer cents → dollars.
pub fn from_cents(cents: i64) -> Decimal {
    Decimal::new(cents, 2)
}

/// Round to whole cents.
pub fn round_money(dollars: Decimal) -> Decimal {
    from_cents(to_cents(dollars))
}

/// Sum of amounts, each rounded to cents first.
pub fn add_money(values: &[Decimal]) -> Decimal {
    from_cents(
        values
            .iter()
            .fold(0i64, |sum, v| sum.saturating_add(to_cents(*v))),
    )
}

pub fn subtract_money(a: Decimal, b: Decimal) -> Decimal {
    from_cents(to_cents(a).saturating_sub(to_cents(b)))
}

/// `amount * factor`, rounded to cents.
pub fn multiply_money(amount: Decimal, factor: Decimal) -> Decimal {
    let cents = mul(Decimal::from(to_cents(amount)), factor);
    from_cents(decimal_to_i64(js_round(cents)))
}

/// `pct` percent of `amount`, rounded to cents.
pub fn pct_of_money(amount: Decimal, pct: Decimal) -> Decimal {
    let cents = mul(Decimal::from(to_cents(amount)), pct) / HUNDRED;
    from_cents(decimal_to_i64(js_round(cents)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    // Same cases as server/test/money.test.js where they apply.
    #[test]
    fn huge_amounts_saturate_instead_of_panicking() {
        let huge = d("1e27");
        assert_eq!(to_cents(huge), i64::MAX);
        assert_eq!(to_cents(-huge), i64::MIN);
        let max = from_cents(i64::MAX);
        assert_eq!(add_money(&[max, max]), max);
        assert_eq!(subtract_money(-max, max), from_cents(i64::MIN));
        assert_eq!(pct_of_money(max, d("1e20")), max);
        assert_eq!(multiply_money(max, d("1e20")), max);
    }

    #[test]
    fn cents_round_trip() {
        assert_eq!(to_cents(d("12.34")), 1234);
        assert_eq!(to_cents(d("0")), 0);
        assert_eq!(from_cents(1234), d("12.34"));
        assert_eq!(round_money(d("10.005")), d("10.01"));
    }

    #[test]
    fn halves_round_toward_positive_infinity_like_math_round() {
        assert_eq!(to_cents(d("0.125")), 13);
        assert_eq!(to_cents(d("-0.125")), -12);
    }

    #[test]
    fn add_avoids_float_drift() {
        assert_eq!(add_money(&[d("0.1"), d("0.2")]), d("0.3"));
        assert_eq!(add_money(&[]), d("0"));
    }

    #[test]
    fn subtract() {
        assert_eq!(subtract_money(d("10.00"), d("3.33")), d("6.67"));
    }

    #[test]
    fn multiply_and_percent() {
        assert_eq!(multiply_money(d("19.99"), d("3")), d("59.97"));
        assert_eq!(multiply_money(d("10.00"), d("0.333")), d("3.33"));
        assert_eq!(pct_of_money(d("100.00"), d("8.25")), d("8.25"));
        assert_eq!(pct_of_money(d("33.33"), d("7.5")), d("2.50"));
    }
}
