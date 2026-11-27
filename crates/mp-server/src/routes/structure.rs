//! Clients and productions: how an organization groups its projects (Organization > Client >
//! Production > Project). Both levels are optional, and removing one never removes projects:
//! what was in it moves up a level. Access is still decided per project (crate::access).

use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::access;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::AppState;

/// A field that can be left out (no change), null (clear it) or set.
pub fn some_or_null<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
    Ok(Some(Option::deserialize(d)?))
}

fn clean_name(name: &str, what: &str) -> ApiResult<String> {
    let n = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() {
        return Err(ApiError::invalid(format!("Give the {what} a name.")));
    }
    if n.chars().count() > 120 {
        return Err(ApiError::invalid(format!("Keep the {what}'s name under 120 characters.")));
    }
    Ok(n)
}

fn taken(e: sqlx::Error, msg: String) -> ApiError {
    match ApiError::from(e) {
        err if err.code == "conflict" => ApiError::conflict(msg),
        err => err,
    }
}

async fn client_org(s: &AppState, id: Uuid) -> ApiResult<Uuid> {
    sqlx::query_scalar("SELECT org_id FROM clients WHERE id = $1").bind(id).fetch_optional(&s.db).await?.ok_or_else(|| ApiError::not_found("That client"))
}

async fn production_org(s: &AppState, id: Uuid) -> ApiResult<Uuid> {
    sqlx::query_scalar("SELECT org_id FROM productions WHERE id = $1").bind(id).fetch_optional(&s.db).await?.ok_or_else(|| ApiError::not_found("That production"))
}

