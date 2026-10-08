#![cfg(unix)]

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use lorehub::{
    ci::{config::PipelineFile, db, settings},
    runner::{CoordinatorClient, Worker},
    server::{
        api,
        auth::{AuthConfig, AuthService},
        repositories::RepositoryService,
        tokens::TokenIssuer,
        triggers,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::os::unix::fs::PermissionsExt;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use uuid::Uuid;

const RESOURCE: &str = "urc-11111111111111111111111111111111";
const URL: &str = "lores://fixture:41337/db-test";

fn source(command: &str) -> String {
    format!(
        "[[pipelines]]\nname='build'\nrunner_os='{}'\nchanges=['src/**']\nworking_directory='.'\nstages=['test']\n[[pipelines.jobs]]\nname='check'\nstage='test'\nscript=['{command}']\ntimeout_seconds=5\n",
        std::env::consts::OS
    )
}

struct Fixture {
    root: tempfile::TempDir,
    app: Router,
    owner: Uuid,
    outsider: Uuid,
    issuer: TokenIssuer,
}

async fn fixture(pool: &PgPool) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("lore");
    std::fs::write(&binary, include_str!("fixtures/ci-database-lore.py")).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let issuer = TokenIssuer::from_files(
        "tests/fixtures/test-private.pem",
        "tests/fixtures/test-jwks.json",
        "http://127.0.0.1:8080",
        "zenogrid.co.kr",
    )
    .unwrap();
    let app = api::router(
        pool.clone(),
        AuthService::new(
            pool.clone(),
            AuthConfig::new("test".into(), "test".into(), "http://127.0.0.1:8080").unwrap(),
        )
        .unwrap(),
        RepositoryService::new(binary, "lores://fixture:41337", "lores://fixture:41337").unwrap(),
        Some(issuer.clone()),
    );
    let owner = Uuid::new_v4();
    let outsider = Uuid::new_v4();
    for user in [owner, outsider] {
        sqlx::query("INSERT INTO users(id,google_sub,email) VALUES($1,$2,$3)")
            .bind(user)
            .bind(user.to_string())
            .bind(format!("{user}@zenogrid.co.kr"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,csrf_hash,expires_at) VALUES($1,$2,$3,now()+interval '1 hour')").bind(Sha256::digest(user.to_string().as_bytes()).to_vec()).bind(user).bind(Sha256::digest(b"csrf").to_vec()).execute(pool).await.unwrap();
    }
    sqlx::query(
        "INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES($1,'db-test',$2)",
    )
    .bind(RESOURCE)
    .bind(owner.to_string())
    .execute(pool)
    .await
    .unwrap();
    Fixture {
        root,
        app,
        owner,
        outsider,
        issuer,
    }
}

async fn request(
    f: &Fixture,
    user: Uuid,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .header(
                    "cookie",
                    format!("lorehub_session={user}; lorehub_csrf=csrf"),
                )
                .header("x-csrf-token", "csrf")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)))
    };
    (status, body)
}

