//! Local Lore repository folders archived as tar.gz, without custom Lore commands.
use super::{
    api::{ApiError, AppState},
    auth::AuthSession,
    repositories::validate_name,
    repository_access,
};
use anyhow::{Context, Result, ensure};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool};
use std::{
    path::{Path as FsPath, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/server-backups/local", post(create_server))
        .route(
            "/api/v1/repositories/{name}/backups",
            get(list_repository).post(create),
        )
        .route("/api/v1/repository-backups", get(list_all))
        .route("/api/v1/repository-backups/{id}", get(detail))
        .route("/api/v1/repository-backups/{id}/verify", post(verify))
        .route(
            "/api/v1/repository-backups/{id}/restore-preview",
            post(preview),
        )
        .route("/api/v1/repository-backups/{id}/restore", post(restore))
        .route("/api/v1/repository-restores/{id}", get(restore_detail))
}

#[derive(Serialize, Deserialize, FromRow, Clone)]
struct Policy {
    root_branch: String,
    link_path: String,
    auto_update: bool,
}
#[derive(Serialize, Deserialize, FromRow, Clone)]
struct View {
    name: String,
    mode: String,
    rules: String,
}
#[derive(Serialize, Deserialize, Clone)]
struct HubMetadata {
    branches: Vec<String>,
    groups: Vec<Uuid>,
    policies: Vec<Policy>,
    views: Vec<View>,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    format: u32,
    folder: Value,
    hub: HubMetadata,
    archive_sha256: String,
    #[serde(default)]
    repositories: Vec<Value>,
}
#[derive(Serialize, FromRow)]
pub struct Backup {
    id: Uuid,
    resource_id: String,
    repository_name: String,
    storage_backend: String,
    requested_by: Uuid,
    status: String,
    stage: String,
    manifest: Option<String>,
    archive_sha256: Option<String>,
    size_bytes: Option<i64>,
    error: Option<String>,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}
#[derive(Serialize, FromRow)]
pub struct Restore {
    id: Uuid,
    backup_id: Uuid,
    requested_by: Uuid,
    target_name: String,
    target_backend: String,
    target_resource_id: Option<String>,
    target_path: Option<String>,
    status: String,
    stage: String,
    error: Option<String>,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

fn bad(message: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message.into())
}
fn internal(error: impl std::fmt::Display) -> ApiError {
    tracing::error!(%error, "repository folder backup failed");
    ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Repository folder backup service failed.".into(),
    )
}
fn unavailable(error: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::SERVICE_UNAVAILABLE, error.to_string())
}
fn conflict(error: sqlx::Error) -> ApiError {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        ApiError(
            StatusCode::CONFLICT,
            "An operation is already running for this repository or folder.".into(),
        )
    } else {
        internal(error)
    }
}
#[derive(Clone, Default)]
pub struct BackupPaths {
    pub archive_root: Option<PathBuf>,
    pub repository_root: Option<PathBuf>,
    pub server_container: Option<String>,
}
impl BackupPaths {
    pub fn from_env() -> Self {
        Self {
            server_container: std::env::var("LOREHUB_BACKUP_SERVER_CONTAINER").ok(),
            archive_root: std::env::var_os("LOREHUB_BACKUP_DIR").map(PathBuf::from),
            repository_root: std::env::var_os("LOREHUB_REPOSITORY_DIR").map(PathBuf::from),
        }
    }
    fn directory(path: &Option<PathBuf>, key: &str) -> Result<PathBuf> {
        let path = path
            .as_ref()
            .with_context(|| format!("Configure {key} first"))?;
        ensure!(
            path.is_absolute() && path.is_dir(),
            "{key} must be an existing absolute directory"
        );
        ensure!(
            path.canonicalize()? == *path,
            "{key} must not contain symbolic links"
        );
        Ok(path.clone())
    }
    fn backup_root(&self) -> Result<PathBuf> {
        Self::directory(&self.archive_root, "LOREHUB_BACKUP_DIR")
    }
}
fn archive_path(root: &FsPath, id: Uuid) -> PathBuf {
    root.join(id.to_string()).join("repository.tar.gz")
}
fn restore_path(root: &FsPath, name: &str) -> Result<PathBuf> {
    validate_name(name)?;
    let parent = root.join("restored");
    if parent.exists() {
        ensure!(
            parent.canonicalize()? == parent,
            "Restore directory must not be a symbolic link"
        );
    }
    let target = parent.join(name);
    ensure!(
        std::fs::symlink_metadata(&target).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "Restore folder already exists; choose a new name"
    );
    Ok(target)
}
async fn private_directory(path: &FsPath) -> Result<()> {
    match tokio::fs::create_dir(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    ensure!(
        path.is_dir() && path.canonicalize()? == path,
        "Storage path must be a real directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    }
    Ok(())
}
async fn folder_command(request: Value) -> Result<Value> {
    let server_snapshot = request["action"] == "snapshot-server";
    let python = std::env::var_os("LOREHUB_BACKUP_PYTHON").unwrap_or_else(|| "python3".into());
    let mut child = Command::new(python)
        .args([
            "-I",
            "-c",
            include_str!("../../scripts/repository-folder-archive.py"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(!server_snapshot)
        .spawn()
        .context("Folder backups require Python 3.9+ on the coordinator")?;
    child
        .stdin
        .take()
        .context("archive helper stdin")?
        .write_all(&serde_json::to_vec(&request)?)
        .await?;
    let output = timeout(Duration::from_secs(45 * 60), child.wait_with_output())
        .await
        .context("Folder archive operation timed out")??;
    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
            .trim()
            .chars()
            .take(1000)
            .collect::<String>()
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}
async fn manageable(
    state: &AppState,
    session: &AuthSession,
    name: &str,
) -> Result<String, ApiError> {
    repository_access::manageable_by_name(&state.pool, &session.user.id.to_string(), name)
        .await?
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                "Repository not found or owner access required.".into(),
            )
        })
}
async fn authorized(state: &AppState, session: &AuthSession, id: Uuid) -> Result<Backup, ApiError> {
    let backup: Backup = sqlx::query_as("SELECT * FROM repository_backups WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Backup not found.".into()))?;
    // A new owner controls existing repository backups. After deletion, the requester/admin retains recovery access.
    let owner: Option<Option<String>> =
        sqlx::query_scalar("SELECT owner_subject FROM lore_resources WHERE resource_id=$1")
            .bind(&backup.resource_id)
            .fetch_optional(&state.pool)
            .await?;
    let role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id=$1")
        .bind(session.user.id)
        .fetch_optional(&state.pool)
        .await?;
    let allowed = role.as_deref() == Some("admin")
        || (backup.resource_id != "server:local"
            && role.is_some()
            && match owner {
                Some(owner) => owner.as_deref() == Some(&session.user.id.to_string()),
                None => backup.requested_by == session.user.id,
            });
    if !allowed {
        return Err(ApiError(StatusCode::NOT_FOUND, "Backup not found.".into()));
    }
    Ok(backup)
}
async fn list_repository(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let resource = manageable(&state, &session, &name).await?;
    list_for(&state, &session, Some(&resource)).await
}
async fn list_all(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<Value>, ApiError> {
    list_for(&state, &session, None).await
}
async fn list_for(
    state: &AppState,
    session: &AuthSession,
    resource: Option<&str>,
) -> Result<Json<Value>, ApiError> {
    let backups: Vec<Backup> = sqlx::query_as("SELECT b.* FROM repository_backups b LEFT JOIN lore_resources r USING(resource_id) WHERE ($1 OR r.owner_subject=$2 OR (r.resource_id IS NULL AND b.resource_id != 'server:local' AND b.requested_by=$3)) AND ($4::text IS NULL OR b.resource_id=$4 OR ($1 AND b.resource_id='server:local')) ORDER BY b.created_at DESC LIMIT 100")
        .bind(session.user.role == "admin").bind(session.user.id.to_string()).bind(session.user.id).bind(resource).fetch_all(&state.pool).await?;
    let restores: Vec<Restore> = sqlx::query_as("SELECT r.* FROM repository_restores r JOIN repository_backups b ON b.id=r.backup_id WHERE ($1 OR (r.requested_by=$2 AND b.resource_id != 'server:local')) AND ($3::text IS NULL OR b.resource_id=$3 OR ($1 AND b.resource_id='server:local')) ORDER BY r.created_at DESC LIMIT 30")
        .bind(session.user.role == "admin").bind(session.user.id).bind(resource).fetch_all(&state.pool).await?;
    let backups: Vec<Value> = backups
        .into_iter()
        .map(|backup| {
            let manifest = backup
                .manifest
                .as_deref()
                .and_then(|v| serde_json::from_str::<Manifest>(v).ok())
                .filter(|m| matches!(m.format, 2 | 3));
            let mut row = serde_json::to_value(&backup).expect("backup serialization");
            row["archive_kind"] = json!(if backup.resource_id == "server:local" {
                "server-store-tar-gzip"
            } else {
                "folder-tar-gzip"
            });
            row["has_manifest"] = json!(manifest.is_some());
            if let Some(manifest) = manifest {
                row["file_count"] = manifest.folder["file_count"].clone();
                row["total_bytes"] = manifest.folder["total_bytes"].clone();
                row["source_path"] = manifest.folder["source_path"].clone();
                row["archive_path"] = json!(
                    state
                        .backup_paths
                        .backup_root()
                        .ok()
                        .map(|root| archive_path(&root, backup.id))
                );
            }
            row.as_object_mut().unwrap().remove("manifest");
            row
        })
        .collect();
    let root = state.backup_paths.backup_root();
    Ok(Json(json!({"configured":root.is_ok(), "can_create":false,
        "can_snapshot_server":root.is_ok() && session.user.role == "admin" && state.backup_paths.server_container.is_some(),
        "server_container":state.backup_paths.server_container, "backup_directory":root.ok(),
        "backups":backups,"restores":restores,"archive_kind":"server-store-tar-gzip"})))
}
async fn detail(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<Backup>, ApiError> {
    Ok(Json(authorized(&state, &session, id).await?))
}
async fn capture(pool: &PgPool, resource: &str) -> Result<HubMetadata> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let branches = sqlx::query_scalar(
        "SELECT branches FROM ci_repository_pipeline_branches WHERE resource_id=$1",
    )
    .bind(resource)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or_else(|| vec!["main".into()]);
    let groups = sqlx::query_scalar("SELECT group_id FROM repository_account_group_access WHERE resource_id=$1 ORDER BY group_id")
        .bind(resource).fetch_all(&mut *tx).await?;
    let policies = sqlx::query_as("SELECT root_branch,link_path,auto_update FROM repository_link_policies WHERE root_resource_id=$1 ORDER BY root_branch,link_path")
        .bind(resource).fetch_all(&mut *tx).await?;
    let views = sqlx::query_as("SELECT name,mode,rules FROM sparse_workspace_views WHERE resource_id=$1 ORDER BY lower(name),id")
        .bind(resource).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(HubMetadata {
        branches,
        groups,
        policies,
        views,
    })
}
async fn set_stage(pool: &PgPool, id: Uuid, stage: &str, restore: bool) -> Result<()> {
    let table = if restore {
        "repository_restores"
    } else {
        "repository_backups"
    };
    sqlx::query(&format!(
        "UPDATE {table} SET stage=$2 WHERE id=$1 AND status='running'"
    ))
    .bind(id)
    .bind(stage)
    .execute(pool)
    .await?;
    Ok(())
}
async fn fail(pool: &PgPool, id: Uuid, error: &str, restore: bool) {
    let table = if restore {
        "repository_restores"
    } else {
        "repository_backups"
    };
    let message: String = error.chars().take(1000).collect();
    let _ = sqlx::query(&format!("UPDATE {table} SET status='failed',error=$2,finished_at=now() WHERE id=$1 AND status='running'"))
        .bind(id).bind(message).execute(pool).await;
}

async fn create(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    manageable(&state, &session, &name).await?;
    Err(bad(
        "Server data is shared across repositories. Use the administrator's local server snapshot action.",
    ))
}
async fn require_backup_admin(state: &AppState, session: &AuthSession) -> Result<(), ApiError> {
    let admin: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND role='admin')")
            .bind(session.user.id)
            .fetch_one(&state.pool)
            .await?;
    if !admin {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Server snapshots require administrator access.".into(),
        ));
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerSnapshotInput {
    acknowledge_downtime: bool,
}
async fn create_server(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Json(input): Json<ServerSnapshotInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    require_backup_admin(&state, &session).await?;
    if !input.acknowledge_downtime {
        return Err(bad(
            "Confirm that all repositories on the local server will briefly be unavailable.",
        ));
    }
    let root = state.backup_paths.backup_root().map_err(unavailable)?;
    let container = state
        .backup_paths
        .server_container
        .clone()
        .ok_or_else(|| unavailable("Configure LOREHUB_BACKUP_SERVER_CONTAINER first"))?;
    folder_command(json!({"action":"inspect-server","container":container}))
        .await
        .map_err(unavailable)?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO repository_backups(id,resource_id,repository_name,storage_backend,requested_by,status,stage) VALUES($1,'server:local','lore-server-local','local_file',$2,'running','snapshotting')")
        .bind(id).bind(session.user.id).execute(&state.pool).await.map_err(conflict)?;
    tokio::spawn(async move {
        let result=async {
            require_backup_admin(&state,&session).await.map_err(|e|anyhow::anyhow!(e.1))?;
            let path=archive_path(&root,id);
            let directory=path.parent().context("archive parent")?;
            private_directory(directory).await?;
            let resources: Vec<(String,String)> = sqlx::query_as("SELECT resource_id,name FROM lore_resources WHERE storage_backend='local_file' ORDER BY resource_id").fetch_all(&state.pool).await?;
            let mut repositories=Vec::new();
            for (resource,name) in &resources {
                repositories.push(json!({"resource_id":resource,"name":name,"hub":capture(&state.pool,resource).await?}));
            }
            let folder=folder_command(json!({"action":"snapshot-server","container":container,"root":root,"archive":path})).await?;
            set_stage(&state.pool,id,"verifying",false).await?;
            let digest=folder["archive_sha256"].as_str().context("archive digest missing")?.to_owned();
            let size=folder["size_bytes"].as_i64().context("archive size missing")?;
            let verified=folder_command(json!({"action":"verify","archive":path,"archive_sha256":digest})).await?;
            let manifest=Manifest {format:3,folder:verified,hub:HubMetadata{branches:vec![],groups:vec![],policies:vec![],views:vec![]},archive_sha256:digest.clone(),repositories};
            let serialized=serde_json::to_string(&manifest)?;
            let mut sidecar=tokio::fs::File::create(directory.join("manifest.json")).await?;
            sidecar.write_all(serialized.as_bytes()).await?; sidecar.sync_all().await?;
            sqlx::query("UPDATE repository_backups SET status='succeeded',stage='verified',manifest=$2,archive_sha256=$3,size_bytes=$4,finished_at=now() WHERE id=$1 AND status='running'")
                .bind(id).bind(serialized).bind(digest).bind(size).execute(&state.pool).await?;
            Ok::<(),anyhow::Error>(())
        }.await;
        if let Err(error) = result {
            fail(&state.pool, id, &error.to_string(), false).await;
        }
    });
    Ok((StatusCode::ACCEPTED, Json(json!({"id":id}))))
}
fn parsed(backup: &Backup) -> Result<Manifest> {
    ensure!(
        backup.status == "succeeded",
        "A verified successful backup is required"
    );
    let manifest: Manifest = serde_json::from_str(
        backup
            .manifest
            .as_deref()
            .context("Backup manifest missing")?,
    )
    .context("This backup is not a folder archive; create a new folder backup")?;
    ensure!(
        matches!(manifest.format, 2 | 3)
            && backup.archive_sha256.as_deref() == Some(&manifest.archive_sha256)
            && manifest.folder["repository_id"] == backup.resource_id
            && ((manifest.format == 2
                && manifest.folder["kind"] == "folder-tar-gzip"
                && backup.resource_id != "server:local")
                || (manifest.format == 3
                    && manifest.folder["kind"] == "server-store-tar-gzip"
                    && backup.resource_id == "server:local")),
        "Backup manifest mismatch"
    );
    Ok(manifest)
}
async fn check_archive(backup: &Backup, root: &FsPath) -> Result<Manifest> {
    let manifest = parsed(backup)?;
    let folder = folder_command(json!({"action":"verify","archive":archive_path(root,backup.id),"archive_sha256":manifest.archive_sha256})).await?;
    ensure!(
        folder == manifest.folder,
        "Backup manifest does not match the archive"
    );
    Ok(manifest)
}
async fn verify(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let mut backup = authorized(&state, &session, id).await?;
    if backup.status == "running" {
        return Err(ApiError(StatusCode::CONFLICT, "Backup is busy.".into()));
    }
    backup.status = "succeeded".into();
    parsed(&backup).map_err(|e| bad(e.to_string()))?;
    let root = state.backup_paths.backup_root().map_err(unavailable)?;
    let changed=sqlx::query("UPDATE repository_backups SET status='running',stage='verifying',error=NULL,finished_at=NULL WHERE id=$1 AND status IN ('succeeded','failed')")
        .bind(id).execute(&state.pool).await.map_err(conflict)?;
    if changed.rows_affected() == 0 {
        return Err(ApiError(StatusCode::CONFLICT, "Backup is busy.".into()));
    }
    tokio::spawn(async move {
        match check_archive(&backup, &root).await {
            Ok(_) => {
                let _=sqlx::query("UPDATE repository_backups SET status='succeeded',stage='verified',finished_at=now() WHERE id=$1 AND status='running'").bind(id).execute(&state.pool).await;
            }
            Err(error) => fail(&state.pool, id, &error.to_string(), false).await,
        }
    });
    Ok((StatusCode::ACCEPTED, Json(json!({"id":id}))))
}
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct RestoreInput {
    name: String,
}
async fn validate_restore(
    state: &AppState,
    backup: &Backup,
    input: &RestoreInput,
) -> Result<Value, ApiError> {
    let manifest = parsed(backup).map_err(|e| bad(e.to_string()))?;
    let root = state.backup_paths.backup_root().map_err(unavailable)?;
    let target = restore_path(&root, &input.name).map_err(|e| bad(e.to_string()))?;
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM repository_restores WHERE lower(target_name)=lower($1) AND status='running')").bind(&input.name).fetch_one(&state.pool).await?;
    if active {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "This folder is being restored.".into(),
        ));
    }
    Ok(
        json!({"ready":true,"target_name":input.name,"target_path":target,"file_count":manifest.folder["file_count"],"total_bytes":manifest.folder["total_bytes"],"size_bytes":backup.size_bytes,"repository_identity_preserved":true,"archive_kind":manifest.folder["kind"],"offline_restore":true}),
    )
}
async fn preview(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<RestoreInput>,
) -> Result<Json<Value>, ApiError> {
    let backup = authorized(&state, &session, id).await?;
    Ok(Json(validate_restore(&state, &backup, &input).await?))
}
async fn restore(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(backup_id): Path<Uuid>,
    Json(input): Json<RestoreInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let backup = authorized(&state, &session, backup_id).await?;
    validate_restore(&state, &backup, &input).await?;
    let root = state.backup_paths.backup_root().map_err(unavailable)?;
    let target = restore_path(&root, &input.name).map_err(|e| bad(e.to_string()))?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO repository_restores(id,backup_id,requested_by,target_name,target_backend,target_path,status) VALUES($1,$2,$3,$4,'local_file',$5,'running')")
        .bind(id).bind(backup_id).bind(session.user.id).bind(&input.name).bind(target.to_string_lossy().as_ref()).execute(&state.pool).await.map_err(conflict)?;
    tokio::spawn(async move {
        let result=async {
            let manifest=check_archive(&backup,&root).await?;
            authorized(&state,&session,backup.id).await.map_err(|e|anyhow::anyhow!(e.1))?;
            private_directory(&root.join("restored")).await?;
            set_stage(&state.pool,id,"importing",true).await?;
            let restored=folder_command(json!({"action":"restore","archive":archive_path(&root,backup.id),"archive_sha256":manifest.archive_sha256,"target":target})).await?;
            ensure!(restored["repository_id"]==backup.resource_id,"Restored folder identity mismatch");
            sqlx::query("UPDATE repository_restores SET status='succeeded',stage='verified',finished_at=now() WHERE id=$1 AND status='running'").bind(id).execute(&state.pool).await?;
            Ok::<(),anyhow::Error>(())
        }.await;
        if let Err(error) = result {
            fail(&state.pool, id, &error.to_string(), true).await;
        }
    });
    Ok((StatusCode::ACCEPTED, Json(json!({"id":id}))))
}
async fn restore_detail(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<Restore>, ApiError> {
    let restore: Restore =
        sqlx::query_as("SELECT * FROM repository_restores WHERE id=$1 AND ($2 OR requested_by=$3)")
            .bind(id)
            .bind(session.user.role == "admin")
            .bind(session.user.id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Restore not found.".into()))?;
    authorized(&state, &session, restore.backup_id).await?;
    Ok(Json(restore))
}
pub async fn recover_interrupted(pool: &PgPool) -> Result<()> {
    let paths = BackupPaths::from_env();
    if let Ok(root) = paths.backup_root()
        && root.join(".server-snapshot-recovery.json").exists()
    {
        folder_command(json!({"action":"recover-server","root":root})).await?;
    }
    for table in ["repository_backups", "repository_restores"] {
        sqlx::query(&format!("UPDATE {table} SET status='failed',error='Coordinator restarted during the operation. Inspect partial restore folders before use.',finished_at=now() WHERE status='running'")).execute(pool).await?;
    }
    Ok(())
}
