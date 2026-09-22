//! Account directory, owner-managed groups, and downloadable client view presets.
use super::{
    api::{ApiError, AppState},
    auth::AuthSession,
};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/operations", get(super::operations::overview))
        .route("/api/v1/accounts", get(accounts))
        .route("/api/v1/accounts/me", post(update_profile))
        .route("/api/v1/accounts/{id}/role", post(update_account_role))
        .route("/api/v1/account-groups", get(groups).post(create_group))
        .route(
            "/api/v1/account-groups/{id}",
            post(update_group).delete(delete_group),
        )
        .route("/api/v1/workspace-repositories", get(repositories))
        .route(
            "/api/v1/repository-group-access",
            get(repository_group_access),
        )
        .route(
            "/api/v1/repository-group-access/{resource}",
            post(update_repository_group_access),
        )
        .route(
            "/api/v1/sparse-views",
            get(sparse_views).post(create_sparse_view),
        )
        .route(
            "/api/v1/sparse-views/{id}",
            post(update_sparse_view).delete(delete_sparse_view),
        )
        .route("/api/v1/account-groups/{id}/views", get(group_views))
        .route(
            "/api/v1/account-groups/{id}/views/{resource}",
            post(select_view).delete(unselect_view),
        )
        .route_layer(middleware::from_fn(require_admin))
}

async fn require_admin(
    Extension(session): Extension<AuthSession>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    if session.user.role != "admin" {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Administrator access required.".into(),
        ));
    }
    Ok(next.run(request).await)
}

fn bad(message: &str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message.into())
}
fn conflict(error: sqlx::Error) -> ApiError {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        ApiError(
            StatusCode::CONFLICT,
            "A group with this name already exists.".into(),
        )
    } else {
        error.into()
    }
}

#[derive(Serialize, FromRow)]
struct Account {
    id: Uuid,
    email: String,
    name: Option<String>,
    role: String,
    created_at: DateTime<Utc>,
    last_login_at: DateTime<Utc>,
}
async fn accounts(State(s): State<AppState>) -> Result<Json<Vec<Account>>, ApiError> {
    Ok(Json(sqlx::query_as("SELECT id,email,COALESCE(display_name,name) AS name,role,created_at,last_login_at FROM users ORDER BY lower(email)")
        .fetch_all(&s.pool).await?))
}

#[derive(Deserialize)]
struct AccountRoleInput {
    role: String,
}

async fn update_account_role(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<AccountRoleInput>,
) -> Result<StatusCode, ApiError> {
    if session.user.role != "admin" {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Administrator access required.".into(),
        ));
    }
    if !matches!(input.role.as_str(), "user" | "admin") {
        return Err(bad("Role must be user or admin."));
    }
    let mut tx = s.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(1196577879)")
        .execute(&mut *tx)
        .await?;
    let current: Option<String> =
        sqlx::query_scalar("SELECT role FROM users WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(current) = current else {
        return Err(ApiError(StatusCode::NOT_FOUND, "Account not found.".into()));
    };
    if current == "admin" && input.role == "user" {
        let administrator_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE role='admin'")
                .fetch_one(&mut *tx)
                .await?;
        if administrator_count <= 1 {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "The last administrator cannot be changed to a regular user.".into(),
            ));
        }
    }
    sqlx::query("UPDATE users SET role=$1 WHERE id=$2")
        .bind(&input.role)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
struct ProfileInput {
    name: String,
}
async fn update_profile(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Json(input): Json<ProfileInput>,
) -> Result<StatusCode, ApiError> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err(bad("Display name must contain 1–100 characters."));
    }
    sqlx::query("UPDATE users SET display_name=$1 WHERE id=$2")
        .bind(name)
        .bind(session.user.id)
        .execute(&s.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, FromRow)]
