// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! The public demo account (`DEMO_MODE=true`).
//!
//! The server keeps one account, [`DEMO_EMAIL`], that anyone can open with
//! "Try the demo" (`POST /api/auth/demo`). It has no password, so nobody can
//! sign in to it any other way. On start, and then every `DEMO_RESET_HOURS`,
//! its data is wiped and reseeded: a starter recipe library, a handful of
//! clients, past and upcoming sessions with grocery lists, invoices in each
//! state, and booking inquiries. Dates are relative to the reset day, so the
//! dashboard always has a current week.
//!
//! While signed in to the demo, nothing leaves the server: client emails are
//! refused ([`refuse`]) or dropped (`email::send`), and AI keys can't be saved.
//! The demo account doesn't count as an account for `SIGNUP_MODE=first`.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use axum::http::StatusCode;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::{PgConnection, PgPool};

use crate::accounts;
use crate::db;
use crate::error::AppError;
use crate::routes::grocery::generate_grocery_list;
use crate::seed;
use crate::state::AppState;

/// The demo account's login email. `.invalid` is reserved (RFC 2606), so
/// mail to it can never be delivered anywhere.
pub const DEMO_EMAIL: &str = "demo@platebook.invalid";

/// Business name on the demo's settings, booking page and invoices. Also the
/// base of its booking slug (`/book/demo-kitchen`).
const DEMO_BUSINESS: &str = "Demo Kitchen";

/// Which account is the demo, and when it resets next (both 0 = unknown).
/// Set by [`reset`]; read by the auth routes and the email guards.
#[derive(Debug, Default)]
pub struct DemoState {
    account_id: AtomicI64,
    next_reset_unix: AtomicI64,
}

impl DemoState {
    /// The demo account's id, once the first reset has run.
    pub fn account_id(&self) -> Option<i64> {
        Some(self.account_id.load(Ordering::Relaxed)).filter(|id| *id > 0)
    }

    pub fn is_demo(&self, account_id: i64) -> bool {
        self.account_id() == Some(account_id)
    }

    /// When the scheduler resets the demo next.
    pub fn next_reset(&self) -> Option<DateTime<Utc>> {
        let ts = self.next_reset_unix.load(Ordering::Relaxed);
        (ts > 0).then(|| DateTime::from_timestamp(ts, 0)).flatten()
    }

    fn set(&self, account_id: i64, next_reset: DateTime<Utc>) {
        self.account_id.store(account_id, Ordering::Relaxed);
        self.next_reset_unix
            .store(next_reset.timestamp(), Ordering::Relaxed);
    }
}

/// 403 for something the demo account may not do (send email, save an AI
/// key). `what` finishes the sentence "… is turned off in the demo".
pub fn refuse(state: &AppState, account_id: i64, what: &str) -> Result<(), AppError> {
    if state.demo.is_demo(account_id) {
        return Err(AppError::new(
            StatusCode::FORBIDDEN,
            "DEMO_DISABLED",
            format!("{what} is turned off in the demo, so nothing leaves this server."),
        ));
    }
    Ok(())
}

/// With `DEMO_MODE=true`: resets the demo now (so "Try the demo" works as
/// soon as the server listens), then every `DEMO_RESET_HOURS`.
pub async fn start(state: AppState) {
    if !state.config.demo.enabled {
        return;
    }
    let every = Duration::from_secs(u64::from(state.config.demo.reset_hours) * 60 * 60);
    run_reset(&state, every).await;
    tokio::spawn(async move {
        let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + every, every);
        loop {
            tick.tick().await;
            run_reset(&state, every).await;
        }
    });
}

async fn run_reset(state: &AppState, every: Duration) {
    match reset(&state.pool).await {
        Ok(id) => {
            let next = Utc::now() + every;
            state.demo.set(id, next);
            tracing::info!(account_id = id, next_reset = %next, "demo account reset");
        }
        // Keep serving; the next tick tries again.
        Err(err) => tracing::error!(%err, "demo account reset failed"),
    }
}