/// Everything an organization's admin can file projects under, with how much is in each.
pub async fn structure(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>) -> ApiResult<Json<Value>> {
    access::org_admin(&s, &user, org).await?;
    let clients = sqlx::query(
        "SELECT c.id, c.name, c.created_at,
           (SELECT count(*) FROM productions pr WHERE pr.client_id = c.id) AS productions,
           (SELECT count(*) FROM projects p WHERE p.client_id = c.id AND p.archived_at IS NULL) AS projects
         FROM clients c WHERE c.org_id = $1 ORDER BY lower(c.name)",
    )
    .bind(org)
    .fetch_all(&s.db)
    .await?;
    let productions = sqlx::query(
        "SELECT pr.id, pr.name, pr.client_id, pr.main_project_id, pr.created_at,
           (SELECT count(*) FROM projects p WHERE p.production_id = pr.id AND p.archived_at IS NULL) AS projects
         FROM productions pr WHERE pr.org_id = $1 ORDER BY lower(pr.name)",
    )
    .bind(org)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(json!({
        "clients": clients.iter().map(|r| json!({
            "id": r.get::<Uuid,_>("id"), "name": r.get::<String,_>("name"), "created_at": r.get::<DateTime<Utc>,_>("created_at"),
            "productions": r.get::<i64,_>("productions"), "projects": r.get::<i64,_>("projects"),
        })).collect::<Vec<_>>(),
        "productions": productions.iter().map(|r| json!({
            "id": r.get::<Uuid,_>("id"), "name": r.get::<String,_>("name"), "client_id": r.get::<Option<Uuid>,_>("client_id"),
            "main_project_id": r.get::<Option<Uuid>,_>("main_project_id"), "created_at": r.get::<DateTime<Utc>,_>("created_at"),
            "projects": r.get::<i64,_>("projects"),
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct NewClient {
    name: String,
}

pub async fn create_client(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>, Json(b): Json<NewClient>) -> ApiResult<Json<Value>> {
    access::org_admin(&s, &user, org).await?;
    let name = clean_name(&b.name, "client")?;
    let mut tx = s.db.begin().await?;
    let id: Uuid = sqlx::query_scalar("INSERT INTO clients (org_id, name) VALUES ($1,$2) RETURNING id")
        .bind(org)
        .bind(&name)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| taken(e, format!("There is already a client called {name}.")))?;
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.client_created", json!({ "id": id, "name": name })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "id": id, "name": name })))
}

#[derive(Deserialize)]
pub struct EditClient {
    name: String,
}

pub async fn update_client(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, Json(b): Json<EditClient>) -> ApiResult<Json<Value>> {
    let org = client_org(&s, id).await?;
    access::org_admin(&s, &user, org).await?;
    let name = clean_name(&b.name, "client")?;
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE clients SET name = $2 WHERE id = $1")
        .bind(id)
        .bind(&name)
        .execute(&mut *tx)
        .await
        .map_err(|e| taken(e, format!("There is already a client called {name}.")))?;
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.client_changed", json!({ "id": id, "name": name })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "id": id, "name": name })))
}

/// Removes the client only: its productions and projects stay, no longer under a client.
pub async fn delete_client(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let org = client_org(&s, id).await?;
    access::org_admin(&s, &user, org).await?;
    let mut tx = s.db.begin().await?;
    // Two productions of the same name, one under this client and one not, would clash.
    let clash: Option<String> = sqlx::query_scalar(
        "SELECT a.name FROM productions a JOIN productions b ON b.org_id = a.org_id AND b.client_id IS NULL AND lower(b.name) = lower(a.name)
         WHERE a.client_id = $1 LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(n) = clash {
        return Err(ApiError::conflict(format!("{n} would clash with the production of that name that has no client. Rename one first.")));
    }
    let name: String = sqlx::query_scalar("DELETE FROM clients WHERE id = $1 RETURNING name").bind(id).fetch_one(&mut *tx).await?;
    // Policy rules on it no longer cover anything.
    sqlx::query("DELETE FROM policy_rules WHERE scope_kind = 'client' AND scope_id = $1").bind(id).execute(&mut *tx).await?;
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.client_removed", json!({ "id": id, "name": name })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct NewProduction {
    name: String,
    client: Option<Uuid>,
}

pub async fn create_production(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>, Json(b): Json<NewProduction>) -> ApiResult<Json<Value>> {
    access::org_admin(&s, &user, org).await?;
    let name = clean_name(&b.name, "production")?;
    if let Some(c) = b.client {
        if client_org(&s, c).await? != org {
            return Err(ApiError::invalid("That client belongs to another organization."));
        }
    }
    let mut tx = s.db.begin().await?;
    let id: Uuid = sqlx::query_scalar("INSERT INTO productions (org_id, client_id, name) VALUES ($1,$2,$3) RETURNING id")
        .bind(org)
        .bind(b.client)
        .bind(&name)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| taken(e, format!("There is already a production called {name} there.")))?;
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.production_created", json!({ "id": id, "name": name, "client": b.client })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "id": id, "name": name, "client_id": b.client })))
}

#[derive(Deserialize)]
pub struct EditProduction {
    name: Option<String>,
    #[serde(default, deserialize_with = "some_or_null")]
    client: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "some_or_null")]
    main_project: Option<Option<Uuid>>,
}

pub async fn update_production(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, Json(b): Json<EditProduction>) -> ApiResult<Json<Value>> {
    let org = production_org(&s, id).await?;
    access::org_admin(&s, &user, org).await?;
    let mut tx = s.db.begin().await?;
    if let Some(name) = b.name.as_deref() {
        let name = clean_name(name, "production")?;
        sqlx::query("UPDATE productions SET name = $2 WHERE id = $1")
            .bind(id)
            .bind(&name)
            .execute(&mut *tx)
            .await
            .map_err(|e| taken(e, format!("There is already a production called {name} there.")))?;
    }
    if let Some(client) = b.client {
        if let Some(c) = client {
            if client_org(&s, c).await? != org {
                return Err(ApiError::invalid("That client belongs to another organization."));
            }
        }
        sqlx::query("UPDATE productions SET client_id = $2 WHERE id = $1")
            .bind(id)
            .bind(client)
            .execute(&mut *tx)
            .await
            .map_err(|e| taken(e, "That client already has a production of this name. Rename one first.".into()))?;
        // Its projects go with it.
        sqlx::query("UPDATE projects SET client_id = $2 WHERE production_id = $1").bind(id).bind(client).execute(&mut *tx).await?;
    }
    if let Some(main) = b.main_project {
        if let Some(p) = main {
            let inside: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM projects WHERE id = $1 AND production_id = $2)").bind(p).bind(id).fetch_one(&mut *tx).await?;
            if !inside {
                return Err(ApiError::invalid("The main project has to be one of this production's projects."));
            }
        }
        sqlx::query("UPDATE productions SET main_project_id = $2 WHERE id = $1").bind(id).bind(main).execute(&mut *tx).await?;
    }
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.production_changed", json!({ "id": id, "name": b.name, "client": b.client, "main_project": b.main_project })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "ok": true })))
}

/// Removes the production only: its projects stay, still under its client if it had one.
pub async fn delete_production(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let org = production_org(&s, id).await?;
    access::org_admin(&s, &user, org).await?;
    let mut tx = s.db.begin().await?;
    let name: String = sqlx::query_scalar("DELETE FROM productions WHERE id = $1 RETURNING name").bind(id).fetch_one(&mut *tx).await?;
    sqlx::query("DELETE FROM policy_rules WHERE scope_kind = 'production' AND scope_id = $1").bind(id).execute(&mut *tx).await?;
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.production_removed", json!({ "id": id, "name": name })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "ok": true })))
}

/// File a project under a production, or straight under a client, or neither. A production
/// decides the client; a project that leaves a production stops being its main project.
pub async fn place_project(tx: &mut Transaction<'_, Postgres>, org: Uuid, project: Uuid, client: Option<Uuid>, production: Option<Uuid>) -> ApiResult<()> {
    let client = match production {
        Some(pr) => {
            let row = sqlx::query("SELECT org_id, client_id FROM productions WHERE id = $1").bind(pr).fetch_optional(&mut **tx).await?.ok_or_else(|| ApiError::not_found("That production"))?;
            if row.get::<Uuid, _>("org_id") != org {
                return Err(ApiError::invalid("That production belongs to another organization."));
            }
            row.get::<Option<Uuid>, _>("client_id")
        }
        None => {
            if let Some(c) = client {
                let o: Option<Uuid> = sqlx::query_scalar("SELECT org_id FROM clients WHERE id = $1").bind(c).fetch_optional(&mut **tx).await?;
                match o {
                    None => return Err(ApiError::not_found("That client")),
                    Some(o) if o != org => return Err(ApiError::invalid("That client belongs to another organization.")),
                    _ => {}
                }
            }
            client
        }
    };
    sqlx::query("UPDATE productions SET main_project_id = NULL WHERE main_project_id = $1 AND id IS DISTINCT FROM $2").bind(project).bind(production).execute(&mut **tx).await?;
    sqlx::query("UPDATE projects SET client_id = $2, production_id = $3 WHERE id = $1").bind(project).bind(client).bind(production).execute(&mut **tx).await?;
    Ok(())
}