struct Group {
    id: Uuid,
    name: String,
    description: String,
    owner_id: Uuid,
    member_ids: Vec<Uuid>,
}
async fn groups(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<Vec<Group>>, ApiError> {
    Ok(Json(sqlx::query_as("SELECT g.id,g.name,g.description,g.owner_id,ARRAY(SELECT user_id FROM account_group_members WHERE group_id=g.id ORDER BY user_id) AS member_ids FROM account_groups g WHERE $2 OR g.owner_id=$1 OR EXISTS(SELECT 1 FROM account_group_members m WHERE m.group_id=g.id AND m.user_id=$1) ORDER BY lower(g.name),g.id")
        .bind(session.user.id).bind(session.user.role == "admin").fetch_all(&s.pool).await?))
}
#[derive(Deserialize)]
struct GroupInput {
    name: String,
    description: String,
    member_ids: Vec<Uuid>,
}
impl GroupInput {
    fn normalize(&mut self, owner: Uuid) -> Result<(), ApiError> {
        self.name = self.name.trim().into();
        self.description = self.description.trim().into();
        if self.name.is_empty()
            || self.name.chars().count() > 100
            || self.name.chars().any(char::is_control)
            || self.description.chars().count() > 500
        {
            return Err(bad(
                "Group name must contain 1–100 characters; description may contain up to 500.",
            ));
        }
        self.member_ids.push(owner);
        self.member_ids.sort();
        self.member_ids.dedup();
        if self.member_ids.len() > 200 {
            return Err(bad("A group supports up to 200 members."));
        }
        Ok(())
    }
}
async fn store_group(
    s: AppState,
    owner: Uuid,
    id: Uuid,
    mut input: GroupInput,
    create: bool,
) -> Result<Json<Group>, ApiError> {
    input.normalize(owner)?;
    let mut tx = s.pool.begin().await?;
    if create {
        sqlx::query("INSERT INTO account_groups(id,name,description,owner_id) VALUES($1,$2,$3,$4)")
            .bind(id)
            .bind(&input.name)
            .bind(&input.description)
            .bind(owner)
            .execute(&mut *tx)
            .await
            .map_err(conflict)?;
    } else {
        let changed = sqlx::query(
            "UPDATE account_groups SET name=$1,description=$2 WHERE id=$3 AND owner_id=$4",
        )
        .bind(&input.name)
        .bind(&input.description)
        .bind(id)
        .bind(owner)
        .execute(&mut *tx)
        .await
        .map_err(conflict)?;
        if changed.rows_affected() == 0 {
            return Err(ApiError(
                StatusCode::FORBIDDEN,
                "Group owner required.".into(),
            ));
        }
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE id=ANY($1)")
        .bind(&input.member_ids)
        .fetch_one(&mut *tx)
        .await?;
    if count != input.member_ids.len() as i64 {
        return Err(bad("One or more accounts no longer exist."));
    }
    sqlx::query("DELETE FROM account_group_members WHERE group_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO account_group_members(group_id,user_id) SELECT $1,unnest($2::uuid[])")
        .bind(id)
        .bind(&input.member_ids)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(Group {
        id,
        name: input.name,
        description: input.description,
        owner_id: owner,
        member_ids: input.member_ids,
    }))
}
async fn create_group(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Json(input): Json<GroupInput>,
) -> Result<(StatusCode, Json<Group>), ApiError> {
    Ok((
        StatusCode::CREATED,
        store_group(s, session.user.id, Uuid::new_v4(), input, true).await?,
    ))
}
async fn update_group(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<GroupInput>,
) -> Result<Json<Group>, ApiError> {
    store_group(s, session.user.id, id, input, false).await
}
async fn delete_group(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let result = sqlx::query("DELETE FROM account_groups WHERE id=$1 AND owner_id=$2")
        .bind(id)
        .bind(session.user.id)
        .execute(&s.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Group owner required.".into(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Serialize, FromRow)]
struct Resource {
    resource_id: String,
    name: String,
}
async fn repositories(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<Vec<Resource>>, ApiError> {
    Ok(Json(sqlx::query_as("SELECT resource_id,name FROM lore_resources WHERE owner_subject=$1 OR EXISTS(SELECT 1 FROM users WHERE id::text=$1 AND role='admin') ORDER BY lower(name)").bind(session.user.id.to_string()).fetch_all(&s.pool).await?))
}

#[derive(Serialize, FromRow)]
struct RepositoryGroupAccess {
    resource_id: String,
    repository_name: String,
    group_ids: Vec<Uuid>,
}

#[derive(Serialize, FromRow)]
struct RepositoryGroupOption {
    id: Uuid,
    name: String,
}

#[derive(Serialize)]
struct RepositoryGroupAccessResponse {
    repositories: Vec<RepositoryGroupAccess>,
    groups: Vec<RepositoryGroupOption>,
}

async fn repository_group_access(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<RepositoryGroupAccessResponse>, ApiError> {
    let repositories = sqlx::query_as(
        "SELECT r.resource_id,r.name AS repository_name,ARRAY(SELECT access.group_id FROM repository_account_group_access access WHERE access.resource_id=r.resource_id ORDER BY access.group_id) AS group_ids FROM lore_resources r WHERE r.owner_subject=$1 OR $2 ORDER BY lower(r.name),r.resource_id"
    )
    .bind(session.user.id.to_string())
    .bind(session.user.role == "admin")
    .fetch_all(&s.pool)
    .await?;
    let groups = if repositories.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as("SELECT id,name FROM account_groups ORDER BY lower(name),id")
            .fetch_all(&s.pool)
            .await?
    };
    Ok(Json(RepositoryGroupAccessResponse {
        repositories,
        groups,
    }))
}

#[derive(Deserialize)]
struct RepositoryGroupAccessInput {
    group_ids: Vec<Uuid>,
}

async fn update_repository_group_access(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(resource_id): Path<String>,
    Json(mut input): Json<RepositoryGroupAccessInput>,
) -> Result<StatusCode, ApiError> {
    input.group_ids.sort();
    input.group_ids.dedup();
    if input.group_ids.len() > 200 {
        return Err(bad("A repository supports up to 200 account groups."));
    }

    let mut tx = s.pool.begin().await?;
    let manageable: Option<String> = sqlx::query_scalar(
        "SELECT resource_id FROM lore_resources WHERE resource_id=$1 AND (owner_subject=$2 OR $3) FOR UPDATE",
    )
    .bind(&resource_id)
    .bind(session.user.id.to_string())
    .bind(session.user.role == "admin")
    .fetch_one(&mut *tx)
    .await?;
    if manageable.is_none() {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Repository owner or administrator required.".into(),
        ));
    }

    let group_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM account_groups WHERE id=ANY($1)")
            .bind(&input.group_ids)
            .fetch_one(&mut *tx)
            .await?;
    if group_count != input.group_ids.len() as i64 {
        return Err(bad("One or more account groups no longer exist."));
    }

    sqlx::query("DELETE FROM repository_account_group_access WHERE resource_id=$1")
        .bind(&resource_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO repository_account_group_access(resource_id,group_id) SELECT $1,unnest($2::uuid[])")
        .bind(&resource_id)
        .bind(&input.group_ids)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Serialize, FromRow)]
struct SparseWorkspaceView {
    id: Uuid,
    name: String,
    resource_id: String,
    repository_name: String,
    mode: String,
    rules: String,
    owner_id: Uuid,
    updated_at: DateTime<Utc>,
    can_manage: bool,
}
async fn sparse_views(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<Vec<SparseWorkspaceView>>, ApiError> {
    Ok(Json(sqlx::query_as("SELECT v.id,v.name,v.resource_id,r.name AS repository_name,v.mode,v.rules,v.owner_id,v.updated_at,(v.owner_id=$1 AND (r.owner_subject=$2 OR $3)) AS can_manage FROM sparse_workspace_views v JOIN lore_resources r USING(resource_id) WHERE $3 OR v.owner_id=$1 OR EXISTS(SELECT 1 FROM account_group_view_selections selected JOIN account_groups g ON g.id=selected.group_id WHERE selected.view_id=v.id AND (g.owner_id=$1 OR EXISTS(SELECT 1 FROM account_group_members member WHERE member.group_id=g.id AND member.user_id=$1))) ORDER BY lower(v.name),v.id")
        .bind(session.user.id).bind(session.user.id.to_string()).bind(session.user.role == "admin").fetch_all(&s.pool).await?))
}

#[derive(Serialize, FromRow)]
struct GroupViewSelection {
    resource_id: String,
    repository_name: String,
    view_id: Uuid,
    view_name: String,
    mode: String,
    rules: String,
    updated_at: DateTime<Utc>,
    can_manage: bool,
}
async fn group_views(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<GroupViewSelection>>, ApiError> {
    let visible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_groups g WHERE g.id=$1 AND ($3 OR g.owner_id=$2 OR EXISTS(SELECT 1 FROM account_group_members m WHERE m.group_id=g.id AND m.user_id=$2)))")
        .bind(id).bind(session.user.id).bind(session.user.role == "admin").fetch_one(&s.pool).await?;
    if !visible {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Group membership required.".into(),
        ));
    }
    Ok(Json(sqlx::query_as("SELECT selected.resource_id,r.name AS repository_name,v.id AS view_id,v.name AS view_name,v.mode,v.rules,v.updated_at,(g.owner_id=$2 AND v.owner_id=$2 AND (r.owner_subject=$3 OR EXISTS(SELECT 1 FROM users WHERE id=$2 AND role='admin'))) AS can_manage FROM account_group_view_selections selected JOIN sparse_workspace_views v ON v.id=selected.view_id AND v.resource_id=selected.resource_id JOIN lore_resources r ON r.resource_id=selected.resource_id JOIN account_groups g ON g.id=selected.group_id WHERE selected.group_id=$1 ORDER BY lower(r.name)")
        .bind(id).bind(session.user.id).bind(session.user.id.to_string()).fetch_all(&s.pool).await?))
}
#[derive(Deserialize)]
struct ViewRulesInput {
    mode: String,
    rules: String,
}
impl ViewRulesInput {
    fn normalize(&mut self) -> Result<(), ApiError> {
        if !matches!(self.mode.as_str(), "full" | "sparse") {
            return Err(bad("Mode must be full or sparse."));
        }
        self.rules = self.rules.replace("\r\n", "\n");
        if self.rules.len() > 10000
            || self
                .rules
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(bad(
                "View rules must be valid text of at most 10,000 bytes.",
            ));
        }
        if self.mode == "full" {
            self.rules.clear();
        } else if !self
            .rules
            .lines()
            .any(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
        {
            return Err(bad("Sparse mode requires at least one view rule."));
        }
        Ok(())
    }
}

fn normalize_view_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err(bad("View name must contain 1–100 characters."));
    }
    Ok(name.into())
}