/// Creates the demo account if needed, wipes its data and reseeds it, in one
/// transaction. Returns its id. Safe to run from several instances at once.
pub async fn reset(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('platebook.demo.reset'))")
        .execute(&mut *tx)
        .await?;
    let id = match accounts::find_by_email(&mut tx, DEMO_EMAIL).await? {
        Some(a) => a.id,
        None => {
            accounts::create_account(&mut tx, DEMO_EMAIL, None, DEMO_BUSINESS)
                .await?
                .id
        }
    };
    // No password: "Try the demo" is the only way in.
    sqlx::query(
        "UPDATE accounts SET password_hash = NULL,
           onboarding_completed_at = COALESCE(onboarding_completed_at, now())
         WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    db::enter_tenant(&mut tx, id).await?;
    wipe(&mut tx, id).await?;
    seed_demo(&mut tx, id).await?;
    tx.commit().await?;
    Ok(id)
}

/// Deletes every row the account owns (children go with their parents),
/// in foreign-key order.
async fn wipe(conn: &mut PgConnection, id: i64) -> Result<(), sqlx::Error> {
    for table in [
        "payment_events",
        "invoices",
        "recipe_import_log",
        "client_intakes",
        "sessions",
        "session_templates",
        "recipes",
        "clients",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE account_id = $1"))
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

struct DemoClient {
    name: &'static str,
    email: &'static str,
    phone: &'static str,
    address: &'static str,
    household_type: &'static str,
    adults: i32,
    children: i32,
    serving_size: i32,
    allergens: &'static [&'static str],
    dietary_styles: &'static [&'static str],
    liked_proteins: &'static [&'static str],
    disliked_foods: &'static str,
    spice_tolerance: &'static str,
    preferred_day: &'static str,
    preferred_time: &'static str,
    parking_notes: &'static str,
    chef_notes: &'static str,
    /// Service fee and typical grocery spend per session, in dollars.
    fee: i64,
    groceries: i64,
    hours: Decimal,
    /// Menus this client's sessions rotate through (starter recipe names).
    menus: &'static [&'static [&'static str]],
}

const CHEN: usize = 0;
const PATEL: usize = 1;
const JOHNSON: usize = 2;
const OKAFOR: usize = 3;
const GOLDBERG: usize = 4;

