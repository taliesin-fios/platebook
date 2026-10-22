// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! The server-rendered PDFs: each endpoint answers `application/pdf` whose
//! text carries the session / invoice / planner data and the chef's settings.

mod common;

use axum::http::{StatusCode, header};
use chef_server::db::MIGRATOR;
use common::{Chef, TestResponse, app};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A valid 1×1 PNG.
const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// The PDF's text with whitespace collapsed (extraction splits lines).
fn pdf_text(res: &TestResponse) -> String {
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    assert_eq!(res.headers[header::CONTENT_TYPE], "application/pdf");
    assert!(res.bytes.starts_with(b"%PDF-"), "not a PDF");
    if let Ok(dir) = std::env::var("PDF_OUT_DIR") {
        // Handy for eyeballing: PDF_OUT_DIR=/tmp/pdfs cargo test --test pdfs
        let name = disposition(res)
            .rsplit('"')
            .nth(1)
            .unwrap_or("out.pdf")
            .to_string();
        std::fs::write(std::path::Path::new(&dir).join(name), &res.bytes).unwrap();
    }
    let text = pdf_extract::extract_text_from_mem(&res.bytes).expect("extract text");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn disposition(res: &TestResponse) -> String {
    res.headers[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_string()
}

struct Fixture {
    chef: Chef,
    session: i64,
    invoice: i64,
    recipes: Vec<i64>,
}

async fn fixture(pool: &PgPool) -> Fixture {
    let app = app(pool.clone());
    let chef = Chef::new(pool, &app, "Ignored").await;
    let res = chef
        .put(
            "/api/settings",
            json!({ "business_name": "Saffron & Thyme", "website": "saffronthyme.example",
                    "chef_name": "Rosa Diaz", "email": "rosa@saffronthyme.example",
                    "logo_base64": PNG }),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    let id = |v: &Value| v["id"].as_i64().unwrap();
    let client = chef
        .post(
            "/api/clients",
            json!({ "name": "Morgan Lee", "email": "morgan@example.com",
                    "allergens": ["Peanuts", "Shellfish"] }),
        )
        .await
        .body;
    let lasagna = chef
        .post(
            "/api/recipes",
            json!({ "name": "Beef Lasagna", "ingredients": [
                { "name": "Ground beef", "qty_4_serving": "1 lb", "qty_6_serving": "1.5 lb", "store_section": "Meat & Seafood" },
                { "name": "Onion", "qty_4_serving": "1", "qty_6_serving": "1.5", "store_section": "Produce" }
            ] }),
        )
        .await
        .body;
    let curry = chef
        .post(
            "/api/recipes",
            json!({ "name": "Panang Curry", "ingredients": [
                { "name": "Onion", "qty_4_serving": "2", "qty_6_serving": "3", "store_section": "Produce" },
                { "name": "Coconut milk", "qty_4_serving": "1 can", "qty_6_serving": "2 can", "store_section": "Canned & Jarred" }
            ] }),
        )
        .await
        .body;
    let session = chef
        .post(
            "/api/sessions",
            json!({ "client_id": id(&client), "session_date": "2026-03-15", "serving_size": 6,
                    "service_fee": 250, "chef_notes": "Use the blue containers",
                    "recipe_ids": [id(&lasagna), id(&curry)] }),
        )
        .await
        .body;
    let invoice = chef
        .post(
            &format!("/api/invoices/from-session/{}", id(&session)),
            json!({}),
        )
        .await
        .body;
    // Line items with exact cents, tax, a note and a multi-line address.
    let res = chef
        .put(
            &format!("/api/invoices/{}", id(&invoice)),
            json!({ "issue_date": "2026-03-16", "due_date": "2026-03-30", "tax_rate": 8.25,
                    "client_name": "Morgan Lee", "client_address": "12 Elm St\nSpringfield",
                    "business_name": "Saffron & Thyme",
                    "message": "Thank you for your business!",
                    "line_items": [
                        { "description": "Meal Prep Session", "quantity": 1, "unit_price": 250, "amount": 250 },
                        { "description": "Groceries", "quantity": 1, "unit_price": 87.35, "amount": 87.35 }
                    ] }),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    Fixture {
        chef,
        session: id(&session),
        invoice: id(&invoice),
        recipes: vec![id(&lasagna), id(&curry)],
    }
}

fn assert_has(text: &str, wants: &[&str]) {
    for want in wants {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn meal_plan_pdf(pool: PgPool) {
    let f = fixture(&pool).await;
    let res = f
        .chef
        .get(&format!("/api/sessions/{}/pdf", f.session))
        .await;
    let t = pdf_text(&res);
    assert_has(
        &t,
        &[
            "SAFFRON & THYME",
            "Morgan Lee — Weekly Meals",
            "Sunday, March 15, 2026",
            "Serves 6",
            "Beef Lasagna",
            "Oven 325°F covered 20 min",
            "Panang Curry",
            "splash of water",
            "Use by Mar 19",
            "FREEZE",
            "This menu was planned to avoid: Peanuts, Shellfish.",
            "Prepared with care by your chef",
            "saffronthyme.example",
        ],
    );
    assert_eq!(
        disposition(&res),
        "attachment; filename=\"meal-plan-morgan-lee-2026-03-15.pdf\""
    );
    assert_eq!(res.headers[header::CACHE_CONTROL], "no-store");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn grocery_pdf_generated_and_stored(pool: PgPool) {
    let f = fixture(&pool).await;
    let uri = format!("/api/sessions/{}/grocery/pdf", f.session);
    // No stored list yet: generated on the fly, like GET .../grocery.
    let t = pdf_text(&f.chef.get(&uri).await);
    assert_has(
        &t,
        &[
            "Shopping List — Morgan Lee",
            "Chef Notes",
            "Use the blue containers",
            "Produce",
            "Ground beef",
            "Coconut milk — 2 can",
            "Beef Lasagna: 1.5 lb",
            "Prepared Sunday, March 15, 2026",
        ],
    );
    f.chef
        .post(
            &format!("/api/sessions/{}/grocery/generate", f.session),
            json!({}),
        )
        .await;
    let res = f.chef.get(&uri).await;
    assert_has(&pdf_text(&res), &["Coconut milk", "Beef Lasagna"]);
    assert_eq!(
        disposition(&res),
        "attachment; filename=\"grocery-morgan-lee-2026-03-15.pdf\""
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn invoice_pdf(pool: PgPool) {
    let f = fixture(&pool).await;
    let inv = f
        .chef
        .get(&format!("/api/invoices/{}", f.invoice))
        .await
        .body;
    let res = f
        .chef
        .get(&format!("/api/invoices/{}/pdf", f.invoice))
        .await;
    let t = pdf_text(&res);
    let number = inv["invoice_number"].as_str().unwrap();
    // The letter-spaced label extracts as "I N V O I C E".
    assert!(t.replace(' ', "").contains("INVOICE"), "{t}");
    assert_has(
        &t,
        &[
            "Saffron & Thyme",
            number,
            "Issued: March 16, 2026",
            "Due: March 30, 2026",
            "BILL TO",
            "Morgan Lee",
            "12 Elm St",
            "Springfield",
            "Meal Prep Session",
            "$250.00",
            "$87.35",
            "Subtotal $337.35",
            "Tax (8.25%) $27.83",
            "Total Due $365.18",
            "Thank you for your business!",
        ],
    );
    assert_eq!(
        disposition(&res),
        format!("attachment; filename=\"invoice-{number}.pdf\"")
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn planner_pdf(pool: PgPool) {
    let f = fixture(&pool).await;
    let ids = f
        .recipes
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let res = f
        .chef
        .get(&format!(
            "/api/planner/pdf?recipe_ids={ids}&serving_size=8&date=2026-04-02"
        ))
        .await;
    let t = pdf_text(&res);
    assert_has(
        &t,
        &[
            "SAFFRON & THYME",
            "Menu Planner — Ingredient List",
            "April 2, 2026",
            "Serves 8",
            "Recipes: Beef Lasagna, Panang Curry",
            "unique ingredients · 2 recipes",
            "Onion",
            "Coconut milk",
            "Generated April 2, 2026",
        ],
    );
    assert_eq!(
        disposition(&res),
        "attachment; filename=\"planner-2026-04-02.pdf\""
    );
    let none = f.chef.get("/api/planner/pdf?recipe_ids=").await;
    assert_eq!(none.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn falls_back_to_app_name_without_a_business_name(pool: PgPool) {
    let f = fixture(&pool).await;
    f.chef
        .put("/api/settings", json!({ "business_name": "" }))
        .await;
    let t = pdf_text(
        &f.chef
            .get(&format!("/api/sessions/{}/pdf", f.session))
            .await,
    );
    assert!(t.contains("PLATEBOOK"), "{t}");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn pdfs_need_auth_and_are_rate_limited_per_account(pool: PgPool) {
    let f = fixture(&pool).await;
    let anon = common::get(format!("/api/sessions/{}/pdf", f.session))
        .send(&f.chef.app)
        .await;
    assert_eq!(anon.status, StatusCode::UNAUTHORIZED);

    // Use up the window directly; the next download is refused.
    sqlx::query(
        "INSERT INTO rate_limit_hits (bucket, key, window_start, hits)
         VALUES ('acct.pdf', $1, now(), 60)",
    )
    .bind(format!("acct:{}", f.chef.id))
    .execute(&pool)
    .await
    .unwrap();
    let res = f
        .chef
        .get(&format!("/api/invoices/{}/pdf", f.invoice))
        .await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(res.body["code"], "RATE_LIMITED");
}