fn view_conflict(error: sqlx::Error) -> ApiError {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        ApiError(
            StatusCode::CONFLICT,
            "A Sparse View with this name already exists.".into(),
        )
    } else {
        error.into()
    }
}

#[derive(Deserialize)]
struct CreateViewInput {
    name: String,
    resource_id: String,
    mode: String,
    rules: String,
}
async fn create_sparse_view(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Json(input): Json<CreateViewInput>,
) -> Result<(StatusCode, Json<SparseWorkspaceView>), ApiError> {
    let name = normalize_view_name(&input.name)?;
    let mut rules = ViewRulesInput {
        mode: input.mode,
        rules: input.rules,
    };
    rules.normalize()?;
    let repository_name: Option<String> = sqlx::query_scalar(
        "SELECT name FROM lore_resources WHERE resource_id=$1 AND (owner_subject=$2 OR EXISTS(SELECT 1 FROM users WHERE id::text=$2 AND role='admin'))",
    )
    .bind(&input.resource_id)
    .bind(session.user.id.to_string())
    .fetch_optional(&s.pool)
    .await?;
    let Some(repository_name) = repository_name else {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Repository owner or administrator required.".into(),
        ));
    };
    let id = Uuid::new_v4();
    let updated_at: DateTime<Utc> = sqlx::query_scalar("INSERT INTO sparse_workspace_views(id,owner_id,resource_id,name,mode,rules) VALUES($1,$2,$3,$4,$5,$6) RETURNING updated_at")
        .bind(id).bind(session.user.id).bind(&input.resource_id).bind(&name).bind(&rules.mode).bind(&rules.rules).fetch_one(&s.pool).await.map_err(view_conflict)?;
    Ok((
        StatusCode::CREATED,
        Json(SparseWorkspaceView {
            id,
            name,
            resource_id: input.resource_id,
            repository_name,
            mode: rules.mode,
            rules: rules.rules,
            owner_id: session.user.id,
            updated_at,
            can_manage: true,
        }),
    ))
}