fn clients() -> [DemoClient; 5] {
    [
        DemoClient {
            name: "The Chen Family",
            email: "chen.family@example.com",
            phone: "(503) 555-0142",
            address: "2218 NE Alameda St, Portland, OR",
            household_type: "Small Family",
            adults: 2,
            children: 2,
            serving_size: 6,
            allergens: &["Tree Nuts"],
            dietary_styles: &["Dairy-Free"],
            liked_proteins: &["Chicken", "Beef"],
            disliked_foods: "Olives, very spicy food",
            spice_tolerance: "Mild",
            preferred_day: "Sunday",
            preferred_time: "Morning",
            parking_notes: "Driveway is fine; side door code 4417",
            chef_notes: "Two young kids, mild spice only. Label containers with reheat times.",
            fee: 325,
            groceries: 128,
            hours: Decimal::new(35, 1),
            menus: &[
                &[
                    "Lemon-Herb Roast Chicken Thighs",
                    "Beef and Vegetable Stew",
                    "Chicken Tortilla Soup",
                ],
                &[
                    "Turkey and Black Bean Chili",
                    "Chicken and Rice Casserole",
                    "Red Lentil Soup",
                ],
                &[
                    "Pork Carnitas",
                    "Braised Short Ribs",
                    "Lemon-Herb Roast Chicken Thighs",
                ],
            ],
        },
        DemoClient {
            name: "Priya & Sam Patel",
            email: "patels@example.com",
            phone: "(503) 555-0187",
            address: "915 SE Clinton St, Portland, OR",
            household_type: "Couple",
            adults: 2,
            children: 0,
            serving_size: 4,
            allergens: &["Eggs"],
            dietary_styles: &["Vegetarian"],
            liked_proteins: &["Plant-Based Only"],
            disliked_foods: "Mushrooms (Sam)",
            spice_tolerance: "Bold",
            preferred_day: "Wednesday",
            preferred_time: "Afternoon",
            parking_notes: "Street parking only",
            chef_notes: "Love Indian and Mexican flavors. Extra lunches when possible.",
            fee: 260,
            groceries: 84,
            hours: Decimal::new(30, 1),
            menus: &[
                &[
                    "Chickpea and Spinach Coconut Curry",
                    "Black Bean and Sweet Potato Enchiladas",
                    "Mediterranean Quinoa Salad",
                ],
                &[
                    "Red Lentil Soup",
                    "Roasted Vegetable Lasagna",
                    "Chickpea and Spinach Coconut Curry",
                ],
            ],
        },
        DemoClient {
            name: "Marcus Johnson",
            email: "marcus.j@example.com",
            phone: "(503) 555-0110",
            address: "4410 N Williams Ave, Apt 6, Portland, OR",
            household_type: "Individual",
            adults: 1,
            children: 0,
            serving_size: 4,
            allergens: &["Shellfish"],
            dietary_styles: &["Omnivore"],
            liked_proteins: &["Chicken", "Beef", "Pork"],
            disliked_foods: "",
            spice_tolerance: "Medium",
            preferred_day: "Monday",
            preferred_time: "Morning",
            parking_notes: "Visitor spot B; buzz unit 6",
            chef_notes: "Training for a marathon: high-protein, portions for lunch and dinner.",
            fee: 280,
            groceries: 112,
            hours: Decimal::new(30, 1),
            menus: &[
                &[
                    "Herb-Crusted Pork Tenderloin",
                    "Chicken Tikka-Style Curry",
                    "Turkey and Black Bean Chili",
                ],
                &[
                    "Braised Short Ribs",
                    "Lemon-Herb Roast Chicken Thighs",
                    "Chicken Tortilla Soup",
                ],
            ],
        },
        DemoClient {
            name: "The Okafor Household",
            email: "okafor.home@example.com",
            phone: "(503) 555-0163",
            address: "7720 SW Capitol Hwy, Portland, OR",
            household_type: "Large Family",
            adults: 2,
            children: 3,
            serving_size: 6,
            allergens: &["Gluten", "Peanuts"],
            dietary_styles: &["Gluten-Free"],
            liked_proteins: &["Chicken", "Beef", "Pork"],
            disliked_foods: "Cilantro",
            spice_tolerance: "Medium",
            preferred_day: "Thursday",
            preferred_time: "Morning",
            parking_notes: "Park on the street; the driveway is steep",
            chef_notes: "Celiac in the family: separate boards, no shared fryer oil.",
            fee: 340,
            groceries: 146,
            hours: Decimal::new(40, 1),
            menus: &[
                &[
                    "Pork Carnitas",
                    "Chicken Tortilla Soup",
                    "Beef and Vegetable Stew",
                ],
                &[
                    "Chicken and Rice Casserole",
                    "Turkey and Black Bean Chili",
                    "Lemon-Herb Roast Chicken Thighs",
                ],
            ],
        },
        DemoClient {
            name: "Ruth & David Goldberg",
            email: "goldbergs@example.com",
            phone: "(503) 555-0129",
            address: "3105 NW Thurman St, Portland, OR",
            household_type: "Couple",
            adults: 2,
            children: 0,
            serving_size: 4,
            allergens: &["Sesame"],
            dietary_styles: &["Pescatarian", "Mediterranean"],
            liked_proteins: &["Fish"],
            disliked_foods: "Cilantro",
            spice_tolerance: "Mild",
            preferred_day: "Friday",
            preferred_time: "Late morning",
            parking_notes: "Garage spot 2 is free on Fridays",
            chef_notes: "New clients from the booking page. Lower-sodium where possible.",
            fee: 240,
            groceries: 96,
            hours: Decimal::new(30, 1),
            menus: &[&[
                "Baked Salmon with Dill Yogurt Sauce",
                "Red Lentil Soup",
                "Mediterranean Quinoa Salad",
            ]],
        },
    ]
}

/// One session on the demo calendar, `days` from the reset day.
struct Plan {
    client: usize,
    days: i64,
    time: &'static str,
    status: &'static str,
    /// Override the client's menu rotation.
    menu: Option<&'static [&'static str]>,
    notes: &'static str,
    invoice: Option<Inv>,
}