async fn save(
    pool: &PgPool,
    actor: Uuid,
    branch: &str,
    expected: i64,
    content: &str,
) -> settings::Revision {
    let mut tx = pool.begin().await.unwrap();
    let version = settings::save(
        &mut tx,
        settings::Save {
            resource: RESOURCE,
            branch,
            expected_lock_version: expected,
            content,
            source_revision: Some(&"a".repeat(64)),
            actor,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    version
}

#[test]
fn structured_definitions_share_toml_validation_and_round_trip() {
    for source in [
        source("echo first"),
        include_str!("../examples/.lore-ci.toml").to_owned(),
    ] {
        let file = PipelineFile::parse(&source).unwrap();
        let json = serde_json::to_value(&file).unwrap();
        let structured = serde_json::from_value::<PipelineFile>(json.clone())
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(serde_json::to_value(&structured).unwrap(), json);
        assert_eq!(
            serde_json::to_value(
                PipelineFile::parse(&toml::to_string(&structured).unwrap()).unwrap()
            )
            .unwrap(),
            json
        );
    }
    let mut invalid =
        serde_json::to_value(PipelineFile::parse(&source("echo ok")).unwrap()).unwrap();
    invalid["pipelines"][0]["jobs"][0]["timeout_seconds"] = json!(0);
    assert!(
        serde_json::from_value::<PipelineFile>(invalid)
            .unwrap()
            .validate()
            .is_err()
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn api_versions_conflicts_permissions_restore_export_and_rerun(pool: PgPool) {
    let f = fixture(&pool).await;
    let path = "/api/v1/repositories/db-test/ci-config";
    let (_, empty) = request(
        &f,
        f.owner,
        "GET",
        &format!("{path}?branch=main"),
        Value::Null,
    )
    .await;
    assert!(empty["content"].is_null(), "{empty}");
    let first = source("echo first");
    let payload = json!({"branch":"main","expected_revision":"a".repeat(64),"expected_lock_version":0,"content":first});
    assert_eq!(
        request(&f, f.outsider, "POST", path, payload.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, v1) = request(&f, f.owner, "POST", path, payload.clone()).await;
    assert_eq!(status, StatusCode::OK, "{v1}");
    assert_eq!(v1["source_mode"], "db");
    assert_eq!(v1["config_version"], 1);
    assert_eq!(v1["revision"], "a".repeat(64));
    let (_, read) = request(
        &f,
        f.owner,
        "GET",
        &format!("{path}?branch=main"),
        Value::Null,
    )
    .await;
    assert_eq!(read["content"], first);
    let (_, choices) = request(
        &f,
        f.owner,
        "GET",
        &format!(
            "/api/v1/repositories/db-test/pipelines?branch=main&revision={}",
            "a".repeat(64)
        ),
        Value::Null,
    )
    .await;
    assert_eq!(choices[0]["config_revision_id"], v1["config_revision_id"]);
    assert_eq!(
        request(&f, f.owner, "POST", path, payload.clone()).await.0,
        StatusCode::CONFLICT
    );
    let second = source("echo second");
    let mut second_payload = payload.clone();
    second_payload["expected_lock_version"] = json!(1);
    second_payload["content"] = json!(second);
    let (status, v2) = request(&f, f.owner, "POST", path, second_payload).await;
    assert_eq!(status, StatusCode::OK, "{v2}");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM pipelines")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let (status, run) = request(&f, f.owner, "POST", "/api/v1/pipelines", json!({"repository_url":URL,"branch":"main","revision":"a".repeat(64),"pipeline_name":"build","config_revision_id":v1["config_revision_id"]})).await;
    assert_eq!(status, StatusCode::CREATED, "{run}");
    let id = run["id"].as_str().unwrap();
    let (_, detail) = request(
        &f,
        f.owner,
        "GET",
        &format!("/api/v1/pipelines/{id}"),
        Value::Null,
    )
    .await;
    assert_eq!(
        detail["execution_spec"]["config"]["jobs"][0]["script"][0],
        "echo first"
    );
    std::fs::write(f.root.path().join("head"), "b".repeat(64)).unwrap();
    let (status, rerun) = request(
        &f,
        f.owner,
        "POST",
        &format!("/api/v1/pipelines/{id}/rerun"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{rerun}");
    assert_eq!(rerun["revision"], run["revision"]);
    assert_eq!(rerun["config_revision_id"], v1["config_revision_id"]);
    assert_ne!(rerun["run_group_id"], run["run_group_id"]);
    assert_eq!(
        request(
            &f,
            f.outsider,
            "POST",
            &format!("/api/v1/pipelines/{id}/rerun"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, restored) = request(
        &f,
        f.owner,
        "POST",
        &format!("{path}/restore"),
        json!({"branch":"main","expected_lock_version":2,"revision_id":v1["config_revision_id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["lock_version"], 3);
    assert_eq!(restored["config_version"], 1);
    let (_, history) = request(
        &f,
        f.owner,
        "GET",
        &format!("{path}/history?branch=main"),
        Value::Null,
    )
    .await;
    assert_eq!(history["versions"].as_array().unwrap().len(), 2);
    assert_eq!(history["events"].as_array().unwrap().len(), 3);
    assert_eq!(
        request(
            &f,
            f.outsider,
            "GET",
            &format!("{path}/history?branch=main"),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let mode = json!({"branch":"main","expected_lock_version":3});
    assert_eq!(
        request(
            &f,
            f.owner,
            "POST",
            &format!("{path}/file-mode"),
            mode.clone()
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    std::fs::write(f.root.path().join("source.toml"), second).unwrap();
    assert_eq!(
        request(
            &f,
            f.owner,
            "POST",
            &format!("{path}/file-mode"),
            mode.clone()
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    std::fs::write(f.root.path().join("source.toml"), first).unwrap();
    assert_eq!(
        request(&f, f.owner, "POST", &format!("{path}/file-mode"), mode)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, file) = request(
        &f,
        f.owner,
        "GET",
        &format!("{path}?branch=main"),
        Value::Null,
    )
    .await;
    assert_eq!(file["source_mode"], "file");
    assert_eq!(file["lock_version"], 4);
    let commands = std::fs::read_to_string(f.root.path().join("commands")).unwrap();
    assert!(
        !commands
            .lines()
            .any(|line| line.starts_with("commit") || line.starts_with("push"))
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn concurrent_saves_are_atomic_and_history_survives_repository_deletion(pool: PgPool) {
    let f = fixture(&pool).await;
    let version = save(&pool, f.owner, "main", 0, &source("echo old")).await;
    let save_next = |command: &'static str| {
        let pool = pool.clone();
        async move {
            let mut tx = pool.begin().await.unwrap();
            let content = source(command);
            let result = settings::save(
                &mut tx,
                settings::Save {
                    resource: RESOURCE,
                    branch: "main",
                    expected_lock_version: 1,
                    content: &content,
                    source_revision: None,
                    actor: f.owner,
                },
            )
            .await;
            if result.is_ok() {
                tx.commit().await.unwrap();
            } else {
                tx.rollback().await.unwrap();
            }
            result
        }
    };
    let (a, b) = tokio::join!(save_next("echo a"), save_next("echo b"));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!((if let Err(e) = a { e } else { b.unwrap_err() }).is::<settings::Conflict>());
    let copied = save(&pool, f.owner, "release", 0, &version.source_toml).await;
    let mut tx = pool.begin().await.unwrap();
    assert!(
        settings::revision(&mut tx, version.config_id, copied.id)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    assert!(
        sqlx::query("UPDATE ci_config_revisions SET source_toml='changed' WHERE id=$1")
            .bind(version.id)
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM lore_resources WHERE resource_id=$1")
        .bind(RESOURCE)
        .execute(&pool)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ci_config_revisions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn push_snapshots_are_idempotent_and_dependencies_are_group_scoped(pool: PgPool) {
    let f = fixture(&pool).await;
    let base = source("echo upstream").replace("name='build'", "name='prepare'");
    let dependent =
        source("echo dependent").replace("name='build'", "name='build'\nneeds=['prepare']");
    let file = format!("{base}\n{dependent}");
    let first = save(&pool, f.owner, "main", 0, &file).await;
    let obsolete = PipelineFile::parse(&source("echo must-not-run")).unwrap();
    let mut tx = pool.begin().await.unwrap();
    let count = triggers::enqueue(
        &mut tx,
        RESOURCE,
        URL,
        "main",
        &"0".repeat(64),
        &"a".repeat(64),
        f.owner,
        &obsolete,
        &["src/main.rs".into()],
    )
    .await
    .unwrap();
    assert_eq!(count, 2);
    tx.commit().await.unwrap();
    save(
        &pool,
        f.owner,
        "main",
        1,
        &file.replace("upstream", "new-upstream"),
    )
    .await;
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        triggers::enqueue(
            &mut tx,
            RESOURCE,
            URL,
            "main",
            &"0".repeat(64),
            &"a".repeat(64),
            f.owner,
            &obsolete,
            &["src/main.rs".into()]
        )
        .await
        .unwrap(),
        0
    );
    let group = settings::group(
        &mut tx,
        RESOURCE,
        "main",
        &"a".repeat(64),
        first.id,
        "manual",
    )
    .await
    .unwrap()
    .unwrap();
    for name in ["prepare", "build"] {
        settings::enqueue(
            &mut tx,
            settings::Run {
                group,
                resource: RESOURCE,
                url: URL,
                branch: "main",
                code_revision: &"a".repeat(64),
                previous_revision: None,
                actor: f.owner,
                revision: &first,
                pipeline_name: Some(name),
                changes: &[],
            },
        )
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    sqlx::query("UPDATE pipelines SET status=CASE WHEN run_group_id=$1 THEN 'succeeded' ELSE 'failed' END WHERE pipeline_name='prepare'").bind(group).execute(&pool).await.unwrap();
    let worker = Uuid::new_v4();
    db::register_runner(
        &pool,
        worker,
        "test",
        std::env::consts::OS,
        std::env::consts::ARCH,
        "0.2.21",
        false,
    )
    .await
    .unwrap();
    assert!(db::claim(&pool, worker).await.unwrap().is_none());
    let claimed = db::claim_with_spec_support(&pool, worker, Some(Uuid::new_v4()), true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.run_group_id, Some(group));
    assert_eq!(claimed.pipeline_name.as_deref(), Some("build"));
    let failed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pipelines WHERE pipeline_name='build' AND status='failed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failed, 1);
    assert!(
        sqlx::query("UPDATE pipelines SET execution_spec='{}' WHERE id=$1")
            .bind(claimed.id)
            .execute(&pool)
            .await
            .is_err()
    );
    let submitted = lorehub::ci::config::PipelineConfig::parse(
        "stages=['other']\n[[jobs]]\nname='injected'\nstage='other'\nscript=['echo injected']",
    )
    .unwrap();
    let jobs = db::create_jobs(&pool, claimed.id, worker, &submitted)
        .await
        .unwrap();
    assert_eq!(jobs[0].name, "check");
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn worker_executes_saved_commands_without_a_toml_file(pool: PgPool) {
    let f = fixture(&pool).await;
    let version = save(&pool, f.owner, "main", 0, &source("echo SNAPSHOT_ORIGINAL")).await;
    let mut tx = pool.begin().await.unwrap();
    let group = settings::group(
        &mut tx,
        RESOURCE,
        "main",
        &"a".repeat(64),
        version.id,
        "manual",
    )
    .await
    .unwrap()
    .unwrap();
    let pipeline = settings::enqueue(
        &mut tx,
        settings::Run {
            group,
            resource: RESOURCE,
            url: URL,
            branch: "main",
            code_revision: &"a".repeat(64),
            previous_revision: None,
            actor: f.owner,
            revision: &version,
            pipeline_name: Some("build"),
            changes: &[],
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    save(&pool, f.owner, "main", 1, &source("exit 99")).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = f.app.clone();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let id = Uuid::new_v4();
    let worker = Worker {
        coordinator: CoordinatorClient::new(&format!("http://{address}"), id, f.issuer).unwrap(),
        id,
        work_dir: f.root.path().join("work"),
        lore_bin: f.root.path().join("lore").to_string_lossy().into_owned(),
        token_issuer: None,
    };
    worker.run(CancellationToken::new(), true).await.unwrap();
    server.abort();
    let status: String = sqlx::query_scalar("SELECT status FROM pipelines WHERE id=$1")
        .bind(pipeline.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "succeeded");
    let logs: String =
        sqlx::query_scalar("SELECT string_agg(content,'') FROM logs WHERE pipeline_id=$1")
            .bind(pipeline.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(logs.contains("SNAPSHOT_ORIGINAL"), "{logs}");
    let commands = std::fs::read_to_string(f.root.path().join("commands")).unwrap();
    assert!(commands.contains(&format!("--revision {}", "a".repeat(64))));
    assert!(!commands.contains("--branch"));
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requires PostgreSQL"]
async fn graph_layouts_persist_merge_and_preserve_ci_definitions(pool: PgPool) {
    let f = fixture(&pool).await;
    let endpoint = "/api/v1/repositories/db-test/ci-config/layout";
    let query = format!(
        "{endpoint}?{}",
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair("branch", "main")
            .append_pair("graph", "[\"overview\"]")
            .finish()
    );
    let (status, empty) = request(&f, f.owner, "GET", &query, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["positions"], json!({}));
    let change = |key: &str, x: i32| json!({"branch":"main", "graph":"[\"overview\"]", "positions":{key:{"x":x,"y":140}}});
    let (a, b) = tokio::join!(
        request(&f, f.owner, "POST", endpoint, change("pipeline-a", 120)),
        request(&f, f.owner, "POST", endpoint, change("pipeline-b", 400)),
    );
    assert_eq!(a.0, StatusCode::OK);
    assert_eq!(b.0, StatusCode::OK);
    let (_, saved) = request(&f, f.owner, "GET", &query, Value::Null).await;
    assert_eq!(saved["positions"]["pipeline-a"]["x"], 120.0);
    assert_eq!(saved["positions"]["pipeline-b"]["x"], 400.0);
    for (branch, graph) in [
        ("release", "[\"overview\"]"),
        ("main", "[\"detail\",\"build\"]"),
    ] {
        let path = format!(
            "{endpoint}?{}",
            url::form_urlencoded::Serializer::new(String::new())
                .append_pair("branch", branch)
                .append_pair("graph", graph)
                .finish()
        );
        assert_eq!(
            request(&f, f.owner, "GET", &path, Value::Null).await.1["positions"],
            json!({})
        );
    }
    for method in ["GET", "POST"] {
        let result = request(
            &f,
            f.outsider,
            method,
            if method == "GET" { &query } else { endpoint },
            change("pipeline-a", 0),
        )
        .await;
        assert_eq!(result.0, StatusCode::FORBIDDEN);
    }
    for invalid in [
        change("bad", -1),
        change("bad", 50001),
        json!({"branch":"main","graph":"unknown","positions":{}}),
        json!({"branch":"main","graph":"[\"overview\"]","reset":true,"positions":{"a":{"x":1,"y":1}}}),
    ] {
        assert_eq!(
            request(&f, f.owner, "POST", endpoint, invalid).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let without_csrf = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(endpoint)
                .header("content-type", "application/json")
                .header("cookie", format!("lorehub_session={}", f.owner))
                .body(Body::from(change("no-csrf", 0).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(without_csrf.status(), StatusCode::FORBIDDEN);
    let (_, removed) = request(
        &f,
        f.owner,
        "POST",
        endpoint,
        json!({"branch":"main","graph":"[\"overview\"]","positions":{"pipeline-a":null}}),
    )
    .await;
    assert!(removed["positions"].get("pipeline-a").is_none());
    assert_eq!(removed["positions"]["pipeline-b"]["x"], 400.0);
    assert_eq!(
        request(
            &f,
            f.owner,
            "POST",
            endpoint,
            json!({"branch":"main","graph":"[\"overview\"]","reset":true})
        )
        .await
        .1["positions"],
        json!({})
    );
    for table in [
        "ci_configs",
        "ci_config_revisions",
        "ci_config_events",
        "pipelines",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
    sqlx::query("DELETE FROM lore_resources WHERE resource_id=$1")
        .bind(RESOURCE)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ci_graph_layouts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
