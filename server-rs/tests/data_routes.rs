// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Route-group tests for the ported data routes: settings, onboarding,
//! public profile, clients, recipes, templates, sessions, grocery, planner,
//! costs, invoices. (Intake, payments, AI and email live in
//! `integrations.rs`; cross-account checks in `tenancy.rs`.)

mod common;

use axum::http::StatusCode;
use chef_server::db::MIGRATOR;
use common::{Chef, app, get};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn chef(pool: &PgPool, name: &str) -> Chef {
    Chef::new(pool, &app(pool.clone()), name).await
}

/// A client + recipe + session with that recipe; returns (client, recipe, session).
async fn basics(c: &Chef) -> (i64, i64, i64) {
    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Jo", "email": "jo@example.com" }),
        )
        .await;
    assert_eq!(client.status, StatusCode::CREATED, "{}", client.body);
    let recipe = c
        .post(
            "/api/recipes",
            json!({ "name": "Stew", "prep_time_min": 10, "cook_time_min": 50,
                    "ingredients": [{ "name": "onion", "qty_4_serving": "1 1/2 cups" },
                                    { "name": "stock", "qty_4_serving": "2 cups", "store_section": "Canned & Jarred" }] }),
        )
        .await;
    assert_eq!(recipe.status, StatusCode::CREATED, "{}", recipe.body);
    let session = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": "2026-11-03",
                    "service_fee": 200, "ingredient_cost": 50,
                    "recipe_ids": [recipe.body["id"]] }),
        )
        .await;
    assert_eq!(session.status, StatusCode::CREATED, "{}", session.body);
    (
        client.body["id"].as_i64().unwrap(),
        recipe.body["id"].as_i64().unwrap(),
        session.body["id"].as_i64().unwrap(),
    )
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn settings_round_trip_validation_and_public_profile(pool: PgPool) {
    let c = chef(&pool, "Policy Kitchen").await;
    let s = c.get("/api/settings").await;
    assert_eq!(s.status, StatusCode::OK);
    assert_eq!(s.body["business_name"], "Policy Kitchen");
    assert_eq!(s.body["slug"], "policy-kitchen");
    assert_eq!(s.body["ai_configured"], false);
    assert_eq!(s.body["service_types"], json!([]));

    let put = c
        .put(
            "/api/settings",
            json!({ "deposit_cents": 7500.4, "cancellation_hours": 72, "booking_terms": " Be nice. ",
                    "service_types": ["Weekly meal prep"], "invoice_prefix": "bk-2!",
                    "default_session_window": 3.5 }),
        )
        .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert_eq!(put.body["deposit_cents"], 7500);
    assert_eq!(put.body["booking_terms"], "Be nice.");
    assert_eq!(put.body["invoice_prefix"], "BK2");
    assert_eq!(put.body["default_session_window"], 3.5);
    assert_eq!(put.body["service_types"], json!(["Weekly meal prep"]));
    assert!(put.body["updated_at"].as_str().unwrap().len() == 19);

    for bad in [
        json!({ "deposit_cents": -5 }),
        json!({ "email": "nope" }),
        json!({ "service_types": "x" }),
        json!({ "logo_base64": "http://evil/x.png" }),
        json!({ "business_name": "x".repeat(121) }),
    ] {
        let r = c.put("/api/settings", bad.clone()).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{bad}");
        assert_eq!(r.body["code"], "VALIDATION_ERROR");
    }
    assert_eq!(
        c.put("/api/settings", json!({ "cancellation_hours": 9999 }))
            .await
            .body["error"],
        "cancellation_hours must be between 0 and 720"
    );

    let public = get("/api/public/policy-kitchen/settings")
        .send(&c.app)
        .await;
    assert_eq!(public.status, StatusCode::OK);
    assert_eq!(public.body["deposit_cents"], 7500);
    assert_eq!(public.body["payments_mode"], "manual");
    assert!(public.body.get("email").is_none());
    assert!(public.body.get("phone").is_none());
    let missing = get("/api/public/nobody/settings").send(&c.app).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.body["error"], "Chef not found");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn onboarding_seed_is_idempotent_and_clear_sample_cascades(pool: PgPool) {
    let c = chef(&pool, "Seed Kitchen").await;
    let first = c
        .post(
            "/api/onboarding/seed",
            json!({ "starter_library": true, "sample_client": true }),
        )
        .await;
    assert_eq!(first.status, StatusCode::OK);
    assert!(first.body["library"]["recipes"].as_u64().unwrap() > 10);
    assert_eq!(first.body["library"]["templates"], 4);
    assert_eq!(first.body["sample_client"], true);
    let again = c
        .post(
            "/api/onboarding/seed",
            json!({ "starter_library": true, "sample_client": true }),
        )
        .await;
    assert_eq!(
        again.body,
        json!({ "library": { "recipes": 0, "templates": 0 }, "sample_client": false })
    );
    let templates = c.get("/api/templates").await.body;
    assert_eq!(templates[0]["is_default"], 1);
    assert_eq!(templates[0]["recipes"].as_array().unwrap().len(), 3);

    let sample = c.get("/api/clients").await.body[0].clone();
    assert_eq!(sample["is_sample"], 1);
    assert_eq!(sample["allergens"], json!(["Tree Nuts"]));
    let session = c
        .post(
            "/api/sessions",
            json!({ "client_id": sample["id"], "service_fee": 100 }),
        )
        .await;
    c.post(
        &format!("/api/invoices/from-session/{}", session.body["id"]),
        json!({}),
    )
    .await;
    let cleared = c.delete("/api/settings/clear-sample").await;
    assert_eq!(cleared.status, StatusCode::OK, "{}", cleared.body);
    assert_eq!(cleared.body["removed"], 1);
    assert_eq!(c.get("/api/clients").await.body, json!([]));
    assert_eq!(c.get("/api/invoices").await.body, json!([]));

    let done = c.post("/api/onboarding/complete", json!({})).await;
    assert_eq!(done.body, json!({ "success": true }));
    assert_eq!(
        c.get("/api/auth/me").await.body["onboarding_completed"],
        true
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn clients_crud_and_validation(pool: PgPool) {
    let c = chef(&pool, "Client Kitchen").await;
    for (body, msg) in [
        (json!({}), "Name is required"),
        (json!({ "name": 5 }), "Name is required"),
        (
            json!({ "name": "x".repeat(201) }),
            "Name must be 200 characters or fewer",
        ),
        (
            json!({ "name": "A", "email": "bad" }),
            "Invalid email format",
        ),
        (
            json!({ "name": "A", "phone": "12" }),
            "Invalid phone format",
        ),
    ] {
        let r = c.post("/api/clients", body).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
        assert_eq!(r.body["error"], msg);
    }
    let created = c
        .post(
            "/api/clients",
            json!({ "name": "Jo", "allergens": ["Peanuts"], "oven_available": 0, "client_containers": 1 }),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let id = created.body["id"].as_i64().unwrap();
    assert_eq!(created.body["adults"], 2);
    assert_eq!(created.body["serving_size"], 4);
    assert_eq!(created.body["oven_available"], 0);
    assert_eq!(created.body["client_containers"], 1);
    assert_eq!(created.body["equipment"], json!([]));

    let put = c
        .put(
            &format!("/api/clients/{id}"),
            json!({ "adults": "3", "equipment": ["Wok"], "phone": null }),
        )
        .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert_eq!(put.body["adults"], 3);
    assert_eq!(put.body["equipment"], json!(["Wok"]));
    let empty = c.put(&format!("/api/clients/{id}"), json!({})).await;
    assert_eq!(empty.status, StatusCode::BAD_REQUEST);
    assert_eq!(empty.body["code"], "EMPTY_BODY");
    let bad_int = c
        .put(&format!("/api/clients/{id}"), json!({ "children": 1.5 }))
        .await;
    assert_eq!(bad_int.body["error"], "children must be an integer");

    let list = c.get("/api/clients").await;
    assert_eq!(list.body[0]["session_count"], 0);
    assert_eq!(list.body[0]["last_session_date"], Value::Null);
    let detail = c.get(&format!("/api/clients/{id}")).await;
    assert_eq!(detail.body["sessions"], json!([]));

    assert_eq!(
        c.delete(&format!("/api/clients/{id}")).await.body,
        json!({ "success": true })
    );
    assert_eq!(
        c.get(&format!("/api/clients/{id}")).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.get("/api/clients/abc").await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn recipes_crud_filters_and_in_use_guard(pool: PgPool) {
    let c = chef(&pool, "Recipe Kitchen").await;
    let (_, recipe, _) = basics(&c).await;
    let detail = c.get(&format!("/api/recipes/{recipe}")).await;
    assert_eq!(detail.body["total_time"], 60);
    assert_eq!(detail.body["category"], "Other");
    assert_eq!(detail.body["session_fit"], "Session-Friendly");
    assert_eq!(detail.body["ingredients"].as_array().unwrap().len(), 2);

    // COALESCE semantics: absent fields keep their value; ingredients replace.
    let put = c
        .put(
            &format!("/api/recipes/{recipe}"),
            json!({ "chef_notes": "Day 2", "ingredients": [{ "name": "salt" }, { "name": " " }] }),
        )
        .await;
    assert_eq!(put.body["name"], "Stew");
    assert_eq!(put.body["chef_notes"], "Day 2");
    assert_eq!(put.body["ingredients"].as_array().unwrap().len(), 1);

    assert_eq!(
        c.get("/api/recipes?search=STE")
            .await
            .body
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(c.get("/api/recipes?category=Couples").await.body, json!([]));

    let in_use = c.delete(&format!("/api/recipes/{recipe}")).await;
    assert_eq!(in_use.status, StatusCode::CONFLICT);
    assert_eq!(in_use.body["code"], "RECIPE_IN_USE");
    assert_eq!(in_use.body["session_count"], 1);

    let spare = c.post("/api/recipes", json!({ "name": "Salad" })).await;
    assert_eq!(
        spare.body.get("total_time"),
        None,
        "POST has no total_time, like Node"
    );
    let id = spare.body["id"].as_i64().unwrap();
    assert_eq!(
        c.delete(&format!("/api/recipes/{id}")).await.status,
        StatusCode::OK
    );
    assert_eq!(
        c.get("/api/recipes").await.body.as_array().unwrap().len(),
        1
    );
    assert_eq!(
        c.get("/api/recipes?active=false")
            .await
            .body
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        c.post("/api/recipes", json!({})).await.status,
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn templates_dedupe_own_recipes_and_reject_duplicates(pool: PgPool) {
    let c = chef(&pool, "Template Kitchen").await;
    let (_, recipe, _) = basics(&c).await;
    let t = c
        .post(
            "/api/templates",
            json!({ "name": " Weeknight ", "recipe_ids": [recipe, recipe.to_string(), 999_999, -1, "x"] }),
        )
        .await;
    assert_eq!(t.status, StatusCode::CREATED);
    assert_eq!(t.body["name"], "Weeknight");
    assert_eq!(t.body["recipe_ids"], json!([recipe]));
    assert_eq!(t.body["is_default"], 0);
    let dup = c
        .post("/api/templates", json!({ "name": "Weeknight" }))
        .await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    assert_eq!(dup.body["code"], "DUPLICATE_NAME");
    assert_eq!(
        c.post("/api/templates", json!({ "name": " " }))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );

    let id = t.body["id"].as_i64().unwrap();
    let put = c
        .put(
            &format!("/api/templates/{id}"),
            json!({ "name": "Weekend", "recipe_ids": [] }),
        )
        .await;
    assert_eq!(put.body["recipes"], json!([]));
    let from = c
        .post(&format!("/api/sessions/from-template/{id}"), json!({}))
        .await;
    assert_eq!(
        from.body,
        json!({ "recipe_ids": [], "template_name": "Weekend" })
    );
    assert_eq!(
        c.delete(&format!("/api/templates/{id}")).await.status,
        StatusCode::OK
    );
    assert_eq!(
        c.delete(&format!("/api/templates/{id}")).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn sessions_lifecycle(pool: PgPool) {
    let c = chef(&pool, "Session Kitchen").await;
    let (client, recipe, session) = basics(&c).await;
    let d = c.get(&format!("/api/sessions/{session}")).await.body;
    assert_eq!(d["client_name"], "Jo");
    assert_eq!(d["client_allergens"], json!([]));
    assert_eq!(d["gross_margin"], 150.0);
    assert_eq!(d["margin_pct"], 75.0);
    assert_eq!(d["dishes"][0]["total_time"], 60);
    assert_eq!(d["dishes"][0]["sort_order"], 0);

    let list = c
        .get(&format!("/api/sessions?client_id={client}"))
        .await
        .body;
    assert_eq!(list[0]["dish_count"], 1);
    assert_eq!(
        list[0]["client_allergens"], "[]",
        "the list route returns raw JSON text"
    );
    let upcoming = c
        .get("/api/sessions?upcoming=true&today=2026-11-04")
        .await
        .body;
    assert_eq!(upcoming, json!([]));
    assert_eq!(
        c.get("/api/sessions?status=Nope").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        c.post(
            "/api/sessions",
            json!({ "client_id": client, "session_date": "soon" })
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );

    let put = c
        .put(
            &format!("/api/sessions/{session}"),
            json!({ "status": "Confirmed", "recipe_ids": [recipe, recipe] }),
        )
        .await;
    assert_eq!(put.body["status"], "Confirmed");
    assert_eq!(put.body["dishes"].as_array().unwrap().len(), 2);
    let bad = c
        .put(
            &format!("/api/sessions/{session}"),
            json!({ "recipe_ids": [999_999] }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);

    let clone = c
        .post(
            &format!("/api/sessions/{session}/clone"),
            json!({ "days_offset": "-3" }),
        )
        .await;
    assert_eq!(clone.status, StatusCode::CREATED);
    assert_eq!(clone.body["session_date"], "2026-10-31");
    assert_eq!(clone.body["status"], "Pending");
    let cid = clone.body["id"].as_i64().unwrap();
    assert_eq!(
        c.post(
            &format!("/api/sessions/{session}/clone"),
            json!({ "days_offset": 400 })
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );

    let done = c
        .post(&format!("/api/sessions/{cid}/complete"), json!({}))
        .await;
    assert_eq!(done.body["status"], "Completed");
    assert!(done.body["completed_at"].is_string());
    let again = c
        .post(&format!("/api/sessions/{cid}/complete"), json!({}))
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.body["code"], "ALREADY_COMPLETED");

    let dishes = c
        .post(
            &format!("/api/sessions/{cid}/dishes"),
            json!({ "recipe_ids": [] }),
        )
        .await;
    assert_eq!(dishes.body["dishes"], json!([]));

    // An attached invoice blocks deletion until it's voided.
    let inv = c
        .post(&format!("/api/invoices/from-session/{session}"), json!({}))
        .await;
    let blocked = c.delete(&format!("/api/sessions/{session}")).await;
    assert_eq!(blocked.status, StatusCode::CONFLICT);
    assert_eq!(blocked.body["invoice_id"], inv.body["id"]);
    c.put(
        &format!("/api/invoices/{}", inv.body["id"]),
        json!({ "status": "void" }),
    )
    .await;
    assert_eq!(
        c.delete(&format!("/api/sessions/{session}")).await.status,
        StatusCode::OK
    );
    assert_eq!(
        c.get(&format!("/api/sessions/{session}")).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn repeating_sessions_book_the_next_one_on_completion(pool: PgPool) {
    let c = chef(&pool, "Weekly Kitchen").await;
    let (client, _, session) = basics(&c).await;

    // Validation on create and update.
    let bad = c
        .post(
            "/api/sessions",
            json!({ "client_id": client, "session_date": "2026-11-03", "repeat_every_weeks": 9 }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    // Fractions and junk are rejected, not rounded or read as "off".
    for junk in [json!(2.5), json!("invalid"), json!({}), json!(-1)] {
        let res = c
            .put(
                &format!("/api/sessions/{session}"),
                json!({ "repeat_every_weeks": junk }),
            )
            .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST, "{junk}");
    }
    let set = c
        .put(
            &format!("/api/sessions/{session}"),
            json!({ "repeat_every_weeks": 2, "session_time": "09:30" }),
        )
        .await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.body);
    assert_eq!(set.body["repeat_every_weeks"], 2);

    // Completing books the next session two weeks later, with the menu.
    let done = c
        .post(&format!("/api/sessions/{session}/complete"), json!({}))
        .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert_eq!(done.body["status"], "Completed");
    assert_eq!(done.body["next_session_date"], "2026-11-17");
    let next_id = done.body["next_session_id"].as_i64().unwrap();
    let next = c.get(&format!("/api/sessions/{next_id}")).await.body;
    assert_eq!(next["status"], "Pending");
    assert_eq!(next["session_time"], "09:30");
    assert_eq!(next["service_fee"], 200.0);
    assert_eq!(next["repeat_every_weeks"], 2);
    assert_eq!(next["repeat_of"], session);
    assert_eq!(next["dishes"].as_array().unwrap().len(), 1);
    assert!(next["completed_at"].is_null());

    // Re-completing (after a status reset) never books a second follow-up.
    c.put(
        &format!("/api/sessions/{session}"),
        json!({ "status": "Pending" }),
    )
    .await;
    let again = c
        .post(&format!("/api/sessions/{session}/complete"), json!({}))
        .await;
    assert_eq!(again.body["next_session_id"], next_id);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE repeat_of = $1")
        .bind(session)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // The chain continues from the follow-up.
    let third = c
        .post(&format!("/api/sessions/{next_id}/complete"), json!({}))
        .await;
    assert_eq!(third.body["next_session_date"], "2026-12-01");

    // One-off sessions and inactive clients book nothing.
    let one_off = c
        .post(
            "/api/sessions",
            json!({ "client_id": client, "session_date": "2026-11-04" }),
        )
        .await;
    let done = c
        .post(
            &format!("/api/sessions/{}/complete", one_off.body["id"]),
            json!({}),
        )
        .await;
    assert!(done.body["next_session_id"].is_null());
    let weekly = c
        .post(
            "/api/sessions",
            json!({ "client_id": client, "session_date": "2026-11-05", "repeat_every_weeks": 1 }),
        )
        .await;
    assert_eq!(weekly.body["repeat_every_weeks"], 1);
    sqlx::query("UPDATE clients SET is_active = false WHERE id = $1")
        .bind(client)
        .execute(&pool)
        .await
        .unwrap();
    let done = c
        .post(
            &format!("/api/sessions/{}/complete", weekly.body["id"]),
            json!({}),
        )
        .await;
    assert_eq!(done.status, StatusCode::OK);
    assert!(done.body["next_session_id"].is_null());

    // Clearing the repeat.
    let cleared = c
        .put(
            &format!("/api/sessions/{}", weekly.body["id"]),
            json!({ "repeat_every_weeks": null }),
        )
        .await;
    assert!(cleared.body["repeat_every_weeks"].is_null());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn grocery_generates_merges_and_toggles(pool: PgPool) {
    let c = chef(&pool, "Grocery Kitchen").await;
    let (_, recipe, session) = basics(&c).await;
    let other = c
        .post(
            "/api/recipes",
            json!({ "name": "Soup", "ingredients": [{ "name": "Onion ", "qty_4_serving": "1/2 cups" }] }),
        )
        .await;
    c.put(
        &format!("/api/sessions/{session}"),
        json!({ "recipe_ids": [recipe, other.body["id"]] }),
    )
    .await;

    let generated = c
        .get(&format!("/api/sessions/{session}/grocery"))
        .await
        .body;
    assert_eq!(generated["generated"], true);
    assert_eq!(generated["session_id"], session.to_string());
    let onion = generated["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["ingredient_name"] == "onion")
        .unwrap()
        .clone();
    assert_eq!(onion["total_quantity"], "2 cups");
    assert_eq!(
        onion["source_dishes"],
        json!(["Stew: 1 1/2 cups", "Soup: 1/2 cups"])
    );
    // Sorted by store section: Produce ("Other" for missing) ... Canned before Other
    assert_eq!(generated["items"][0]["store_section"], "Canned & Jarred");

    let stored = c
        .post(
            &format!("/api/sessions/{session}/grocery/generate"),
            json!({}),
        )
        .await
        .body;
    assert!(stored.get("generated").is_none());
    let item = stored["items"][0]["id"].as_i64().unwrap();
    assert_eq!(stored["items"][0]["is_checked"], 0);
    let t1 = c.put(&format!("/api/grocery/{item}"), json!({})).await;
    assert_eq!(t1.body["is_checked"], 1);
    let t2 = c
        .put(&format!("/api/grocery/{item}"), json!({ "is_checked": 1 }))
        .await;
    assert_eq!(t2.body["is_checked"], 1);
    assert_eq!(
        c.put("/api/grocery/999999", json!({})).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn planner_scales_and_groups(pool: PgPool) {
    let c = chef(&pool, "Planner Kitchen").await;
    let (_, recipe, _) = basics(&c).await;
    let res = c
        .post(
            "/api/planner/ingredients",
            json!({ "recipe_ids": [recipe], "serving_size": 6 }),
        )
        .await
        .body;
    assert_eq!(res["recipe_count"], 1);
    assert_eq!(res["recipe_names"], json!(["Stew"]));
    assert_eq!(res["total_items"], 2);
    let sections: Vec<&str> = res["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["section"].as_str().unwrap())
        .collect();
    assert_eq!(sections, ["Canned & Jarred", "Other"]);
    // 1 1/2 cups for 4 → 2 1/4 cups for 6
    assert_eq!(res["sections"][1]["items"][0]["quantity"], "2 1/4 cups");
    assert_eq!(res["sections"][1]["items"][0]["amount"], 2.25);
    let empty = c.post("/api/planner/ingredients", json!({})).await.body;
    assert_eq!(
        empty,
        json!({ "sections": [], "total_items": 0, "recipe_count": 0 })
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn costs_summary_and_csv(pool: PgPool) {
    let c = chef(&pool, "Cost Kitchen").await;
    let client = c
        .post("/api/clients", json!({ "name": "=HYPERLINK(\"x\")" }))
        .await;
    let today = chrono::Utc::now().date_naive().to_string();
    c.post(
        "/api/sessions",
        json!({ "client_id": client.body["id"], "session_date": today,
                "service_fee": 200, "ingredient_cost": 50.5 }),
    )
    .await;
    let s = c.get("/api/costs/summary").await.body;
    assert_eq!(s["total_revenue"], 200.0);
    assert_eq!(s["gross_margin"], 149.5);
    assert_eq!(s["margin_pct"], 74.8);
    assert_eq!(s["session_count"], 1);
    assert_eq!(s["ytd_sessions"], 1);
    assert_eq!(s["overdue_count"], 0);

    let sessions = c.get("/api/costs/sessions?days=7").await.body;
    assert_eq!(sessions[0]["margin_pct"], 74.8);
    assert_eq!(sessions[0]["invoice_id"], Value::Null);
    let by_client = c.get("/api/costs/by-client").await.body;
    assert_eq!(by_client[0]["avg_fee"], 200.0);

    let csv = c.get("/api/costs/export-csv").await;
    assert_eq!(csv.status, StatusCode::OK);
    assert!(
        csv.headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/csv")
    );
    assert_eq!(
        csv.text,
        format!(
            "Date,Client,Service Fee,Ingredient Cost,Gross Margin,Margin %,Status\n\"{today}\",\"'=HYPERLINK(\"\"x\"\")\",\"200\",\"50.5\",\"149.5\",\"74.8%\",\"Pending\""
        )
    );
    // SEC8: 10 exports per 15 minutes per account
    for _ in 0..9 {
        assert_eq!(c.get("/api/costs/export-csv").await.status, StatusCode::OK);
    }
    let limited = c.get("/api/costs/export-csv").await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.body["code"], "RATE_LIMITED");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn costs_keep_negative_margins_numeric_and_show_invoice_due_dates(pool: PgPool) {
    let c = chef(&pool, "Loss Kitchen").await;
    let client = c.post("/api/clients", json!({ "name": "Al" })).await;
    let today = chrono::Utc::now().date_naive().to_string();
    let session = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": today,
                    "service_fee": 100, "ingredient_cost": 120 }),
        )
        .await;
    // R7: a loss stays a number in the CSV, not the text "'-20".
    let csv = c.get("/api/costs/export-csv").await.text;
    assert!(
        csv.ends_with(r#""100","120","-20","-20%","Pending""#),
        "{csv}"
    );

    // R6: the costs table gets the invoice's due date for its overdue ring.
    let inv = c
        .post(
            &format!("/api/invoices/from-session/{}", session.body["id"]),
            json!({}),
        )
        .await;
    let rows = c.get("/api/costs/sessions?days=7").await.body;
    assert_eq!(rows[0]["invoice_id"], inv.body["id"]);
    assert_eq!(rows[0]["invoice_due_date"], inv.body["due_date"]);
    assert!(rows[0]["invoice_due_date"].is_string());
}

/// Today's date at a fixed UTC offset (zones without DST only).
fn today_at(offset_hours: i64) -> chrono::NaiveDate {
    (chrono::Utc::now() + chrono::Duration::hours(offset_hours)).date_naive()
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn today_month_and_overdue_follow_the_account_time_zone(pool: PgPool) {
    // UTC+14: its date is ahead of UTC's for most of every day.
    let c = chef(&pool, "Island Kitchen").await;
    let set = c
        .put("/api/settings", json!({ "timezone": "Pacific/Kiritimati" }))
        .await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.body);
    let today = today_at(14);
    let yesterday = today - chrono::Duration::days(1);
    let client = c.post("/api/clients", json!({ "name": "Kai" })).await;
    let cid = client.body["id"].as_i64().unwrap();

    // New invoices are issued on the local date.
    let inv = c
        .post(&format!("/api/invoices/from-client/{cid}"), json!({}))
        .await;
    assert_eq!(inv.body["issue_date"], today.to_string());
    assert_eq!(
        inv.body["due_date"],
        (today + chrono::Duration::days(14)).to_string()
    );

    // Overdue: due yesterday (local) counts, due today doesn't.
    for (due, status) in [(yesterday, "sent"), (today, "sent")] {
        c.post(
            "/api/invoices",
            json!({ "client_id": cid, "status": status, "due_date": due.to_string(),
                    "line_items": [{ "quantity": 1, "unit_price": 10 }] }),
        )
        .await;
    }
    let s = c.get("/api/costs/summary").await.body;
    assert_eq!(s["overdue_count"], 1, "{s}");

    // "Upcoming" without the client's date uses the local date too.
    for date in [yesterday, today] {
        c.post(
            "/api/sessions",
            json!({ "client_id": cid, "session_date": date.to_string(), "service_fee": 100 }),
        )
        .await;
    }
    let upcoming = c.get("/api/sessions?upcoming=true").await.body;
    let dates: Vec<&str> = upcoming
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["session_date"].as_str().unwrap())
        .collect();
    assert_eq!(dates, vec![today.to_string()]);

    // UTC-12: a session dated "today" there is this month's revenue locally,
    // and an invoice due today isn't overdue yet.
    let west = chef(&pool, "Far West Kitchen").await;
    west.put("/api/settings", json!({ "timezone": "Etc/GMT+12" }))
        .await;
    let w_today = today_at(-12);
    let wc = west.post("/api/clients", json!({ "name": "Wes" })).await;
    let wid = wc.body["id"].as_i64().unwrap();
    west.post(
        "/api/sessions",
        json!({ "client_id": wid, "session_date": w_today.to_string(), "service_fee": 80 }),
    )
    .await;
    west.post(
        "/api/invoices",
        json!({ "client_id": wid, "status": "sent", "due_date": w_today.to_string() }),
    )
    .await;
    let ws = west.get("/api/costs/summary").await.body;
    assert_eq!(ws["total_revenue"], 80.0, "{ws}");
    assert_eq!(ws["overdue_count"], 0, "{ws}");

    // No time zone set: UTC, as before.
    let plain = chef(&pool, "Plain Kitchen").await;
    let pc = plain.post("/api/clients", json!({ "name": "Pat" })).await;
    let pinv = plain
        .post(
            &format!("/api/invoices/from-client/{}", pc.body["id"]),
            json!({}),
        )
        .await;
    assert_eq!(pinv.body["issue_date"], today_at(0).to_string());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn invoices_numbering_totals_and_status_rules(pool: PgPool) {
    let c = chef(&pool, "Invoice Kitchen").await;
    let (client, _, session) = basics(&c).await;
    c.put(
        "/api/settings",
        json!({ "invoice_prefix": "ik", "default_service_fee": 120 }),
    )
    .await;
    let year = chrono::Utc::now().format("%Y");

    let from_session = c
        .post(&format!("/api/invoices/from-session/{session}"), json!({}))
        .await
        .body;
    assert_eq!(from_session["invoice_number"], format!("IK-{year}-0001"));
    assert_eq!(from_session["total"], 250.0);
    assert_eq!(from_session["recipes_summary"], json!(["Stew"]));
    assert_eq!(from_session["container_count"], 4);
    assert_eq!(from_session["line_items"].as_array().unwrap().len(), 2);
    assert_eq!(from_session["client_name_ref"], "Jo");

    let from_client = c
        .post(&format!("/api/invoices/from-client/{client}"), json!({}))
        .await
        .body;
    assert_eq!(from_client["invoice_number"], format!("IK-{year}-0002"));
    assert_eq!(from_client["service_fee"], 120.0);
    assert_eq!(from_client["line_items"][0]["description"], "Chef Service");

    let blank = c
        .post(
            "/api/invoices",
            json!({ "tax_rate": 8.25, "line_items": [
                { "description": "A", "quantity": 1, "unit_price": 100.10, "amount": 100.10 },
                { "description": "B", "quantity": "2", "unit_price": "10.025", "amount": 20.05 } ] }),
        )
        .await;
    assert_eq!(blank.status, StatusCode::CREATED, "{}", blank.body);
    assert_eq!(blank.body["subtotal"], 120.15);
    assert_eq!(blank.body["tax_amount"], 9.91);
    assert_eq!(blank.body["total"], 130.06);
    // An item without `amount` is worth qty × price, in the stored line and
    // in the totals alike (R5).
    let partial = c
        .post(
            "/api/invoices",
            json!({ "line_items": [{ "quantity": 2, "unit_price": 5 }, { "quantity": 1, "unit_price": 1, "amount": 1 }] }),
        )
        .await
        .body;
    assert_eq!(partial["line_items"][0]["amount"], 10.0);
    assert_eq!(partial["subtotal"], 11.0);
    assert_eq!(partial["total"], 11.0);
    c.delete(&format!("/api/invoices/{}", partial["id"])).await;
    let id = blank.body["id"].as_i64().unwrap();

    let bad = c
        .post(
            "/api/invoices",
            json!({ "line_items": [{ "quantity": "x", "unit_price": 1 }] }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    // Unknown statuses would drop out of every total (R8).
    let odd = c.post("/api/invoices", json!({ "status": "PAID" })).await;
    assert_eq!(odd.status, StatusCode::BAD_REQUEST);
    let odd = c
        .put(&format!("/api/invoices/{id}"), json!({ "status": "x" }))
        .await;
    assert_eq!(odd.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        c.get("/api/invoices").await.body.as_array().unwrap().len(),
        3
    );

    let paid = c
        .put(&format!("/api/invoices/{id}"), json!({ "status": "paid" }))
        .await;
    assert!(paid.body["paid_at"].is_string());
    let locked = c
        .put(&format!("/api/invoices/{id}"), json!({ "status": "draft" }))
        .await;
    assert_eq!(locked.status, StatusCode::CONFLICT);
    assert_eq!(locked.body["code"], "STATUS_LOCKED");
    let keep = c.delete(&format!("/api/invoices/{id}")).await;
    assert_eq!(keep.status, StatusCode::CONFLICT);

    let void_id = from_client["id"].as_i64().unwrap();
    c.put(
        &format!("/api/invoices/{void_id}"),
        json!({ "status": "void" }),
    )
    .await;
    let soft = c.delete(&format!("/api/invoices/{void_id}")).await;
    assert_eq!(soft.body, json!({ "success": true, "soft_deleted": true }));
    assert_eq!(
        c.get("/api/invoices").await.body.as_array().unwrap().len(),
        2
    );

    // Numbers never repeat after a delete
    let next = c.post("/api/invoices", json!({})).await.body;
    assert_eq!(next["invoice_number"], format!("IK-{year}-0004"));

    let unknown_client = c
        .post("/api/invoices", json!({ "client_id": 999_999 }))
        .await;
    assert_eq!(unknown_client.status, StatusCode::BAD_REQUEST);
    assert_eq!(unknown_client.body["error"], "Client not found");
}

/// The dashboard's month-to-date trend compares with the same days of last
/// month: the 1st up to today's day number (or last month's end), not the
/// whole month, in the account's time zone.
#[sqlx::test(migrator = "MIGRATOR")]
async fn collected_prev_mtd_covers_the_same_days_of_last_month(pool: PgPool) {
    use chrono::{Datelike, Months, NaiveDate};
    let c = chef(&pool, "Trend Kitchen").await;
    let cid = c.post("/api/clients", json!({ "name": "Tess" })).await.body["id"]
        .as_i64()
        .unwrap();
    let today = today_at(0);
    let this_month = today.with_day(1).unwrap();
    let prev_start = this_month - Months::new(1);
    // `checked_sub_months` clamps (Mar 31 → Feb 28/29), like the server.
    let prev_same_day = today.checked_sub_months(Months::new(1)).unwrap();

    // A paid invoice of `amount` with paid_at at noon UTC on `day`.
    let paid = |day: NaiveDate, amount: i64| {
        let (c, pool) = (&c, &pool);
        async move {
            let inv = c
                .post(
                    "/api/invoices",
                    json!({ "client_id": cid, "status": "sent",
                            "line_items": [{ "quantity": 1, "unit_price": amount }] }),
                )
                .await;
            sqlx::query(
                "UPDATE invoices SET status = 'paid', paid_at = ($2::date + time '12:00') AT TIME ZONE 'UTC'
                  WHERE id = $1",
            )
            .bind(inv.body["id"].as_i64().unwrap())
            .bind(day)
            .execute(pool)
            .await
            .unwrap();
        }
    };
    paid(prev_start, 100).await; // counts
    if prev_same_day != prev_start {
        paid(prev_same_day, 20).await; // counts (the same day number)
    }
    let after = prev_same_day + chrono::Duration::days(1);
    if after < this_month {
        paid(after, 1000).await; // later in last month: doesn't count
    }
    paid(prev_start - chrono::Duration::days(1), 5000).await; // two months ago
    paid(today, 7).await; // this month: collected_mtd, not the comparison

    let s = c.get("/api/costs/summary").await.body;
    let expected = if prev_same_day != prev_start {
        120.0
    } else {
        100.0
    };
    assert_eq!(s["collected_prev_mtd"], expected, "{s}");
    assert_eq!(s["collected_mtd"], 7.0, "{s}");
}