#[derive(Clone, Copy)]
enum Inv {
    Paid,
    /// Paid this morning, so "collected this month" is never empty.
    PaidToday,
    /// Sent, due this many days after the session.
    Sent(i64),
    Draft,
}

fn plans() -> Vec<Plan> {
    let mut v = Vec::new();
    let past = |client, days, invoice| Plan {
        client,
        days,
        time: "10:00",
        status: "Completed",
        menu: None,
        notes: "",
        invoice,
    };
    // Weekly regulars, biweekly couples: about two months of history.
    for w in (2..=9).rev() {
        v.push(past(CHEN, -7 * w, Some(Inv::Paid)));
    }
    v.push(past(CHEN, -7, Some(Inv::PaidToday)));
    for d in [-60, -46, -32, -18] {
        v.push(past(PATEL, d, Some(Inv::Paid)));
    }
    v.push(past(PATEL, -4, Some(Inv::Sent(14))));
    for d in [-40, -26] {
        v.push(past(JOHNSON, d, Some(Inv::Paid)));
    }
    // Overdue: due two days ago.
    v.push(past(JOHNSON, -12, Some(Inv::Sent(10))));
    for d in [-44, -30] {
        v.push(past(OKAFOR, d, Some(Inv::Paid)));
    }
    v.push(Plan {
        status: "Cancelled",
        notes: "Family away for spring break.",
        ..past(OKAFOR, -16, None)
    });
    v.push(past(OKAFOR, -2, Some(Inv::Draft)));

    // This week and the next two.
    let up = |client, days, time, status| Plan {
        client,
        days,
        time,
        status,
        menu: None,
        notes: "",
        invoice: None,
    };
    v.push(up(CHEN, 0, "10:00", "Confirmed"));
    v.push(Plan {
        menu: Some(&[
            "Shrimp and Vegetable Fried Rice",
            "Herb-Crusted Pork Tenderloin",
            "Turkey and Black Bean Chili",
        ]),
        notes: "Shrimp fried rice was requested by his partner. Check the allergy warning and swap the shrimp for chicken.",
        ..up(JOHNSON, 1, "09:30", "Pending")
    });
    v.push(up(PATEL, 2, "13:00", "Confirmed"));
    v.push(up(OKAFOR, 5, "10:00", "Confirmed"));
    v.push(up(CHEN, 7, "10:00", "Pending"));
    v.push(Plan {
        notes: "First session. Bring extra containers.",
        ..up(GOLDBERG, 9, "11:00", "Pending")
    });
    v.push(up(PATEL, 16, "13:00", "Pending"));
    // In date order, so invoice numbers run in date order too.
    v.sort_by_key(|p| p.days);
    v
}

/// `d` moved by `days` (negative = earlier).
fn shift(d: NaiveDate, days: i64) -> NaiveDate {
    d + chrono::TimeDelta::days(days)
}

/// "Sunday, March 15, 2026", as invoices label their session date.
fn date_label(d: NaiveDate) -> String {
    d.format("%A, %B %-d, %Y").to_string()
}

/// "10:00" plus 3.5 hours → "13:30".
fn end_time(start: &str, hours: Decimal) -> String {
    let (h, m) = start.split_once(':').unwrap_or(("10", "00"));
    let minutes = h.parse::<i64>().unwrap_or(10) * 60
        + m.parse::<i64>().unwrap_or(0)
        + (hours * Decimal::from(60))
            .trunc()
            .try_into()
            .unwrap_or(180i64);
    format!("{:02}:{:02}", (minutes / 60) % 24, minutes % 60)
}

