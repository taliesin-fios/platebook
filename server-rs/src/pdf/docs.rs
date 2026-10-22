// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! The data each PDF template reads: every string formatted here, the way
//! `client/src/utils/pdfGen.ts` and the invoice preview format them.

use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use serde_json::{Value, json};

use super::{APP_NAME, Asset, logo_asset};
use crate::accounts::Settings;
use crate::models::{GroceryItemRow, InvoiceLineItem};
use crate::money;
use crate::routes::costs::js_num;
use crate::routes::grocery::GeneratedGroceryItem;
use crate::routes::invoices::InvoiceDetail;
use crate::routes::planner::PlannerResponse;
use crate::routes::sessions::SessionDetail;

/// "Sunday, March 15, 2026" (`EEEE, MMMM d, yyyy`).
pub fn long_date(d: Option<NaiveDate>) -> String {
    d.map(|d| d.format("%A, %B %-d, %Y").to_string())
        .unwrap_or_default()
}

/// "March 15, 2026" (`MMMM d, yyyy`).
pub fn month_day_year(d: Option<NaiveDate>) -> String {
    d.map(|d| d.format("%B %-d, %Y").to_string())
        .unwrap_or_default()
}

/// `$1234.50`: integer-cents rounding, two decimals, like `toFixed(2)`.
pub fn usd(d: Option<Decimal>) -> String {
    format!(
        "${}",
        money::from_cents(money::to_cents(d.unwrap_or_default()))
    )
}

fn business_name(settings: &Settings) -> String {
    settings
        .business_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| APP_NAME.to_string())
}

fn website(settings: &Settings) -> String {
    settings.website.clone().unwrap_or_default()
}

/// `name.replace(/\s+/g, '-').toLowerCase()`, for filenames.
pub fn slug(name: Option<&str>, fallback: &str) -> String {
    let s = name
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase();
    if s.is_empty() {
        fallback.to_string()
    } else {
        s
    }
}

// ── Reheat helpers (port of client/src/constants.ts) ─────────────────────────

/// `getReheatedInstructions(name)`.
pub fn reheat_instructions(name: &str) -> &'static str {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["curry", "massaman", "panang"]) {
        "Stovetop, medium-low, splash of water if needed, 5–8 min"
    } else if has(&["ziti", "lasagna", "casserole", "gratin"]) {
        "Oven 325°F covered 20 min, or microwave covered 2–3 min"
    } else if has(&["meatball"]) {
        "Stovetop, medium-low, 5–8 min, or microwave 2 min"
    } else if has(&["bowl"]) {
        "Warm meatballs separately, serve over cold grain"
    } else {
        "Stovetop, medium-low heat, stir occasionally, 5–8 min"
    }
}

/// `getReheatedCategory(name)`: "stovetop" | "oven" | "cold".
pub fn reheat_category(name: &str) -> &'static str {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&[
        "salad",
        "sashimi",
        "ceviche",
        "gazpacho",
        "crudo",
        "tartare",
        "carpaccio",
    ]) {
        "cold"
    } else if has(&[
        "lasagna",
        "ziti",
        "casserole",
        "gratin",
        "bake",
        "roast",
        "frittata",
        "quiche",
        "enchilada",
        "pot pie",
    ]) {
        "oven"
    } else {
        "stovetop"
    }
}

/// `isFreezerFriendly(name)`: most cooked meals freeze well.
pub fn freezer_friendly(name: &str) -> bool {
    let n = name.to_lowercase();
    ![
        "salad",
        "sashimi",
        "ceviche",
        "gazpacho",
        "crudo",
        "tartare",
        "carpaccio",
        "raw ",
        "fresh herb",
    ]
    .iter()
    .any(|w| n.contains(w))
}

fn serves(serving_size: Option<i32>) -> i32 {
    serving_size.filter(|n| *n != 0).unwrap_or(4)
}

fn string_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| match i {
                Value::String(s) => Some(s.clone()),
                Value::Null => None,
                other => Some(other.to_string()),
            })
            .collect(),
        // Older rows can still carry a JSON array as text.
        Some(Value::String(s)) => string_list(serde_json::from_str::<Value>(s).ok().as_ref()),
        _ => vec![],
    }
}

// ── Meal plan (client PDF) ────────────────────────────────────────────────────

