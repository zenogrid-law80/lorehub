use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use lorehub::server::{
    api,
    auth::{AuthConfig, AuthService},
    backups::BackupPaths,
    releases::RunnerReleases,
    repositories::RepositoryService,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn request(
    app: &Router,
    user: Option<Uuid>,
    method: &str,
    path: &str,
    body: Value,
    csrf: bool,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .uri(path)
        .method(method)
        .header("content-type", "application/json");
    if let Some(user) = user {
        builder = builder.header(
            "cookie",
            format!("lorehub_session={user}; lorehub_csrf=test-csrf"),
        );
    }
    if csrf {
        builder = builder.header("x-csrf-token", "test-csrf");
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn wait_job(pool: &PgPool, id: Uuid, restore: bool) -> (String, Option<String>) {
    let table = if restore {
        "repository_restores"
    } else {
        "repository_backups"
    };
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let row: (String, Option<String>) =
                sqlx::query_as(&format!("SELECT status,error FROM {table} WHERE id=$1"))
                    .bind(id)
                    .fetch_one(pool)
                    .await
                    .unwrap();
            if row.0 != "running" {
                return row;
            }
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("folder job timed out")
}

#[cfg(unix)]
#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn folder_backups_restore_without_lore_cli_and_enforce_access(pool: PgPool) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let sources = root.join("repositories");
    let source = sources.join("source");
    let archives = root.join("backups");
    std::fs::create_dir_all(source.join(".lore")).unwrap();
    std::fs::create_dir(&archives).unwrap();
    let repository_id = Uuid::new_v4();
    let resource = format!("urc-{}", repository_id.simple());
    std::fs::write(source.join(".lore/id"), repository_id.as_bytes()).unwrap();
    std::fs::write(source.join(".lore/lock"), "").unwrap();
    std::fs::write(source.join("uncommitted.txt"), "local changes").unwrap();
    let app = api::router_with_backup_paths(
        pool.clone(),
        AuthService::new(
            pool.clone(),
            AuthConfig::new("test".into(), "test".into(), "http://127.0.0.1:8080").unwrap(),
        )
        .unwrap(),
        RepositoryService::new(
            "/must-not-run-lore-for-folder-backups",
            "lores://127.0.0.1:41337",
            "lores://127.0.0.1:41337",
        )
        .unwrap(),
        None,
        RunnerReleases::default(),
        BackupPaths {
            archive_root: Some(archives.clone()),
            server_container: None,
            repository_root: Some(sources),
        },
    );
    let owner = Uuid::new_v4();
    let outsider = Uuid::new_v4();
    let admin = Uuid::new_v4();
    for (id, role) in [(owner, "user"), (outsider, "user"), (admin, "admin")] {
        sqlx::query("INSERT INTO users(id,google_sub,email,role) VALUES($1,$2,$3,$4)")
            .bind(id)
            .bind(id.to_string())
            .bind(format!("{id}@zenogrid.co.kr"))
            .bind(role)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,csrf_hash,expires_at) VALUES($1,$2,$3,now()+interval '1 hour')")
            .bind(Sha256::digest(id.to_string()).to_vec()).bind(id).bind(Sha256::digest(b"test-csrf").to_vec()).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE users SET role='user' WHERE id IN ($1,$2)")
        .bind(owner)
        .bind(outsider)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES($1,'source',$2)",
    )
    .bind(&resource)
    .bind(owner.to_string())
    .execute(&pool)
    .await
    .unwrap();
    let endpoint = "/api/v1/repositories/source/backups";
    assert_eq!(
        request(&app, None, "GET", endpoint, json!({}), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&app, Some(outsider), "POST", endpoint, json!({}), true)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, Some(owner), "POST", endpoint, json!({}), false)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let list = request(&app, Some(owner), "GET", endpoint, json!({}), false).await;
    assert_eq!(list.0, StatusCode::OK);
    assert_eq!(list.1["can_create"], false);
    assert_eq!(
        request(&app, Some(owner), "POST", endpoint, json!({}), true)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            "/api/v1/server-backups/local",
            json!({"acknowledge_downtime":true}),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(admin),
            "POST",
            "/api/v1/server-backups/local",
            json!({"acknowledge_downtime":false}),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    // Legacy folder archives remain readable, but new backups use server snapshots.
    let id = Uuid::new_v4();
    let directory = archives.join(id.to_string());
    std::fs::create_dir(&directory).unwrap();
    let archive = directory.join("repository.tar.gz");
    let mut child = std::process::Command::new("python3")
        .args(["-I", "scripts/repository-folder-archive.py"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            json!({"action":"create","source":source,"archive":archive,"resource_id":resource})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let mut folder: Value = serde_json::from_slice(&output.stdout).unwrap();
    let digest = folder
        .as_object_mut()
        .unwrap()
        .remove("archive_sha256")
        .unwrap();
    let size = folder
        .as_object_mut()
        .unwrap()
        .remove("size_bytes")
        .unwrap();
    let manifest = json!({"format":2,"folder":folder,"hub":{"branches":[],"groups":[],"policies":[],"views":[]},"archive_sha256":digest});
    sqlx::query("INSERT INTO repository_backups(id,resource_id,repository_name,storage_backend,requested_by,status,manifest,archive_sha256,size_bytes) VALUES($1,$2,'source','local_file',$3,'succeeded',$4,$5,$6)")
        .bind(id).bind(&resource).bind(owner).bind(manifest.to_string()).bind(digest.as_str().unwrap()).bind(size.as_i64().unwrap()).execute(&pool).await.unwrap();
    let list = request(&app, Some(owner), "GET", endpoint, json!({}), false)
        .await
        .1;
    assert_eq!(list["backups"][0]["file_count"], 3);
    assert!(list["backups"][0].get("manifest").is_none());
    let detail = format!("/api/v1/repository-backups/{id}");
    assert_eq!(
        request(&app, Some(outsider), "GET", &detail, json!({}), false)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let preview = format!("{detail}/restore-preview");
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &preview,
            json!({"name":"../escape"}),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &preview,
            json!({"name":"restored","storage_backend":"local_file"}),
            true
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let input = json!({"name":"restored"});
    let previewed = request(&app, Some(owner), "POST", &preview, input.clone(), true).await;
    assert_eq!(previewed.0, StatusCode::OK, "{}", previewed.1);
    assert_eq!(
        previewed.1["target_path"],
        archives
            .join("restored/restored")
            .to_string_lossy()
            .as_ref()
    );
    let restored = request(
        &app,
        Some(owner),
        "POST",
        &format!("{detail}/restore"),
        input.clone(),
        true,
    )
    .await;
    assert_eq!(restored.0, StatusCode::ACCEPTED, "{}", restored.1);
    let restore_id: Uuid = restored.1["id"].as_str().unwrap().parse().unwrap();
    let outcome = wait_job(&pool, restore_id, true).await;
    assert_eq!(outcome.0, "succeeded", "{outcome:?}");
    assert_eq!(
        std::fs::read_to_string(archives.join("restored/restored/uncommitted.txt")).unwrap(),
        "local changes"
    );
    assert_eq!(
        std::fs::read(archives.join("restored/restored/.lore/id")).unwrap(),
        repository_id.as_bytes()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM lore_resources")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "folder restore must not create a remote repository"
    );
    assert_eq!(
        request(&app, Some(owner), "POST", &preview, input, true)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    // Corruption is recorded as a failed verification, with no restore allowed.
    let archive = archives.join(id.to_string()).join("repository.tar.gz");
    let original = std::fs::read(&archive).unwrap();
    std::fs::write(&archive, b"corrupt").unwrap();
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &format!("{detail}/verify"),
            json!({}),
            true
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
    assert_eq!(wait_job(&pool, id, false).await.0, "failed");
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &preview,
            json!({"name":"next"}),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    std::fs::write(&archive, original).unwrap();
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &format!("{detail}/verify"),
            json!({}),
            true
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
    assert_eq!(wait_job(&pool, id, false).await.0, "succeeded");
    // Current owner controls the archive; requester regains recovery access after deletion.
    sqlx::query("UPDATE lore_resources SET owner_subject=$1 WHERE resource_id=$2")
        .bind(outsider.to_string())
        .bind(&resource)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, Some(owner), "GET", &detail, json!({}), false)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, Some(outsider), "GET", &detail, json!({}), false)
            .await
            .0,
        StatusCode::OK
    );
    sqlx::query("DELETE FROM lore_resources WHERE resource_id=$1")
        .bind(&resource)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, Some(owner), "GET", &detail, json!({}), false)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        request(&app, Some(admin), "GET", &detail, json!({}), false)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        request(&app, Some(outsider), "GET", &detail, json!({}), false)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let global = request(
        &app,
        Some(owner),
        "GET",
        "/api/v1/repository-backups",
        json!({}),
        false,
    )
    .await;
    assert_eq!(global.1["backups"].as_array().unwrap().len(), 1);
    let server_id = Uuid::new_v4();
    sqlx::query("INSERT INTO repository_backups(id,resource_id,repository_name,storage_backend,requested_by,status) VALUES($1,'server:local','lore-server-local','local_file',$2,'failed')").bind(server_id).bind(owner).execute(&pool).await.unwrap();
    let server_detail = format!("/api/v1/repository-backups/{server_id}");
    assert_eq!(
        request(&app, Some(owner), "GET", &server_detail, json!({}), false)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, Some(admin), "GET", &server_detail, json!({}), false)
            .await
            .0,
        StatusCode::OK
    );
    let owner_list = request(
        &app,
        Some(owner),
        "GET",
        "/api/v1/repository-backups",
        json!({}),
        false,
    )
    .await
    .1;
    assert_eq!(owner_list["backups"].as_array().unwrap().len(), 1);
}