async fn seed_demo(conn: &mut PgConnection, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE settings SET
             business_name = $2, chef_name = 'Alex Rivera', phone = '(503) 555-0100',
             email = $3, website = '', location = 'Portland, OR',
             default_session_window = 3.5, default_serving_size = 4,
             default_service_fee = 275, logo_base64 = NULL, invoice_prefix = 'INV',
             deposit_cents = 5000, cancellation_hours = 48,
             payment_methods = 'Card, Venmo or Zelle',
             booking_terms = 'A $50 deposit holds your first session and is credited to your first invoice. Groceries are billed at cost with receipts. Please give 48 hours'' notice to reschedule.',
             response_time_text = 'within 24 hours',
             service_types = '["Weekly meal prep","Cooking in clients’ homes","Dinner parties & events"]',
             remind_client = false, remind_chef = false, timezone = 'America/Los_Angeles',
             ai_provider = NULL, ai_model = NULL, ai_base_url = NULL,
             ai_api_key_enc = NULL, ai_key_hint = NULL, updated_at = now()
           WHERE account_id = $1"#,
    )
    .bind(id)
    .bind(DEMO_BUSINESS)
    .bind(DEMO_EMAIL)
    .execute(&mut *conn)
    .await?;

    seed::seed_starter_library(&mut *conn, id).await?;
    let recipe_ids: Vec<(String, i64)> =
        sqlx::query_as("SELECT name, id FROM recipes WHERE account_id = $1")
            .bind(id)
            .fetch_all(&mut *conn)
            .await?;
    let recipe = |name: &str| recipe_ids.iter().find(|(n, _)| n == name).map(|(_, i)| *i);

    let today = accounts::local_clock(&mut *conn, id).await?.today;
    let clients = clients();
    let mut client_ids = Vec::with_capacity(clients.len());
    for c in &clients {
        let client_id: i64 = sqlx::query_scalar(
            "INSERT INTO clients (account_id, name, email, phone, address, household_type,
               adults, children, serving_size, allergens, dietary_styles, liked_proteins,
               disliked_foods, spice_tolerance, stovetop_type, equipment, client_containers,
               parking_notes, preferred_day, preferred_time, grocery_handling, chef_notes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 'Gas',
               '[\"Dutch Oven\",\"Large Sheet Pans\"]', true, $15, $16, $17, 'Chef shops', $18)
             RETURNING id",
        )
        .bind(id)
        .bind(c.name)
        .bind(c.email)
        .bind(c.phone)
        .bind(c.address)
        .bind(c.household_type)
        .bind(c.adults)
        .bind(c.children)
        .bind(c.serving_size)
        .bind(json!(c.allergens))
        .bind(json!(c.dietary_styles))
        .bind(json!(c.liked_proteins))
        .bind(c.disliked_foods)
        .bind(c.spice_tolerance)
        .bind(c.parking_notes)
        .bind(c.preferred_day)
        .bind(c.preferred_time)
        .bind(c.chef_notes)
        .fetch_one(&mut *conn)
        .await?;
        client_ids.push(client_id);
    }

    let mut invoice_no = 0;
    let mut rotation = [0usize; 5];
    for (n, p) in plans().iter().enumerate() {
        let c = &clients[p.client];
        let date = shift(today, p.days);
        let menu = p.menu.unwrap_or_else(|| {
            let m = c.menus[rotation[p.client] % c.menus.len()];
            rotation[p.client] += 1;
            m
        });
        // A little variation, so the cost charts aren't flat lines.
        let groceries = Decimal::from(c.groceries + (n as i64 * 7) % 11 - 5);
        let fee = Decimal::from(c.fee);
        let session_id: i64 = sqlx::query_scalar(
            "INSERT INTO sessions (account_id, client_id, session_date, session_time,
               serving_size, session_window_hours, status, service_fee, ingredient_cost,
               chef_notes, completed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
               CASE WHEN $7 = 'Completed' THEN ($3::date + time '15:00') AT TIME ZONE 'America/Los_Angeles' END)
             RETURNING id",
        )
        .bind(id)
        .bind(client_ids[p.client])
        .bind(date)
        .bind(p.time)
        .bind(c.serving_size)
        .bind(c.hours)
        .bind(p.status)
        .bind(fee)
        .bind(groceries)
        .bind(Some(p.notes).filter(|s| !s.is_empty()))
        .fetch_one(&mut *conn)
        .await?;
        for (i, name) in menu.iter().enumerate() {
            sqlx::query(
                "INSERT INTO session_dishes (session_id, recipe_id, sort_order) VALUES ($1, $2, $3)",
            )
            .bind(session_id)
            .bind(recipe(name))
            .bind(i as i32)
            .execute(&mut *conn)
            .await?;
        }
        if p.status != "Cancelled" {
            let checked = p.status == "Completed";
            for item in generate_grocery_list(&mut *conn, session_id, Some(c.serving_size)).await? {
                sqlx::query(
                    "INSERT INTO grocery_items (session_id, ingredient_name, total_quantity,
                       store_section, source_dishes, is_checked)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(session_id)
                .bind(&item.ingredient_name)
                .bind(&item.total_quantity)
                .bind(&item.store_section)
                .bind(json!(item.source_dishes))
                .bind(checked)
                .execute(&mut *conn)
                .await?;
            }
        }

        let Some(kind) = p.invoice else { continue };
        invoice_no += 1;
        let issue = shift(date, 1);
        let (status, due, paid_at) = match kind {
            Inv::Paid => (
                "paid",
                shift(issue, 7),
                shift(issue, 2).and_hms_opt(20, 0, 0).map(|d| d.and_utc()),
            ),
            Inv::PaidToday => ("paid", shift(issue, 7), Some(Utc::now())),
            Inv::Sent(due_in) => ("sent", shift(date, due_in), None),
            Inv::Draft => ("draft", shift(issue, 7), None),
        };
        let total = fee + groceries;
        let number = format!("INV-{}-{:04}", issue.year(), invoice_no);
        let invoice_id: i64 = sqlx::query_scalar(
            "INSERT INTO invoices (account_id, client_id, session_id, invoice_number, status,
               issue_date, due_date, client_name, client_email, client_address, client_phone,
               business_name, chef_name, chef_email, chef_phone, message,
               subtotal, tax_rate, tax_amount, total, paid_at, session_date_label,
               recipes_summary, session_start_time, session_end_time, total_hours,
               container_count, store_trip, grocery_total, service_fee, emailed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'Alex Rivera', $13,
               '(503) 555-0100', 'Thank you! It was a pleasure cooking for you.',
               $14, 0, 0, $14, $15, $16, $17, $18, $19, $20, $21, true, $22, $23,
               CASE WHEN $5 = 'draft' THEN NULL ELSE ($6::date + time '17:00') AT TIME ZONE 'America/Los_Angeles' END)
             RETURNING id",
        )
        .bind(id)
        .bind(client_ids[p.client])
        .bind(session_id)
        .bind(&number)
        .bind(status)
        .bind(issue)
        .bind(due)
        .bind(c.name)
        .bind(c.email)
        .bind(c.address)
        .bind(c.phone)
        .bind(DEMO_BUSINESS)
        .bind(DEMO_EMAIL)
        .bind(total)
        .bind(paid_at)
        .bind(date_label(date))
        .bind(json!(menu))
        .bind(p.time)
        .bind(end_time(p.time, c.hours))
        .bind(c.hours)
        .bind(i32::try_from(menu.len() * 4).unwrap_or(12))
        .bind(groceries)
        .bind(fee)
        .fetch_one(&mut *conn)
        .await?;
        for (sort, (description, amount)) in [
            (format!("Meal prep service, {}", date_label(date)), fee),
            (
                "Groceries at cost (receipts attached)".to_string(),
                groceries,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            sqlx::query(
                "INSERT INTO invoice_line_items (invoice_id, sort_order, description, quantity,
                   unit_price, amount)
                 VALUES ($1, $2, $3, 1, $4, $4)",
            )
            .bind(invoice_id)
            .bind(sort as i32)
            .bind(description)
            .bind(amount)
            .execute(&mut *conn)
            .await?;
        }
    }

    seed_inquiries(conn, id, today, client_ids[GOLDBERG]).await
}