pub fn meal_plan(session: &SessionDetail, settings: &Settings) -> Value {
    let date = session.session.session_date;
    let serving = serves(session.session.serving_size);
    let use_by = date
        .map(|d| (d + Duration::days(4)).format("%b %-d").to_string())
        .unwrap_or_default();
    let dishes: Vec<Value> = session
        .dishes
        .iter()
        .map(|d| {
            let name = &d.recipe.name;
            json!({
                "name": name,
                "reheat": reheat_instructions(name),
                "category": reheat_category(name),
                "freeze": freezer_friendly(name),
            })
        })
        .collect();
    json!({
        "business": business_name(settings).to_uppercase(),
        "client": session.client_name.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "Client".into()),
        "date": long_date(date),
        "serves": serving,
        "use_by": use_by,
        "dishes": dishes,
        "allergens": string_list(session.client_allergens.as_ref()),
        "website": website(settings),
    })
}

pub fn meal_plan_filename(session: &SessionDetail) -> String {
    format!(
        "meal-plan-{}-{}.pdf",
        slug(session.client_name.as_deref(), "client"),
        session
            .session
            .session_date
            .map(|d| d.to_string())
            .unwrap_or_else(|| "session".into())
    )
}

// ── Grocery list ──────────────────────────────────────────────────────────────

/// The PDF's store-section order; anything else is grouped under "Other".
const GROCERY_SECTIONS: &[&str] = &[
    "Produce",
    "Meat & Seafood",
    "Dairy & Eggs",
    "Pantry & Dry Goods",
    "Canned & Jarred",
    "Herbs & Spices",
    "Other",
];

/// One grocery row, stored or generated on the fly.
pub struct GroceryLine {
    pub name: String,
    pub quantity: String,
    pub section: String,
    pub sources: Vec<String>,
}

impl From<GroceryItemRow> for GroceryLine {
    fn from(r: GroceryItemRow) -> Self {
        GroceryLine {
            name: r.ingredient_name.unwrap_or_default(),
            quantity: r.total_quantity.unwrap_or_default(),
            section: r.store_section.unwrap_or_default(),
            sources: string_list(r.source_dishes.as_ref()),
        }
    }
}

