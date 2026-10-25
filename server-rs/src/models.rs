// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Row types for the tenant tables, serialized exactly as the Node server sent
//! them (0/1 flags, `YYYY-MM-DD HH:MM:SS` timestamps, parsed JSON arrays).
//!
//! These also generate the client's API types (`ts-rs`): `cargo test` writes
//! them to `client/src/types/generated/` (see `.cargo/config.toml`).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use ts_rs::TS;

use crate::api_types::{IntakeStatus, InvoiceStatus, PaymentStatus, SessionStatus};
use crate::serde_util::{json_array, opt_bool_as_int, ts};

/// One `clients` row with its JSON columns parsed into arrays.
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export)]
pub struct Client {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub account_id: i64,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub household_type: Option<String>,
    pub adults: Option<i32>,
    pub children: Option<i32>,
    pub serving_size: Option<i32>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub allergens: Option<Value>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub dietary_styles: Option<Value>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub liked_proteins: Option<Value>,
    pub disliked_foods: Option<String>,
    pub spice_tolerance: Option<String>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1 | null")]
    pub oven_available: Option<bool>,
    pub stovetop_type: Option<String>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub equipment: Option<Value>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1 | null")]
    pub client_containers: Option<bool>,
    pub parking_notes: Option<String>,
    pub preferred_day: Option<String>,
    pub preferred_time: Option<String>,
    pub grocery_handling: Option<String>,
    pub chef_notes: Option<String>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub is_active: Option<bool>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub is_sample: Option<bool>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub updated_at: Option<DateTime<Utc>>,
}

/// One `recipes` row with `key_overlap_ingredients` parsed.
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export)]
pub struct RecipeRow {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub account_id: i64,
    pub name: String,
    pub category: Option<String>,
    pub session_fit: Option<String>,
    pub prep_time_min: Option<i32>,
    pub cook_time_min: Option<i32>,
    pub servings_base: Option<i32>,
    pub hold_quality: Option<i32>,
    pub reheat_quality: Option<i32>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub key_overlap_ingredients: Option<Value>,
    pub chef_notes: Option<String>,
    pub raw_instructions: Option<String>,
    pub import_source: Option<String>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub is_active: Option<bool>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub created_at: Option<DateTime<Utc>>,
}

impl RecipeRow {
    /// `prep_time_min + cook_time_min`, as the Node routes computed it.
    pub fn total_time(&self) -> i32 {
        self.prep_time_min.unwrap_or(0) + self.cook_time_min.unwrap_or(0)
    }
}

/// A recipe as the list route returns it (`total_time` added).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Recipe {
    #[serde(flatten)]
    #[ts(flatten)]
    pub row: RecipeRow,
    pub total_time: i32,
}

impl From<RecipeRow> for Recipe {
    fn from(row: RecipeRow) -> Self {
        Recipe {
            total_time: row.total_time(),
            row,
        }
    }
}

/// One `ingredients` row.
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export)]
pub struct Ingredient {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub recipe_id: Option<i64>,
    pub name: String,
    pub qty_4_serving: Option<String>,
    pub qty_6_serving: Option<String>,
    pub store_section: Option<String>,
    pub category: Option<String>,
    pub storage_note: Option<String>,
}

/// One `sessions` row.
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export)]
pub struct SessionRow {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub account_id: i64,
    #[ts(type = "number")]
    pub client_id: i64,
    #[ts(type = "string | null")]
    pub session_date: Option<NaiveDate>,
    pub session_time: Option<String>,
    pub serving_size: Option<i32>,
    #[ts(type = "number | null")]
    pub session_window_hours: Option<Decimal>,
    #[ts(as = "SessionStatus")]
    pub status: Option<String>,
    #[ts(type = "number")]
    pub service_fee: Option<Decimal>,
    #[ts(type = "number")]
    pub ingredient_cost: Option<Decimal>,
    pub chef_notes: Option<String>,
    pub template_name: Option<String>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub pdf_emailed_at: Option<DateTime<Utc>>,
    /// 1–8: completing the session books the next one this many weeks later.
    pub repeat_every_weeks: Option<i16>,
    /// The session this one was booked from by a repeat.
    #[ts(type = "number | null")]
    pub repeat_of: Option<i64>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub client_reminder_sent_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub chef_reminder_sent_at: Option<DateTime<Utc>>,
    /// Last failed meal-plan email; cleared by a successful one.
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub pdf_email_failed_at: Option<DateTime<Utc>>,
}

/// One `session_templates` row with `recipe_ids` parsed.
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export)]
pub struct SessionTemplateRow {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub account_id: i64,
    pub name: String,
    pub description: Option<String>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "number[]")]
    pub recipe_ids: Option<Value>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub is_default: Option<bool>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub created_at: Option<DateTime<Utc>>,
}

impl SessionTemplateRow {
    /// The stored recipe ids (non-integers ignored, like `safeJson`).
    pub fn ids(&self) -> Vec<i64> {
        match &self.recipe_ids {
            Some(Value::Array(items)) => items.iter().filter_map(crate::js::integer).collect(),
            _ => vec![],
        }
    }
}