/// One booking-page inquiry.
struct Inquiry {
    name: &'static str,
    email: &'static str,
    household_type: &'static str,
    adults: i32,
    children: i32,
    allergens: &'static [&'static str],
    dietary_styles: &'static [&'static str],
    preferred_day: &'static str,
    notes: &'static str,
    status: &'static str,
    payment_status: &'static str,
    /// The Goldbergs' inquiry became a client.
    converted: bool,
    days_ago: i64,
}

const INQUIRIES: [Inquiry; 3] = [
    Inquiry {
        name: "Jordan & Casey Lee",
        email: "jordan.lee@example.com",
        household_type: "Couple",
        adults: 2,
        children: 0,
        allergens: &["Shellfish"],
        dietary_styles: &["Omnivore"],
        preferred_day: "Tuesday",
        notes: "We both work long hours and want healthy lunches and dinners for the week. Starting next month would be ideal.",
        status: "submitted",
        payment_status: "not_requested",
        converted: false,
        days_ago: 1,
    },
    Inquiry {
        name: "Nina Alvarez",
        email: "nina.alvarez@example.com",
        household_type: "Large Family",
        adults: 2,
        children: 4,
        allergens: &["Dairy"],
        dietary_styles: &["Omnivore", "Dairy-Free"],
        preferred_day: "Saturday",
        notes: "Four kids with busy sports schedules. Big batches that freeze well, please!",
        status: "reviewed_by_chef",
        payment_status: "deposit_requested",
        converted: false,
        days_ago: 3,
    },
    Inquiry {
        name: "Ruth & David Goldberg",
        email: "goldbergs@example.com",
        household_type: "Couple",
        adults: 2,
        children: 0,
        allergens: &["Sesame"],
        dietary_styles: &["Pescatarian", "Mediterranean"],
        preferred_day: "Friday",
        notes: "Retired and cooking less these days. We love fish and Mediterranean food.",
        status: "booked",
        payment_status: "paid_offline",
        converted: true,
        days_ago: 12,
    },
];