#[derive(Deserialize)]
struct UpdateViewInput {
    name: String,
    mode: String,
    rules: String,
}
async fn update_sparse_view(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateViewInput>,
) -> Result<StatusCode, ApiError> {
    let name = normalize_view_name(&input.name)?;
    let mut rules = ViewRulesInput {
        mode: input.mode,
        rules: input.rules,
    };
    rules.normalize()?;
    let changed = sqlx::query("UPDATE sparse_workspace_views v SET name=$1,mode=$2,rules=$3,updated_at=now() FROM lore_resources r WHERE v.id=$4 AND v.owner_id=$5 AND r.resource_id=v.resource_id AND (r.owner_subject=$6 OR EXISTS(SELECT 1 FROM users WHERE id=$5 AND role='admin'))")
        .bind(name).bind(rules.mode).bind(rules.rules).bind(id).bind(session.user.id).bind(session.user.id.to_string()).execute(&s.pool).await.map_err(view_conflict)?;
    if changed.rows_affected() == 0 {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Sparse View ownership required.".into(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_sparse_view(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = sqlx::query("DELETE FROM sparse_workspace_views WHERE id=$1 AND owner_id=$2")
        .bind(id)
        .bind(session.user.id)
        .execute(&s.pool)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "Sparse View not found or ownership required.".into(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SelectViewInput {
    view_id: Uuid,
}
async fn select_view(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path((group_id, resource_id)): Path<(Uuid, String)>,
    Json(input): Json<SelectViewInput>,
) -> Result<StatusCode, ApiError> {
    let mut tx = s.pool.begin().await?;
    let allowed: Option<Uuid> = sqlx::query_scalar("SELECT g.id FROM account_groups g JOIN sparse_workspace_views v ON v.id=$3 JOIN lore_resources r ON r.resource_id=$4 WHERE g.id=$1 AND g.owner_id=$2 AND v.owner_id=$2 AND v.resource_id=$4 AND (r.owner_subject=$5 OR EXISTS(SELECT 1 FROM users WHERE id=$2 AND role='admin')) FOR UPDATE OF g,v,r")
        .bind(group_id).bind(session.user.id).bind(input.view_id).bind(&resource_id).bind(session.user.id.to_string()).fetch_optional(&mut *tx).await?;
    if allowed.is_none() {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Group and Sparse View ownership, and repository access required.".into(),
        ));
    }
    sqlx::query("INSERT INTO account_group_view_selections(group_id,resource_id,view_id) VALUES($1,$2,$3) ON CONFLICT(group_id,resource_id) DO UPDATE SET view_id=EXCLUDED.view_id,selected_at=now()")
        .bind(group_id).bind(resource_id).bind(input.view_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn unselect_view(
    State(s): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path((group_id, resource_id)): Path<(Uuid, String)>,
) -> Result<StatusCode, ApiError> {
    let deleted = sqlx::query("DELETE FROM account_group_view_selections selected USING account_groups g WHERE selected.group_id=g.id AND selected.group_id=$1 AND selected.resource_id=$2 AND g.owner_id=$3")
        .bind(group_id).bind(resource_id).bind(session.user.id).execute(&s.pool).await?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "Group view selection not found or ownership required.".into(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn view_rules_preserve_order_and_reject_invalid_input() {
        let mut input = ViewRulesInput {
            mode: "sparse".into(),
            rules: "**\r\n!/src/\r\n/src/generated/\n".into(),
        };
        input.normalize().unwrap();
        assert_eq!(input.rules, "**\n!/src/\n/src/generated/\n");
        input.rules = "# comment only".into();
        assert!(input.normalize().is_err());
        input.rules = "a\0b".into();
        assert!(input.normalize().is_err());
        input.rules = "a".repeat(10001);
        assert!(input.normalize().is_err());
        input.rules = "**".into();
        input.mode = "full".into();
        input.normalize().unwrap();
        assert!(input.rules.is_empty());
    }
    #[test]
    fn group_members_always_include_owner_once() {
        let owner = Uuid::new_v4();
        let mut input = GroupInput {
            name: "  Team  ".into(),
            description: "".into(),
            member_ids: vec![owner, owner],
        };
        input.normalize(owner).unwrap();
        assert_eq!(input.name, "Team");
        assert_eq!(input.member_ids, vec![owner]);
        input.name = " ".into();
        assert!(input.normalize(owner).is_err());
    }
}