/// One saved `grocery_items` row.
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export)]
pub struct GroceryItemRow {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number | null")]
    pub session_id: Option<i64>,
    #[ts(type = "string")]
    pub ingredient_name: Option<String>,
    #[ts(type = "string")]
    pub total_quantity: Option<String>,
    #[ts(type = "string")]
    pub store_section: Option<String>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub source_dishes: Option<Value>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub is_checked: Option<bool>,
}

/// One `client_intakes` row with its JSON columns parsed (`Intake` in the client).
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export, rename = "Intake")]
pub struct IntakeRow {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub account_id: i64,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub household_type: Option<String>,
    pub adults: Option<i32>,
    pub children: Option<i32>,
    pub preferred_serving_size: Option<i32>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub allergens: Option<Value>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub dietary_styles: Option<Value>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub liked_proteins: Option<Value>,
    pub disliked_foods: Option<String>,
    pub spice_tolerance: Option<String>,
    pub cuisine_preferences: Option<String>,
    pub preferred_day: Option<String>,
    pub preferred_time: Option<String>,
    pub scheduling_notes: Option<String>,
    pub kitchen_notes: Option<String>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1 | null")]
    pub client_containers: Option<bool>,
    pub grocery_handling: Option<String>,
    pub number_of_meals: Option<i32>,
    pub notes_for_chef: Option<String>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub terms_accepted: Option<bool>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub terms_accepted_at: Option<DateTime<Utc>>,
    #[ts(as = "IntakeStatus")]
    pub status: Option<String>,
    #[ts(as = "PaymentStatus")]
    pub payment_status: Option<String>,
    pub payment_notes: Option<String>,
    pub stripe_session_id: Option<String>,
    pub stripe_payment_intent: Option<String>,
    #[ts(type = "number | null")]
    pub booking_fee_amount: Option<Decimal>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub booking_fee_paid_at: Option<DateTime<Utc>>,
    pub pdf_path: Option<String>,
    pub internal_notes: Option<String>,
    pub source: Option<String>,
    #[ts(type = "number | null")]
    pub converted_client_id: Option<i64>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub updated_at: Option<DateTime<Utc>>,
    /// Last successful confirmation email to the client.
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub confirmation_emailed_at: Option<DateTime<Utc>>,
    /// Last failed confirmation email to the client; cleared by a successful one.
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub confirmation_email_failed_at: Option<DateTime<Utc>>,
    /// The new-booking alert to the chef failed (it is only sent once).
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub chef_alert_failed_at: Option<DateTime<Utc>>,
    /// Soft-deleted: hidden from the inbox and every action, kept for history.
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub deleted_at: Option<DateTime<Utc>>,
}

/// One `invoices` row with `recipes_summary` parsed.
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export)]
pub struct InvoiceRow {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub account_id: i64,
    #[ts(type = "number | null")]
    pub client_id: Option<i64>,
    #[ts(type = "number | null")]
    pub session_id: Option<i64>,
    pub invoice_number: Option<String>,
    #[ts(as = "InvoiceStatus")]
    pub status: Option<String>,
    #[ts(type = "string | null")]
    pub issue_date: Option<NaiveDate>,
    #[ts(type = "string | null")]
    pub due_date: Option<NaiveDate>,
    pub client_name: Option<String>,
    pub client_email: Option<String>,
    pub client_address: Option<String>,
    pub client_phone: Option<String>,
    pub business_name: Option<String>,
    pub chef_name: Option<String>,
    pub chef_email: Option<String>,
    pub chef_phone: Option<String>,
    pub message: Option<String>,
    pub internal_notes: Option<String>,
    #[ts(type = "number")]
    pub subtotal: Option<Decimal>,
    #[ts(type = "number")]
    pub tax_rate: Option<Decimal>,
    #[ts(type = "number")]
    pub tax_amount: Option<Decimal>,
    #[ts(type = "number")]
    pub total: Option<Decimal>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub paid_at: Option<DateTime<Utc>>,
    pub session_date_label: Option<String>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub recipes_summary: Option<Value>,
    pub session_start_time: Option<String>,
    pub session_end_time: Option<String>,
    #[ts(type = "number")]
    pub total_hours: Option<Decimal>,
    pub container_count: Option<i32>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub store_trip: Option<bool>,
    #[ts(type = "number")]
    pub grocery_total: Option<Decimal>,
    #[ts(type = "number")]
    pub service_fee: Option<Decimal>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub updated_at: Option<DateTime<Utc>>,
    /// Last successful invoice email.
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub emailed_at: Option<DateTime<Utc>>,
    /// Last failed invoice email; cleared by a successful one.
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub email_failed_at: Option<DateTime<Utc>>,
}

/// One `invoice_line_items` row.
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export)]
pub struct InvoiceLineItem {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub invoice_id: Option<i64>,
    #[ts(type = "number")]
    pub sort_order: Option<i32>,
    pub description: String,
    #[ts(type = "number")]
    pub quantity: Option<Decimal>,
    #[ts(type = "number")]
    pub unit_price: Option<Decimal>,
    #[ts(type = "number")]
    pub amount: Option<Decimal>,
}

/// `{ success: true }`.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Success {
    #[ts(type = "true")]
    pub success: bool,
}

pub const SUCCESS: Success = Success { success: true };
