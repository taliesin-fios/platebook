// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Per-account starter content offered during onboarding: the recipes and
//! menu templates in `starter_library.json`, and one sample client.
//!
//! Both seeds are idempotent per account: running them twice adds nothing.
//! Run them on a tenant transaction (`db::begin_tenant`).

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::routes::onboarding::SeedLibrary;
use sqlx::PgConnection;

#[derive(Deserialize)]
struct Library {
    recipes: Vec<SeedRecipe>,
    templates: Vec<SeedTemplate>,
}

#[derive(Deserialize)]
struct SeedRecipe {
    name: String,
    category: String,
    session_fit: String,
    prep_time_min: i32,
    cook_time_min: i32,
    hold_quality: i32,
    reheat_quality: i32,
    key_overlap_ingredients: Value,
    chef_notes: String,
    ingredients: Vec<SeedIngredient>,
}

#[derive(Deserialize)]
struct SeedIngredient {
    name: String,
    qty_4_serving: String,
    qty_6_serving: String,
    store_section: String,
}

#[derive(Deserialize)]
struct SeedTemplate {
    name: String,
    description: String,
    recipes: Vec<String>,
}

static LIBRARY: LazyLock<Library> = LazyLock::new(|| {
    serde_json::from_str(include_str!("starter_library.json")).expect("valid starter library")
});

/// `{ recipes, templates }` counts, as the Node route returns them.
pub async fn seed_starter_library(
    conn: &mut PgConnection,
    account_id: i64,
) -> Result<SeedLibrary, sqlx::Error> {
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM recipes WHERE account_id = $1 AND import_source = 'seeded')",
    )
    .bind(account_id)
    .fetch_one(&mut *conn)
    .await?;
    if already {
        return Ok(SeedLibrary::default());
    }

    let mut ids: HashMap<&str, i64> = HashMap::new();
    for r in &LIBRARY.recipes {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO recipes (account_id, name, category, session_fit, prep_time_min, cook_time_min,
               servings_base, hold_quality, reheat_quality, key_overlap_ingredients,
               chef_notes, import_source)
             VALUES ($1, $2, $3, $4, $5, $6, 4, $7, $8, $9, $10, 'seeded') RETURNING id",
        )
        .bind(account_id)
        .bind(&r.name)
        .bind(&r.category)
        .bind(&r.session_fit)
        .bind(r.prep_time_min)
        .bind(r.cook_time_min)
        .bind(r.hold_quality)
        .bind(r.reheat_quality)
        .bind(&r.key_overlap_ingredients)
        .bind(&r.chef_notes)
        .fetch_one(&mut *conn)
        .await?;
        ids.insert(r.name.as_str(), id);
        for i in &r.ingredients {
            sqlx::query(
                "INSERT INTO ingredients (recipe_id, name, qty_4_serving, qty_6_serving, store_section)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(&i.name)
            .bind(&i.qty_4_serving)
            .bind(&i.qty_6_serving)
            .bind(&i.store_section)
            .execute(&mut *conn)
            .await?;
        }
    }

    // Template names are unique per account; keep any the chef already made.
    let mut templates = 0;
    for t in &LIBRARY.templates {
        let recipe_ids: Vec<i64> = t
            .recipes
            .iter()
            .filter_map(|n| ids.get(n.as_str()).copied())
            .collect();
        let res = sqlx::query(
            "INSERT INTO session_templates (account_id, name, description, recipe_ids, is_default)
             VALUES ($1, $2, $3, $4, true) ON CONFLICT (account_id, name) DO NOTHING",
        )
        .bind(account_id)
        .bind(&t.name)
        .bind(&t.description)
        .bind(json!(recipe_ids))
        .execute(&mut *conn)
        .await?;
        templates += res.rows_affected();
    }

    Ok(SeedLibrary {
        recipes: LIBRARY.recipes.len() as u32,
        templates: templates as u32,
    })
}

/// One demo client (`is_sample`); false if the account already has one.
pub async fn seed_sample_client(
    conn: &mut PgConnection,
    account_id: i64,
) -> Result<bool, sqlx::Error> {
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM clients WHERE account_id = $1 AND is_sample)",
    )
    .bind(account_id)
    .fetch_one(&mut *conn)
    .await?;
    if already {
        return Ok(false);
    }
    sqlx::query(
        r#"INSERT INTO clients (
             account_id, name, household_type, adults, children, serving_size,
             allergens, dietary_styles, liked_proteins, spice_tolerance,
             preferred_day, grocery_handling, chef_notes, is_sample
           ) VALUES (
             $1, 'The Chen Family (sample)', 'Small Family', 2, 2, 6,
             '["Tree Nuts"]', '["Dairy-Free"]', '["Chicken","Beef"]', 'Mild',
             'Sunday', 'Chef Shops',
             'Two young kids, mild spice only, parking in driveway', true
           )"#,
    )
    .bind(account_id)
    .execute(&mut *conn)
    .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn library_parses_and_templates_name_real_recipes() {
        let lib = &*super::LIBRARY;
        assert!(lib.recipes.len() > 10);
        for t in &lib.templates {
            for name in &t.recipes {
                assert!(lib.recipes.iter().any(|r| &r.name == name), "{name}");
            }
        }
    }
}