/// Booking-page inquiries: one new, one awaiting its deposit, and the one
/// that became the Goldbergs.
async fn seed_inquiries(
    conn: &mut PgConnection,
    id: i64,
    today: NaiveDate,
    goldberg_id: i64,
) -> Result<(), sqlx::Error> {
    let ago = |days: i64| {
        shift(today, -days)
            .and_hms_opt(18, 30, 0)
            .map(|d| d.and_utc())
    };
    for q in &INQUIRIES {
        let created = ago(q.days_ago);
        let deposit = if q.payment_status == "not_requested" {
            0
        } else {
            50
        };
        let paid_at = (q.payment_status == "paid_offline")
            .then(|| ago(q.days_ago - 1))
            .flatten();
        sqlx::query(
            "INSERT INTO client_intakes (account_id, name, email, household_type, adults, children,
               preferred_serving_size, allergens, dietary_styles, preferred_day, preferred_time,
               notes_for_chef, grocery_handling, number_of_meals, terms_accepted,
               terms_accepted_at, status, payment_status, booking_fee_amount,
               booking_fee_paid_at, converted_client_id, created_at, updated_at,
               confirmation_emailed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'Morning', $11, 'Chef shops', 4,
               true, $12, $13, $14, $15, $16, $17, $12, $12, $12)",
        )
        .bind(id)
        .bind(q.name)
        .bind(q.email)
        .bind(q.household_type)
        .bind(q.adults)
        .bind(q.children)
        .bind(if q.adults + q.children > 3 { 6 } else { 4 })
        .bind(json!(q.allergens))
        .bind(json!(q.dietary_styles))
        .bind(q.preferred_day)
        .bind(q.notes)
        .bind(created)
        .bind(q.status)
        .bind(q.payment_status)
        .bind(Decimal::from(deposit))
        .bind(paid_at)
        .bind(q.converted.then_some(goldberg_id))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_use_starter_recipes() {
        let lib = include_str!("seed/starter_library.json");
        for c in clients() {
            for menu in c.menus {
                for name in *menu {
                    assert!(lib.contains(&format!("\"{name}\"")), "{name}");
                }
            }
        }
        for p in plans() {
            for name in p.menu.unwrap_or(&[]) {
                assert!(lib.contains(&format!("\"{name}\"")), "{name}");
            }
        }
    }

    #[test]
    fn end_time_adds_hours() {
        assert_eq!(end_time("10:00", Decimal::new(35, 1)), "13:30");
        assert_eq!(end_time("09:30", Decimal::from(3)), "12:30");
    }

    #[test]
    fn date_label_reads_like_invoices() {
        let d = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
        assert_eq!(date_label(d), "Sunday, March 15, 2026");
    }
}
