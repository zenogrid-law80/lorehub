#![cfg(unix)]

use std::{os::unix::fs::PermissionsExt, time::Duration};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use lorehub::{
    ci::{config::SubmitPipeline, db},
    runner::{
        Worker,
        executor::{self, Execution},
    },
    server::{
        api,
        auth::{AuthConfig, AuthService},
        repositories::RepositoryService,
    },
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use uuid::Uuid;

fn input() -> SubmitPipeline {
    SubmitPipeline {
        repository_url: "lores://127.0.0.1:41337/example".into(),
        revision: "a".repeat(64),
        branch: None,
        pipeline_name: None,
    }
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn cancellation_reaches_the_running_worker(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let lore_bin = fake_lore(
        root.path(),
        "stages = ['test']\n[[jobs]]\nname = 'slow'\nstage = 'test'\nscript = ['echo started', 'sleep 30']",
    );
    let worker = Worker {
        pool: pool.clone(),
        id: Uuid::new_v4(),
        work_dir: root.path().join("work"),
        lore_bin,
        token_issuer: None,
    };
    let pipeline = db::submit(&pool, &input()).await.unwrap();
    let task = tokio::spawn(async move { worker.run(CancellationToken::new(), true).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let started: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE pipeline_id = $1 AND status = 'running')",
            )
            .bind(pipeline.id)
            .fetch_one(&pool)
            .await
            .unwrap();
            if started {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    db::cancel(&pool, pipeline.id).await.unwrap();
    tokio::time::timeout(Duration::from_secs(8), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(status(&pool, pipeline.id).await, "canceled");
    let job: String = sqlx::query_scalar("SELECT status FROM jobs WHERE pipeline_id = $1")
        .bind(pipeline.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(job, "canceled");
    assert_eq!(
        std::fs::read_dir(root.path().join("work")).unwrap().count(),
        0
    );
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn reaper_preserves_cancellation_on_jobs(pool: PgPool) {
    let pipeline = db::submit(&pool, &input()).await.unwrap();
    let worker = Uuid::new_v4();
    db::claim(&pool, worker).await.unwrap();
    let config = lorehub::ci::config::PipelineConfig::parse(
        "stages = ['test']\n[[jobs]]\nname = 'test'\nstage = 'test'\nscript = ['true']",
    )
    .unwrap();
    let jobs = db::create_jobs(&pool, pipeline.id, worker, &config)
        .await
        .unwrap();
    db::job_status(&pool, jobs[0].id, worker, "running", None)
        .await
        .unwrap();
    db::cancel(&pool, pipeline.id).await.unwrap();
    sqlx::query("UPDATE pipelines SET lease_until = now() - interval '1 second' WHERE id = $1")
        .bind(pipeline.id)
        .execute(&pool)
        .await
        .unwrap();
    db::reap(&pool).await.unwrap();
    assert_eq!(status(&pool, pipeline.id).await, "canceled");
    let job: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
        .bind(jobs[0].id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(job, "canceled");
    assert!(
        db::job_status(&pool, jobs[0].id, worker, "succeeded", Some(0))
            .await
            .is_err()
    );
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn selected_manual_pipeline_preserves_execution_snapshot(pool: PgPool) {
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id,google_sub,email) VALUES ($1,'selected-owner','selected@example.test')")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut request = input();
    request.pipeline_name = Some("build".into());
    request.branch = Some("main".into());
    let selected = db::SelectedPipeline {
        pipeline_name: "build".into(),
        category: "server".into(),
        runner_os: "linux".into(),
        trigger_patterns: vec!["src/**".into()],
        working_directory: "src".into(),
        graph_definition: r#"{"stages":[{"name":"test","jobs":["check"]}]}"#.into(),
        sparse_view_name: Some("Build".into()),
        sparse_view_rules: Some("/src".into()),
    };
    let mut tx = pool.begin().await.unwrap();
    let pipeline = db::submit_selected_for_user(&mut tx, &request, user_id, &selected)
        .await
        .unwrap();
    let rerun = db::submit_selected_for_user(&mut tx, &request, user_id, &selected)
        .await
        .unwrap();
    assert_ne!(pipeline.id, rerun.id);
    tx.commit().await.unwrap();
    // Manual attempts do not consume the once-per-push key. Push retries still
    // deduplicate, and a previous push does not prevent another manual attempt.
    for expected in [1, 0] {
        let result = sqlx::query("INSERT INTO pipelines(id,repository_url,revision,branch,pipeline_name,previous_revision) VALUES($1,$2,$3,'main','build',$4) ON CONFLICT DO NOTHING")
            .bind(Uuid::new_v4()).bind(&request.repository_url).bind(&request.revision).bind("0".repeat(64))
            .execute(&pool).await.unwrap();
        assert_eq!(result.rows_affected(), expected);
    }
    let mut tx = pool.begin().await.unwrap();
    db::submit_selected_for_user(&mut tx, &request, user_id, &selected)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(pipeline.pipeline_name.as_deref(), Some("build"));
    assert_eq!(pipeline.category.as_deref(), Some("server"));
    assert_eq!(pipeline.runner_os.as_deref(), Some("linux"));
    assert_eq!(pipeline.working_directory.as_deref(), Some("src"));
    assert_eq!(pipeline.sparse_view_name.as_deref(), Some("Build"));
    let saved: (Vec<String>, String, String) = sqlx::query_as(
        "SELECT trigger_patterns,graph_definition,sparse_view_rules FROM pipelines WHERE id=$1",
    )
    .bind(pipeline.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(saved.0, ["src/**"]);
    assert_eq!(saved.1, selected.graph_definition);
    assert_eq!(saved.2, selected.sparse_view_rules.unwrap());
}

async fn status(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM pipelines WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn atomic_claim_and_expired_worker_fencing(pool: PgPool) {
    let submitted = db::submit(&pool, &input()).await.unwrap();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let (first, second) = tokio::join!(db::claim(&pool, a), db::claim(&pool, b));
    let claims = [first.unwrap(), second.unwrap()];
    assert_eq!(claims.iter().filter(|c| c.is_some()).count(), 1);
    let winner = claims
        .into_iter()
        .flatten()
        .next()
        .unwrap()
        .worker_id
        .unwrap();
    assert!(db::heartbeat(&pool, submitted.id, winner).await.unwrap());
    assert!(
        !db::heartbeat(&pool, submitted.id, Uuid::new_v4())
            .await
            .unwrap()
    );
    sqlx::query("UPDATE pipelines SET lease_until = now() - interval '1 second' WHERE id = $1")
        .bind(submitted.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(!db::heartbeat(&pool, submitted.id, winner).await.unwrap());
    db::finish(&pool, submitted.id, winner, "succeeded", None)
        .await
        .unwrap();
    assert_eq!(status(&pool, submitted.id).await, "running");
    db::reap(&pool).await.unwrap();
    assert_eq!(status(&pool, submitted.id).await, "failed");
    assert!(db::claim(&pool, a).await.unwrap().is_none());
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn cancellation_wins_completion_race(pool: PgPool) {
    let queued = db::submit(&pool, &input()).await.unwrap();
    db::cancel(&pool, queued.id).await.unwrap();
    assert_eq!(status(&pool, queued.id).await, "canceled");
    let pipeline = db::submit(&pool, &input()).await.unwrap();
    let worker = Uuid::new_v4();
    db::claim(&pool, worker).await.unwrap().unwrap();
    db::cancel(&pool, pipeline.id).await.unwrap();
    assert!(!db::heartbeat(&pool, pipeline.id, worker).await.unwrap());
    db::finish(&pool, pipeline.id, worker, "succeeded", None)
        .await
        .unwrap();
    assert_eq!(status(&pool, pipeline.id).await, "canceled");
    db::finish(&pool, pipeline.id, worker, "failed", None)
        .await
        .unwrap();
    assert_eq!(status(&pool, pipeline.id).await, "canceled");
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn authenticated_api_and_log_cursor(pool: PgPool) {
    let auth = AuthService::new(
        pool.clone(),
        AuthConfig::new(
            "test-client".into(),
            "test-secret".into(),
            "http://127.0.0.1:8080",
        )
        .unwrap(),
    )
    .unwrap();
    let repositories = RepositoryService::new(
        "/usr/bin/false",
        "lores://127.0.0.1:41337",
        "lores://127.0.0.1:41337",
    )
    .unwrap();
    let app = api::router(pool.clone(), auth, repositories, None);
    let home = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(home.status(), StatusCode::OK);
    assert_eq!(
        home.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
    assert!(home.headers().contains_key("content-security-policy"));
    let home_body = home.into_body().collect().await.unwrap().to_bytes();
    let home_body = std::str::from_utf8(&home_body).unwrap();
    assert!(home_body.contains("LoreHub") && home_body.contains("Google 계정으로 계속"));
    assert!(home_body.contains("execution-graph") && home_body.contains("Execution graph"));

    let stylesheet = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/app.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stylesheet.status(), StatusCode::OK);
    assert_eq!(
        stylesheet.headers().get("content-type").unwrap(),
        "text/css; charset=utf-8"
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/pipelines")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/google/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::SEE_OTHER);
    let location = login.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
    assert!(location.contains("hd=zenogrid.co.kr"));
    assert!(
        login
            .headers()
            .get_all("set-cookie")
            .iter()
            .any(|cookie| cookie.to_str().unwrap().starts_with("lorehub_oauth_state="))
    );

    let user_id = Uuid::new_v4();
    let session = "integration-session";
    let csrf = "integration-csrf";
    sqlx::query("INSERT INTO users (id,google_sub,email,name) VALUES ($1,$2,$3,$4)")
        .bind(user_id)
        .bind("google-sub")
        .bind("developer@zenogrid.co.kr")
        .bind("Developer")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions (token_hash,user_id,csrf_hash,expires_at) VALUES ($1,$2,$3,now()+interval '1 hour')")
        .bind(Sha256::digest(session.as_bytes()).to_vec()).bind(user_id)
        .bind(Sha256::digest(csrf.as_bytes()).to_vec()).execute(&pool).await.unwrap();
    let cookies = format!("lorehub_session={session}; lorehub_csrf={csrf}");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/me")
                .header("cookie", &cookies)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let current_user: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(current_user["email"], "developer@zenogrid.co.kr");

    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/pipelines")
                .method("POST")
                .header("cookie", &cookies)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&input()).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);
    sqlx::query("INSERT INTO lore_resources (resource_id,name,owner_subject) VALUES ('api-test-resource','example',$1)")
        .bind(user_id.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO ci_pipeline_routes (resource_id,repository_url,branch,revision,pipeline_name,category,runner_os,trigger_patterns,working_directory,graph_definition) VALUES ('api-test-resource','lores://127.0.0.1:41337/example','main',$1,'server','server','linux',ARRAY['src/**'],'src','{\"stages\":[]}')")
        .bind("a".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/repositories/example/pipelines?revision={}",
                    "a".repeat(64)
                ))
                .header("cookie", &cookies)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let choices: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        choices,
        serde_json::json!([{ "name": "server", "category": "server", "runner_os": "linux" }])
    );
    let mut untrusted_repository = input();
    untrusted_repository.repository_url = "lores://untrusted.example/example".into();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/pipelines")
                .method("POST")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&untrusted_repository).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/pipelines")
                .method("POST")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&input()).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let payload: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let id: Uuid = payload["id"].as_str().unwrap().parse().unwrap();
    assert!(
        payload["revision_number"]
            .as_i64()
            .is_some_and(|number| number > 0)
    );
    db::log(&pool, id, None, "stdout", "first\0line")
        .await
        .unwrap();
    db::log(&pool, id, None, "stderr", "second line")
        .await
        .unwrap();
    let cursor: i64 = sqlx::query_scalar("SELECT min(id) FROM logs WHERE pipeline_id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/pipelines/{id}/logs?after={cursor}"))
                .header("cookie", &cookies)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let logs: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(logs.as_array().unwrap().len(), 1);
    assert_eq!(logs[0]["content"], "second line");
    let mut invalid = input();
    invalid.revision = "main@head".into();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/pipelines")
                .method("POST")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&invalid).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let runner_id = Uuid::new_v4();
    db::register_runner(
        &pool,
        runner_id,
        "offline-runner",
        "linux",
        "x86_64",
        "test",
        false,
    )
    .await
    .unwrap();
    let other_user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id,google_sub,email,name) VALUES ($1,$2,$3,$4)")
        .bind(other_user_id)
        .bind(other_user_id.to_string())
        .bind("other-developer@zenogrid.co.kr")
        .bind("Other developer")
        .execute(&pool)
        .await
        .unwrap();
    let other_pipeline = db::submit_for_user(&pool, &input(), other_user_id)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET role = 'user' WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/pipelines/{}/cancel", other_pipeline.id))
                .method("POST")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/pipelines/{id}/cancel"))
                .method("POST")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/runners/{runner_id}"))
                .method("DELETE")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    sqlx::query("UPDATE users SET role = 'admin' WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/runners")
                .header("cookie", &cookies)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let runners: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let registered = runners
        .as_array()
        .unwrap()
        .iter()
        .find(|runner| runner["id"] == runner_id.to_string())
        .unwrap();
    assert_eq!(registered["docker_available"], false);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/runners/{runner_id}"))
                .method("DELETE")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    sqlx::query("UPDATE runners SET stopped_at = now() WHERE id = $1")
        .bind(runner_id)
        .execute(&pool)
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/runners/{runner_id}"))
                .method("DELETE")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/runners/{runner_id}"))
                .method("DELETE")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/logout")
                .method("POST")
                .header("cookie", &cookies)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(response.headers().get_all("set-cookie").iter().count(), 2);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/me")
                .header("cookie", &cookies)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn pipeline_graphs_are_visible_across_workspace_accounts(pool: PgPool) {
    let auth = AuthService::new(
        pool.clone(),
        AuthConfig::new(
            "test-client".into(),
            "test-secret".into(),
            "http://127.0.0.1:8080",
        )
        .unwrap(),
    )
    .unwrap();
    let repositories = RepositoryService::new(
        "/usr/bin/false",
        "lores://127.0.0.1:41337",
        "lores://127.0.0.1:41337",
    )
    .unwrap();
    let app = api::router(pool.clone(), auth, repositories, None);

    let owner_id = Uuid::new_v4();
    let viewer_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id,google_sub,email,name) VALUES ($1,$2,$3,$4),($5,$6,$7,$8)")
        .bind(owner_id)
        .bind("graph-owner")
        .bind("graph-owner@zenogrid.co.kr")
        .bind("Graph Owner")
        .bind(viewer_id)
        .bind("graph-viewer")
        .bind("graph-viewer@zenogrid.co.kr")
        .bind("Graph Viewer")
        .execute(&pool)
        .await
        .unwrap();
    let session = "graph-viewer-session";
    let csrf = "graph-viewer-csrf";
    sqlx::query("INSERT INTO sessions (token_hash,user_id,csrf_hash,expires_at) VALUES ($1,$2,$3,now()+interval '1 hour')")
        .bind(Sha256::digest(session.as_bytes()).to_vec())
        .bind(viewer_id)
        .bind(Sha256::digest(csrf.as_bytes()).to_vec())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO lore_resources (resource_id,name,owner_subject) VALUES ('shared-graph-resource','shared-graph',$1)")
        .bind(owner_id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ci_pipeline_routes (resource_id,repository_url,branch,revision,pipeline_name,category,runner_os,trigger_patterns,working_directory,graph_definition) VALUES ('shared-graph-resource','lores://127.0.0.1:41337/shared-graph','main',$1,'build','server','linux',$2,'','{\"stages\":[]}')")
        .bind("b".repeat(64))
        .bind(vec!["**".to_string()])
        .execute(&pool)
        .await
        .unwrap();
    let legacy_pipeline_id = Uuid::new_v4();
    sqlx::query("INSERT INTO pipelines (id,repository_url,branch,revision,pipeline_name,runner_os,created_at) VALUES ($1,'lores://127.0.0.1:41337/shared-graph','0123456789abcdef0123456789abcdef',$2,'build','linux',now()-interval '1 day')")
        .bind(legacy_pipeline_id)
        .bind("b".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    let previous_revision_pipeline_id = Uuid::new_v4();
    sqlx::query("INSERT INTO pipelines (id,repository_url,branch,revision,pipeline_name,runner_os) VALUES ($1,'lores://127.0.0.1:41337/shared-graph','main',$2,'build','linux')")
        .bind(previous_revision_pipeline_id)
        .bind("a".repeat(64))
        .execute(&pool)
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/pipeline-graphs")
                .header(
                    "cookie",
                    format!("lorehub_session={session}; lorehub_csrf={csrf}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let graphs: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(graphs.as_array().unwrap().len(), 1);
    assert_eq!(graphs[0]["pipeline_name"], "build");
    assert_eq!(graphs[0]["category"], "server");
    assert_eq!(
        graphs[0]["latest_pipeline_id"],
        previous_revision_pipeline_id.to_string()
    );
    assert!(
        graphs[0]["revision_number"]
            .as_i64()
            .is_some_and(|number| number > 0)
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/pipelines")
                .header(
                    "cookie",
                    format!("lorehub_session={session}; lorehub_csrf={csrf}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let pipelines: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let legacy_pipeline = pipelines
        .as_array()
        .unwrap()
        .iter()
        .find(|pipeline| pipeline["id"] == legacy_pipeline_id.to_string())
        .unwrap();
    assert_eq!(legacy_pipeline["branch"], "main");
    assert_eq!(
        legacy_pipeline["revision_number"],
        graphs[0]["revision_number"]
    );
}

fn fake_lore(root: &std::path::Path, config: &str) -> String {
    let bin = root.join("fake-lore");
    // Verify exact argv, including the immutable revision and end-of-options marker.
    let script = format!(
        r#"#!/bin/sh
set -eu
if [ "$1" = '--version' ]; then echo 'lore fixture'; exit 0; fi
[ "$#" = 8 ]
[ "$1" = '--non-interactive' ]
[ "$2" = '--no-pager' ]
[ "$3" = 'clone' ]
[ "$4" = '--revision' ]
[ "$5" = '{}' ]
[ "$6" = '--' ]
[ "$7" = 'lores://127.0.0.1:41337/example' ]
mkdir -p "$8"
cat > "$8/.lore-ci.toml" <<'PIPELINE_CONFIG'
{config}
PIPELINE_CONFIG
"#,
        "a".repeat(64)
    );
    std::fs::write(&bin, script).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
    bin.to_str().unwrap().into()
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn worker_runs_stages_and_preserves_failure(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let lore_bin = fake_lore(
        root.path(),
        r#"
stages = ["build", "test", "deploy"]
[[jobs]]
name = "build"
stage = "build"
script = ["export MESSAGE=hello", "printf '%s' $MESSAGE > built", "test -z \"${DATABASE_URL:-}\"", "test -z \"${GOOGLE_CLIENT_SECRET:-}\""]
[[jobs]]
name = "test"
stage = "test"
script = ["cat built", "echo test-error >&2", "exit 7"]
[[jobs]]
name = "deploy"
stage = "deploy"
script = ["echo must-not-run"]
"#,
    );
    let worker = Worker {
        pool: pool.clone(),
        id: Uuid::new_v4(),
        work_dir: root.path().join("work"),
        lore_bin,
        token_issuer: None,
    };
    let pipeline = db::submit(&pool, &input()).await.unwrap();
    worker.run(CancellationToken::new(), true).await.unwrap();
    assert_eq!(status(&pool, pipeline.id).await, "failed");
    let jobs: Vec<(String, Option<i32>)> = sqlx::query_as(
        "SELECT status, exit_code FROM jobs WHERE pipeline_id = $1 ORDER BY position",
    )
    .bind(pipeline.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        jobs,
        vec![
            ("succeeded".into(), Some(0)),
            ("failed".into(), Some(7)),
            ("skipped".into(), None)
        ]
    );
    let logs: String = sqlx::query_scalar(
        "SELECT string_agg(content, '' ORDER BY id) FROM logs WHERE pipeline_id = $1",
    )
    .bind(pipeline.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        logs.contains("hello") && logs.contains("test-error") && !logs.contains("must-not-run")
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("work")).unwrap().count(),
        0
    );
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn worker_success_and_invalid_config(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let lore_bin = fake_lore(
        root.path(),
        "stages = ['test']\n[[jobs]]\nname = 'pass'\nstage = 'test'\nscript = ['echo success']",
    );
    let worker = Worker {
        pool: pool.clone(),
        id: Uuid::new_v4(),
        work_dir: root.path().join("work"),
        lore_bin,
        token_issuer: None,
    };
    let pipeline = db::submit(&pool, &input()).await.unwrap();
    worker.run(CancellationToken::new(), true).await.unwrap();
    assert_eq!(status(&pool, pipeline.id).await, "succeeded");
    fake_lore(root.path(), "stages = ['test']\njobs = []");
    let pipeline = db::submit(&pool, &input()).await.unwrap();
    worker.run(CancellationToken::new(), true).await.unwrap();
    assert_eq!(status(&pool, pipeline.id).await, "failed");
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn executor_timeout_cancel_and_log_limit(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let pipeline = db::submit(&pool, &input()).await.unwrap();
    let cancel = CancellationToken::new();
    let execution = Execution {
        pool: &pool,
        pipeline: pipeline.id,
        job: None,
        cancel: &cancel,
    };
    let mut command = executor::command("/bin/sh", root.path());
    // A surviving child would create this file after the parent times out.
    command.args(["-c", "(sleep 1; touch survived) & wait"]);
    assert!(
        execution
            .run(command, Duration::from_millis(100))
            .await
            .unwrap_err()
            .to_string()
            .contains("timed out")
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(!root.path().join("survived").exists());
    let mut command = executor::command("/bin/sh", root.path());
    command.args(["-c", "yes x | head -c 1200000"]);
    assert_eq!(
        execution
            .run(command, Duration::from_secs(10))
            .await
            .unwrap(),
        0
    );
    let bytes: i64 = sqlx::query_scalar("SELECT sum(octet_length(content))::bigint FROM logs WHERE pipeline_id = $1 AND stream <> 'system'").bind(pipeline.id).fetch_one(&pool).await.unwrap();
    assert_eq!(bytes, 1024 * 1024);
    let mut command = executor::command("/bin/sh", root.path());
    command.args(["-c", "sleep 30"]);
    let cancel_later = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel_later.cancel();
    });
    let start = std::time::Instant::now();
    assert!(
        execution
            .run(command, Duration::from_secs(60))
            .await
            .is_err()
    );
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn push_pipelines_are_idempotent_and_claimed_by_matching_os(pool: PgPool) {
    use lorehub::{ci::config::PipelineFile, server::triggers};
    let owner = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id,google_sub,email) VALUES ($1,'push-owner','push@example.test')",
    )
    .bind(owner)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES ('test-resource','test',$1)")
        .bind(owner.to_string())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sparse_workspace_views(id,owner_id,resource_id,name,mode,rules) VALUES ($1,$2,'test-resource','ServerBuildView','sparse',$3)")
        .bind(Uuid::new_v4())
        .bind(owner)
        .bind("**\n!/Server/\n!/GameDesign/\n")
        .execute(&pool)
        .await
        .unwrap();
    let mut file = PipelineFile::parse(include_str!("../examples/monorepo.lore-ci.toml")).unwrap();
    file.pipelines
        .iter_mut()
        .find(|pipeline| pipeline.name == "server")
        .unwrap()
        .sparse_view = Some("ServerBuildView".into());
    let mut tx = pool.begin().await.unwrap();
    let paths = vec!["Client/main.cs".into(), "Server/src/main.rs".into()];
    assert_eq!(
        triggers::enqueue(
            &mut tx,
            "test-resource",
            &input().repository_url,
            "main",
            &"b".repeat(64),
            &input().revision,
            owner,
            &file,
            &paths
        )
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        triggers::enqueue(
            &mut tx,
            "test-resource",
            &input().repository_url,
            "main",
            &"b".repeat(64),
            &input().revision,
            owner,
            &file,
            &paths
        )
        .await
        .unwrap(),
        0
    );
    tx.commit().await.unwrap();
    let (patterns, changed, changed_count, directory, definition): (Vec<String>, Vec<String>, i32, String, String) =
        sqlx::query_as("SELECT trigger_patterns, changed_paths, changed_path_count, working_directory, graph_definition FROM pipelines WHERE pipeline_name = 'client'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(patterns, ["Client/**"]);
    assert_eq!(changed, ["Client/main.cs"]);
    assert_eq!(changed_count, 1);
    assert_eq!(directory, "Client");
    let definition: serde_json::Value = serde_json::from_str(&definition).unwrap();
    assert_eq!(definition["stages"][0]["name"], "check");
    assert_eq!(definition["stages"][0]["jobs"][0], "client-check");
    let (view_name, view_rules): (String, String) = sqlx::query_as(
        "SELECT sparse_view_name, sparse_view_rules FROM pipelines WHERE pipeline_name='server'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(view_name, "ServerBuildView");
    assert_eq!(view_rules, "**\n!/Server/\n!/GameDesign/\n");
    let windows = Uuid::new_v4();
    let macos = Uuid::new_v4();
    let linux = Uuid::new_v4();
    for (id, os) in [(windows, "windows"), (macos, "macos"), (linux, "linux")] {
        db::register_runner(&pool, id, os, os, "x86_64", "test", false)
            .await
            .unwrap();
    }
    assert!(db::claim(&pool, linux).await.unwrap().is_none());
    // Even an old unfiltered worker query cannot start a job on the wrong OS.
    assert!(
        sqlx::query(
            "UPDATE pipelines SET status='running', worker_id=$1 WHERE pipeline_name='client'"
        )
        .bind(linux)
        .execute(&pool)
        .await
        .is_err()
    );
    let (first, second) = tokio::join!(db::claim(&pool, macos), db::claim(&pool, macos));
    let claimed: Vec<_> = [first.unwrap(), second.unwrap()]
        .into_iter()
        .flatten()
        .collect();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].pipeline_name.as_deref(), Some("server"));
    assert_eq!(claimed[0].runner_os.as_deref(), Some("macos"));
    let client = db::claim(&pool, windows).await.unwrap().unwrap();
    assert_eq!(client.pipeline_name.as_deref(), Some("client"));
    assert!(db::claim(&pool, windows).await.unwrap().is_none());
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn named_pipeline_executes_only_selected_jobs_in_its_directory(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let config = format!(
        r#"
[[pipelines]]
name = 'server'
runner_os = '{}'
changes = ['Server/**']
working_directory = 'Server'
stages = ['test']
[[pipelines.jobs]]
name = 'server-test'
stage = 'test'
script = ['test "$(basename "$PWD")" = Server', 'echo server-selected']
[[pipelines]]
name = 'client'
runner_os = 'windows'
changes = ['Client/**']
working_directory = 'Client'
stages = ['test']
[[pipelines.jobs]]
name = 'client-test'
stage = 'test'
script = ['exit 99']
"#,
        std::env::consts::OS
    );
    let lore_bin = fake_lore(root.path(), &config);
    let mut script = std::fs::read_to_string(&lore_bin).unwrap();
    script.push_str("\nmkdir -p \"$8/Server\"\n");
    std::fs::write(&lore_bin, &script).unwrap();
    let worker = Worker {
        pool: pool.clone(),
        id: Uuid::new_v4(),
        work_dir: root.path().join("work"),
        lore_bin: lore_bin.clone(),
        token_issuer: None,
    };
    let pipeline = db::submit(&pool, &input()).await.unwrap();
    sqlx::query("UPDATE pipelines SET pipeline_name='server', runner_os=$2 WHERE id=$1")
        .bind(pipeline.id)
        .bind(std::env::consts::OS)
        .execute(&pool)
        .await
        .unwrap();
    worker.run(CancellationToken::new(), true).await.unwrap();
    assert_eq!(status(&pool, pipeline.id).await, "succeeded");
    let jobs: Vec<String> = sqlx::query_scalar("SELECT name FROM jobs WHERE pipeline_id=$1")
        .bind(pipeline.id)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, ["server-test"]);

    // A repository-controlled symlink cannot move execution outside its checkout.
    script = script.replace("mkdir -p \"$8/Server\"", "ln -s /tmp \"$8/Server\"");
    std::fs::write(&lore_bin, script).unwrap();
    let escaped = db::submit(&pool, &input()).await.unwrap();
    sqlx::query("UPDATE pipelines SET pipeline_name='server', runner_os=$2 WHERE id=$1")
        .bind(escaped.id)
        .bind(std::env::consts::OS)
        .execute(&pool)
        .await
        .unwrap();
    worker.run(CancellationToken::new(), true).await.unwrap();
    assert_eq!(status(&pool, escaped.id).await, "failed");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE pipeline_id=$1")
        .bind(escaped.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
