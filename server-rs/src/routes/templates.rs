// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/templates.js`.
//!
//! GET    /api/templates      — this account's templates with their recipes
//! POST   /api/templates      — create
//! PUT    /api/templates/:id  — replace name, description, recipe_ids
//! DELETE /api/templates/:id

use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgConnection;
use ts_rs::TS;

use crate::db::Tenant;
use crate::error::{AppError, DbResult, is_unique_violation};
use crate::js;
use crate::models::{SUCCESS, SessionTemplateRow, Success};
use crate::ownership::{Scoped, owned_ids};
use crate::routes::{JsonBody, parse_id};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", put(update).delete(remove))
}

/// The recipe summary attached to a template.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct TemplateRecipe {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub category: Option<String>,
    pub session_fit: Option<String>,
}

/// GET /api/templates and PUT /api/templates/:id.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct TemplateWithRecipes {
    #[serde(flatten)]
    #[ts(flatten)]
    pub template: SessionTemplateRow,
    pub recipes: Vec<TemplateRecipe>,
}

/// Incoming `recipe_ids` → deduped positive integers the account owns.
async fn sanitize_recipe_ids(
    conn: &mut PgConnection,
    raw: Option<&Value>,
    account_id: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    let Some(Value::Array(items)) = raw else {
        return Ok(vec![]);
    };
    let mut ids: Vec<i64> = Vec::new();
    for n in items.iter().filter_map(js::parse_int).filter(|n| *n > 0) {
        if !ids.contains(&n) {
            ids.push(n);
        }
    }
    owned_ids(conn, Scoped::Recipes, &ids, account_id).await
}

async fn attach_recipes(
    conn: &mut PgConnection,
    template: SessionTemplateRow,
    account_id: i64,
) -> Result<TemplateWithRecipes, sqlx::Error> {
    let ids = template.ids();
    let recipes = if ids.is_empty() {
        vec![]
    } else {
        sqlx::query_as::<_, TemplateRecipe>(
            "SELECT id, name, category, session_fit FROM recipes
             WHERE account_id = $1 AND id = ANY($2) AND is_active ORDER BY id",
        )
        .bind(account_id)
        .bind(&ids)
        .fetch_all(conn)
        .await?
    };
    Ok(TemplateWithRecipes { template, recipes })
}

fn duplicate_name() -> AppError {
    AppError::new(
        StatusCode::CONFLICT,
        "DUPLICATE_NAME",
        "A template with that name already exists",
    )
}

fn not_found() -> AppError {
    AppError::not_found_msg("Template not found")
}

/// `name` must be a non-blank value; returns it trimmed.
fn required_name(body: &JsonBody) -> Result<String, AppError> {
    let name = body
        .get("name")
        .filter(|v| js::truthy(v))
        .map(|v| js::string(v).trim().to_string())
        .unwrap_or_default();
    if name.is_empty() {
        return Err(AppError::validation("Name is required"));
    }
    Ok(name)
}

async fn list(mut t: Tenant) -> DbResult<Json<Vec<TemplateWithRecipes>>> {
    let id = t.account.id;
    let rows = sqlx::query_as::<_, SessionTemplateRow>(
        r#"SELECT * FROM session_templates WHERE account_id = $1
           ORDER BY is_default DESC NULLS LAST, name COLLATE "C" ASC"#,
    )
    .bind(id)
    .fetch_all(&mut *t.tx)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(attach_recipes(&mut t.tx, row, id).await?);
    }
    t.tx.commit().await?;
    Ok(Json(out))
}

async fn create(mut t: Tenant, body: JsonBody) -> DbResult<(StatusCode, Json<SessionTemplateRow>)> {
    let id = t.account.id;
    let name = required_name(&body)?;
    let ids = sanitize_recipe_ids(&mut t.tx, body.get("recipe_ids"), id).await?;
    let row = sqlx::query_as::<_, SessionTemplateRow>(
        "INSERT INTO session_templates (account_id, name, description, recipe_ids)
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(id)
    .bind(&name)
    .bind(body.text_or_none("description"))
    .bind(json!(ids))
    .fetch_one(&mut *t.tx)
    .await;
    let row = match row {
        Ok(r) => r,
        Err(e) if is_unique_violation(&e) => return Err(duplicate_name().into()),
        Err(e) => return Err(e.into()),
    };
    t.tx.commit().await?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn update(
    mut t: Tenant,
    Path(raw_id): Path<String>,
    body: JsonBody,
) -> DbResult<Json<TemplateWithRecipes>> {
    let account = t.account.id;
    let name = required_name(&body)?;
    let Some(id) = parse_id(&raw_id) else {
        return Err(not_found().into());
    };
    if !crate::ownership::owned(&mut t.tx, Scoped::SessionTemplates, id, account).await? {
        return Err(not_found().into());
    }
    let ids = sanitize_recipe_ids(&mut t.tx, body.get("recipe_ids"), account).await?;
    let row = sqlx::query_as::<_, SessionTemplateRow>(
        "UPDATE session_templates SET name = $1, description = $2, recipe_ids = $3
         WHERE id = $4 AND account_id = $5 RETURNING *",
    )
    .bind(&name)
    .bind(body.text_or_none("description"))
    .bind(json!(ids))
    .bind(id)
    .bind(account)
    .fetch_one(&mut *t.tx)
    .await;
    let row = match row {
        Ok(r) => r,
        Err(e) if is_unique_violation(&e) => return Err(duplicate_name().into()),
        Err(e) => return Err(e.into()),
    };
    let res = attach_recipes(&mut t.tx, row, account).await?;
    t.tx.commit().await?;
    Ok(Json(res))
}

async fn remove(mut t: Tenant, Path(raw_id): Path<String>) -> DbResult<Json<Success>> {
    let Some(id) = parse_id(&raw_id) else {
        return Err(not_found().into());
    };
    let res = sqlx::query("DELETE FROM session_templates WHERE id = $1 AND account_id = $2")
        .bind(id)
        .bind(t.account.id)
        .execute(&mut *t.tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(not_found().into());
    }
    t.tx.commit().await?;
    Ok(Json(SUCCESS))
}