impl From<GeneratedGroceryItem> for GroceryLine {
    fn from(r: GeneratedGroceryItem) -> Self {
        GroceryLine {
            name: r.ingredient_name,
            quantity: r.total_quantity,
            section: r.store_section,
            sources: r.source_dishes,
        }
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

pub fn grocery(session: &SessionDetail, items: Vec<GroceryLine>, settings: &Settings) -> Value {
    let mut sections: Vec<(String, Vec<GroceryLine>)> = GROCERY_SECTIONS
        .iter()
        .map(|s| (s.to_string(), vec![]))
        .collect();
    for item in items {
        let idx = GROCERY_SECTIONS
            .iter()
            .position(|s| *s == item.section)
            .unwrap_or(GROCERY_SECTIONS.len() - 1);
        sections[idx].1.push(item);
    }
    let sections: Vec<Value> = sections
        .into_iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(name, items)| {
            json!({
                "name": name,
                "count": plural(items.len(), "item"),
                "items": items.iter().map(|i| json!({
                    "name": i.name,
                    "quantity": i.quantity,
                    "sources": i.sources.join(" · "),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "business": business_name(settings).to_uppercase(),
        "client": session.client_name.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "Client".into()),
        "date": long_date(session.session.session_date),
        "serves": serves(session.session.serving_size),
        "notes": session.session.chef_notes.clone().unwrap_or_default(),
        "sections": sections,
        "website": website(settings),
    })
}

pub fn grocery_filename(session: &SessionDetail) -> String {
    format!(
        "grocery-{}-{}.pdf",
        slug(session.client_name.as_deref(), "client"),
        session
            .session
            .session_date
            .map(|d| d.to_string())
            .unwrap_or_else(|| "session".into())
    )
}

// ── Planner ingredient list ───────────────────────────────────────────────────

pub fn planner(
    plan: &PlannerResponse,
    serving_size: f64,
    today: NaiveDate,
    settings: &Settings,
) -> Value {
    let names = plan.recipe_names.clone().unwrap_or_default();
    let sections: Vec<Value> = plan
        .sections
        .iter()
        .map(|s| {
            json!({
                "name": s.section,
                "count": plural(s.items.len(), "item"),
                "items": s.items.iter().map(|i| json!({
                    "name": i.name,
                    "quantity": i.quantity,
                    "multi": i.multi_source,
                    "sources": i.source_dishes.join(" · "),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "business": business_name(settings).to_uppercase(),
        "date": month_day_year(Some(today)),
        "serves": js_num(Decimal::try_from(serving_size).unwrap_or(Decimal::from(4))),
        "recipes": if names.is_empty() { String::new() } else { format!("Recipes: {}", names.join(", ")) },
        "summary": format!(
            "{} · {}",
            plural(plan.total_items, "unique ingredient"),
            plural(names.len(), "recipe"),
        ),
        "sections": sections,
        "website": website(settings),
    })
}

// ── Invoice ───────────────────────────────────────────────────────────────────

fn opt(s: &Option<String>) -> String {
    s.clone().unwrap_or_default()
}

fn line_item(li: &InvoiceLineItem) -> Option<Value> {
    let amount = li.amount.unwrap_or_default();
    if li.description.is_empty() && amount.is_zero() {
        return None;
    }
    Some(json!({
        "description": if li.description.is_empty() { "—".to_string() } else { li.description.clone() },
        "quantity": li.quantity.map(js_num).unwrap_or_default(),
        "price": usd(li.unit_price),
        "amount": usd(li.amount),
    }))
}

/// The invoice data plus the chef's logo, if one is set.
pub fn invoice(inv: &InvoiceDetail, settings: &Settings) -> (Value, Option<Asset>) {
    let i = &inv.invoice;
    let business = i
        .business_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| settings.business_name.clone())
        .unwrap_or_default();
    let tax_rate = i.tax_rate.unwrap_or_default();
    let logo = logo_asset(settings.logo_base64.as_deref());
    let data = json!({
        "logo": logo.as_ref().map(|a| a.path),
        "business": business,
        "chef_name": opt(&i.chef_name),
        "chef_email": opt(&i.chef_email),
        "chef_phone": opt(&i.chef_phone),
        "number": i.invoice_number.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "DRAFT".into()),
        "issued": month_day_year(i.issue_date),
        "due": month_day_year(i.due_date),
        "client_name": opt(&i.client_name),
        "client_email": opt(&i.client_email),
        "client_phone": opt(&i.client_phone),
        "client_address": opt(&i.client_address),
        "items": inv.line_items.iter().filter_map(line_item).collect::<Vec<_>>(),
        "subtotal": usd(i.subtotal),
        "tax": if tax_rate > Decimal::ZERO {
            json!({ "label": format!("Tax ({}%)", js_num(tax_rate)), "amount": usd(i.tax_amount) })
        } else {
            Value::Null
        },
        "total": usd(i.total),
        "message": opt(&i.message),
    });
    (data, logo)
}

pub fn invoice_filename(inv: &InvoiceDetail) -> String {
    format!(
        "invoice-{}.pdf",
        inv.invoice
            .invoice_number
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("INV-{}", inv.invoice.id))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reheat_matches_client_constants() {
        assert_eq!(reheat_category("Beef Lasagna"), "oven");
        assert_eq!(reheat_category("Greek Salad"), "cold");
        assert_eq!(reheat_category("Chicken Stew"), "stovetop");
        assert!(reheat_instructions("Thai Panang Curry").contains("splash of water"));
        assert!(reheat_instructions("Baked Ziti").starts_with("Oven 325°F"));
        assert!(!freezer_friendly("Caesar Salad"));
        assert!(freezer_friendly("Chili"));
    }

    #[test]
    fn formats_like_the_client() {
        let d = NaiveDate::from_ymd_opt(2026, 3, 15);
        assert_eq!(long_date(d), "Sunday, March 15, 2026");
        assert_eq!(month_day_year(d), "March 15, 2026");
        assert_eq!(usd(Some(Decimal::new(12345, 1))), "$1234.50");
        assert_eq!(usd(None), "$0.00");
        assert_eq!(slug(Some("Ann  Lee"), "client"), "ann-lee");
        assert_eq!(slug(None, "client"), "client");
    }
}
