use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use lorehub::server::{
    api,
    auth::{AuthConfig, AuthService},
    repositories::RepositoryService,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn ci_analysis_requires_repository_access_and_csrf_without_lore_calls(pool: PgPool) {
    let app = api::router(
        pool.clone(),
        AuthService::new(
            pool.clone(),
            AuthConfig::new("test".into(), "test".into(), "http://127.0.0.1:8080").unwrap(),
        )
        .unwrap(),
        RepositoryService::new(
            "/usr/bin/false",
            "lores://127.0.0.1:41337",
            "lores://127.0.0.1:41337",
        )
        .unwrap(),
        None,
    );
    let owner = Uuid::new_v4();
    let outsider = Uuid::new_v4();
    for id in [owner, outsider] {
        sqlx::query("INSERT INTO users(id,google_sub,email) VALUES($1,$2,$3)")
            .bind(id)
            .bind(id.to_string())
            .bind(format!("{id}@zenogrid.co.kr"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,csrf_hash,expires_at) VALUES($1,$2,$3,now()+interval '1 hour')")
            .bind(Sha256::digest(id.to_string().as_bytes()).to_vec()).bind(id)
            .bind(Sha256::digest(b"test-csrf").to_vec()).execute(&pool).await.unwrap();
    }
    sqlx::query(
        "INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES('ci-test','ci-test',$1)",
    )
    .bind(owner.to_string())
    .execute(&pool)
    .await
    .unwrap();
    let path = "/api/v1/repositories/ci-test/ci-config/analyze";
    let content = "stages = [\"build\"]\n[[jobs]]\nname = \"build\"\nstage = \"build\"\nscript = [\"echo test\"]";
    let input = json!({"content":content, "changed_paths":["src/main.rs"]});
    for (user, csrf, expected) in [
        (None, true, StatusCode::UNAUTHORIZED),
        (Some(owner), false, StatusCode::FORBIDDEN),
        (Some(outsider), true, StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            request(&app, user, "POST", path, input.clone(), csrf)
                .await
                .0,
            expected
        );
    }
    let (status, result) = request(&app, Some(owner), "POST", path, input.clone(), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["valid"], true);
    assert_eq!(result["manual"], true);
    let (status, result) = request(
        &app,
        Some(owner),
        "POST",
        path,
        json!({"content": format!("{content}\ntimeout_seconds = 0")}),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["valid"], false);
    assert_eq!(result["diagnostics"][0]["job_index"], 0);
    assert_eq!(result["diagnostics"][0]["field"], "timeout_seconds");
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            path,
            json!({"content":content, "changed_paths":["../outside"]}),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM pipelines")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
}

#[cfg(unix)]
#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn link_creation_recovers_partial_failure_without_recreating_source(pool: PgPool) {
    use lorehub::server::tokens::TokenIssuer;
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("lore");
    let script = r#"#!/bin/sh
set -eu
shift 7
case "${1:-}:${2:-}" in
  repository:list)
    printf '%s\n' '{"tagName":"repositoryListEntry","data":{"id":"11111111111111111111111111111111","name":"root"}}'
    printf '%s\n' '{"tagName":"repositoryListEntry","data":{"id":"22222222222222222222222222222222","name":"source"}}'
    ;;
  repository:clone) mkdir -p "$6" ;;
  --repository:repository)
    printf '%s\n' '{"tagName":"branchListEntry","data":{"id":"main-id","name":"main","location":"remote","archived":false,"latest":"REV"}}'
    ;;
  clone:--revision)
    mkdir -p "$6"
    case "$5" in
      */source)
        touch "$6/.source-checkout"
        if [ -f "WORK/source-push" ]; then mkdir -p "$6/Test"; fi
        ;;
    esac
    ;;
  --repository:.)
    if [ -f "WORK/root-push" ]; then
      printf '%s\n' '{"tagName":"linkEntry","data":{"link":"22222222222222222222222222222222","linkPath":"Test","sourcePath":"Test","branch":"main-id","tracking":true,"revision":"REV"}}'
    fi
    ;;
  stage:--scan) [ -d Test ] ;;
  commit:*) ;;
  link:add)
    if [ ! -f "WORK/allow-root" ]; then
      printf '%s\n' '{"tagName":"complete","data":{"status":1,"error":{"message":"Failed to add link: Link divergence"}}}'
      exit 1
    fi
    ;;
  push:)
    if [ -f .source-checkout ]; then printf 'push\n' >> "WORK/source-push"; else printf 'push\n' >> "WORK/root-push"; fi
    printf '%s\n' '{"tagName":"branchPushRevisionPushEnd","data":{"newRemoteRevision":"REV"}}'
    ;;
  *) exit 99 ;;
esac
printf '%s\n' '{"tagName":"complete","data":{"status":0}}'
"#.replace("WORK", root.path().to_str().unwrap()).replace("REV", &"a".repeat(64));
    std::fs::write(&binary, script).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let app = api::router(
        pool.clone(),
        AuthService::new(
            pool.clone(),
            AuthConfig::new("test".into(), "test".into(), "http://127.0.0.1:8080").unwrap(),
        )
        .unwrap(),
        RepositoryService::new(binary, "lores://127.0.0.1:41337", "lores://127.0.0.1:41337")
            .unwrap(),
        Some(
            TokenIssuer::from_files(
                "tests/fixtures/test-private.pem",
                "tests/fixtures/test-jwks.json",
                "http://127.0.0.1:8080",
                "zenogrid.co.kr",
            )
            .unwrap(),
        ),
    );
    let owner = Uuid::new_v4();
    let outsider = Uuid::new_v4();
    for id in [owner, outsider] {
        sqlx::query("INSERT INTO users(id,google_sub,email) VALUES($1,$2,$3)")
            .bind(id)
            .bind(id.to_string())
            .bind(format!("{id}@zenogrid.co.kr"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,csrf_hash,expires_at) VALUES($1,$2,$3,now()+interval '1 hour')")
            .bind(Sha256::digest(id.to_string().as_bytes()).to_vec()).bind(id).bind(Sha256::digest(b"test-csrf").to_vec()).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE users SET role='user' WHERE id=$1")
        .bind(outsider)
        .execute(&pool)
        .await
        .unwrap();
    for (id, name) in [
        ("urc-11111111111111111111111111111111", "root"),
        ("urc-22222222222222222222222222222222", "source"),
    ] {
        sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES($1,$2,$3)")
            .bind(id)
            .bind(name)
            .bind(owner.to_string())
            .execute(&pool)
            .await
            .unwrap();
    }
    let id = Uuid::new_v4();
    let input = json!({"operation_id":id,"branch":"main","expected_revision":"a".repeat(64),"path":"Test","source_repository":"source","source_branch":"main","source_path":"Test","auto_update":false});
    let path = "/api/v1/repositories/root/links";
    assert_eq!(
        request(&app, Some(owner), "POST", path, input.clone(), false)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, Some(outsider), "POST", path, input.clone(), true)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let failed = request(&app, Some(owner), "POST", path, input.clone(), true).await;
    assert_eq!(failed.0, StatusCode::CONFLICT, "{}", failed.1);
    assert_eq!(failed.1["status"], "partial");
    assert_eq!(failed.1["stage"], "root");
    assert_eq!(failed.1["source_path_created"], true);
    assert!(
        failed.1["error"]
            .as_str()
            .unwrap()
            .contains("Link divergence")
    );
    // Replaying the accepted create does not run it again.
    assert_eq!(
        request(&app, Some(owner), "POST", path, input.clone(), true)
            .await
            .1["id"],
        id.to_string()
    );
    let mut different = input.clone();
    different["path"] = json!("Different");
    assert_eq!(
        request(&app, Some(owner), "POST", path, different, true)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let retry = format!("/api/v1/repositories/root/link-operations/{id}/retry");
    assert_eq!(
        request(&app, Some(outsider), "POST", &retry, Value::Null, true)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    std::fs::write(root.path().join("allow-root"), "").unwrap();
    let succeeded = request(&app, Some(owner), "POST", &retry, Value::Null, true).await;
    assert_eq!(succeeded.0, StatusCode::OK, "{}", succeeded.1);
    assert_eq!(succeeded.1["status"], "succeeded");
    assert_eq!(succeeded.1["revision"], "a".repeat(64));
    assert_eq!(succeeded.1["source_path_created"], true);
    assert_eq!(
        std::fs::read_to_string(root.path().join("source-push")).unwrap(),
        "push\n"
    );
    assert_eq!(
        request(&app, Some(owner), "POST", &retry, Value::Null, true)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("root-push")).unwrap(),
        "push\n"
    );
    let links = request(
        &app,
        Some(owner),
        "GET",
        "/api/v1/repositories/root/links?branch=main",
        Value::Null,
        false,
    )
    .await;
    assert_eq!(links.0, StatusCode::OK, "{}", links.1);
    assert_eq!(links.1["links"][0]["auto_update"], false);
    assert_eq!(links.1["links"][0]["source_branch_name"], "main");
    assert_eq!(links.1["links"][0]["status"], "current");
    let policy_path = "/api/v1/repositories/root/links/policy";
    let mut policy = json!({"branch":"main","path":"Test","expected_revision":"b".repeat(64),"auto_update":true});
    assert_eq!(
        request(&app, Some(owner), "POST", policy_path, policy.clone(), true)
            .await
            .0,
        StatusCode::CONFLICT
    );
    policy["expected_revision"] = json!("a".repeat(64));
    assert_eq!(
        request(&app, Some(owner), "POST", policy_path, policy, true)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("root-push")).unwrap(),
        "push\n",
        "policy changes must not push Lore"
    );
    sqlx::query("INSERT INTO repository_link_snapshots(root_resource_id,root_branch,root_revision) VALUES('urc-11111111111111111111111111111111','main',$1)").bind("a".repeat(64)).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO repository_link_dependencies(root_resource_id,root_branch,root_revision,link_path,source_resource_id,source_branch_id,source_revision,tracking) VALUES('urc-11111111111111111111111111111111','main',$1,'Test','urc-22222222222222222222222222222222','main-id',$1,true)").bind("a".repeat(64)).execute(&pool).await.unwrap();
    let summary = request(
        &app,
        Some(owner),
        "GET",
        "/api/v1/repository-links/summary",
        Value::Null,
        false,
    )
    .await;
    assert_eq!(summary.1[0]["count"], 1);
    let hidden = request(
        &app,
        Some(outsider),
        "GET",
        "/api/v1/repository-links/summary",
        Value::Null,
        false,
    )
    .await;
    assert_eq!(hidden.1, json!([]));
    let history = request(
        &app,
        Some(owner),
        "GET",
        "/api/v1/repositories/root/link-operations?branch=main",
        Value::Null,
        false,
    )
    .await;
    assert_eq!(history.1.as_array().unwrap().len(), 1);
    assert_eq!(history.1[0]["status"], "succeeded");
}

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

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn runner_maintenance_requires_current_admin_and_csrf(pool: PgPool) {
    use lorehub::ci::{config::SubmitPipeline, db};
    let app = api::router(
        pool.clone(),
        AuthService::new(
            pool.clone(),
            AuthConfig::new("test".into(), "test".into(), "http://127.0.0.1:8080").unwrap(),
        )
        .unwrap(),
        RepositoryService::new(
            "/usr/bin/false",
            "lores://127.0.0.1:41337",
            "lores://127.0.0.1:41337",
        )
        .unwrap(),
        None,
    );
    let admin = Uuid::new_v4();
    let user = Uuid::new_v4();
    for (id, role) in [(admin, "admin"), (user, "user")] {
        sqlx::query("INSERT INTO users(id,google_sub,email,role) VALUES($1,$2,$3,$4)")
            .bind(id)
            .bind(id.to_string())
            .bind(format!("{id}@zenogrid.co.kr"))
            .bind(role)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,csrf_hash,expires_at) VALUES($1,$2,$3,now()+interval '1 hour')")
            .bind(Sha256::digest(id.to_string().as_bytes()).to_vec()).bind(id)
            .bind(Sha256::digest(b"test-csrf").to_vec()).execute(&pool).await.unwrap();
    }
    let worker = Uuid::new_v4();
    db::register_runner(&pool, worker, "maintenance", "linux", "test", "test", false)
        .await
        .unwrap();
    let path = format!("/api/v1/runners/{worker}/drain");
    for (subject, csrf, expected) in [
        (None, true, StatusCode::UNAUTHORIZED),
        (Some(user), true, StatusCode::FORBIDDEN),
        (Some(admin), false, StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            request(&app, subject, "POST", &path, json!({"draining":true}), csrf)
                .await
                .0,
            expected
        );
        assert!(!db::list_runners(&pool).await.unwrap()[0].draining);
    }
    for invalid in [
        json!({}),
        json!({"draining":"true"}),
        json!({"draining":true,"unexpected":true}),
    ] {
        assert_eq!(
            request(&app, Some(admin), "POST", &path, invalid, true)
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let pipeline = db::submit(
        &pool,
        &SubmitPipeline {
            repository_url: "lores://127.0.0.1:41337/private".into(),
            revision: "a".repeat(64),
            branch: None,
            pipeline_name: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        db::claim(&pool, worker).await.unwrap().unwrap().id,
        pipeline.id
    );
    for _ in 0..2 {
        assert_eq!(
            request(
                &app,
                Some(admin),
                "POST",
                &path,
                json!({"draining":true}),
                true
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
    }
    let listed = request(
        &app,
        Some(user),
        "GET",
        "/api/v1/runners",
        Value::Null,
        false,
    )
    .await;
    assert_eq!(listed.0, StatusCode::OK);
    assert_eq!(listed.1[0]["draining"], true);
    assert_eq!(listed.1[0]["busy"], true);
    assert_eq!(listed.1[0]["diagnostic"], "draining");
    assert!(listed.1[0]["last_claim_at"].is_string());
    assert!(listed.1[0]["observed_at"].is_string());
    assert!(listed.1[0]["current_pipeline_id"].is_null());
    assert_eq!(
        request(
            &app,
            Some(admin),
            "POST",
            &format!("/api/v1/runners/{}/drain", Uuid::new_v4()),
            json!({"draining":true}),
            true
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &app,
            Some(admin),
            "POST",
            &path,
            json!({"draining":false}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert!(!db::list_runners(&pool).await.unwrap()[0].draining);
    sqlx::query("UPDATE users SET role='user' WHERE id=$1")
        .bind(admin)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(
            &app,
            Some(admin),
            "POST",
            &path,
            json!({"draining":true}),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert!(!db::list_runners(&pool).await.unwrap()[0].draining);
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn admin_can_use_another_users_repository_and_demotion_removes_access(pool: PgPool) {
    use lorehub::server::tokens::TokenIssuer;
    let tokens = TokenIssuer::from_files(
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
        RepositoryService::new(
            "/usr/bin/false",
            "lores://127.0.0.1:41337",
            "lores://127.0.0.1:41337",
        )
        .unwrap(),
        Some(tokens.clone()),
    );
    let admin = Uuid::new_v4();
    let owner = Uuid::new_v4();
    for id in [admin, owner] {
        sqlx::query("INSERT INTO users(id,google_sub,email) VALUES($1,$2,$3)")
            .bind(id)
            .bind(id.to_string())
            .bind(format!("{id}@zenogrid.co.kr"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,csrf_hash,expires_at) VALUES($1,$2,$3,now()+interval '1 hour')")
            .bind(Sha256::digest(id.to_string().as_bytes()).to_vec()).bind(id).bind(Sha256::digest(b"test-csrf").to_vec()).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES('urc-test','test-project',$1)").bind(owner.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO ci_pipeline_routes(resource_id,repository_url,branch,revision,pipeline_name,category,runner_os,trigger_patterns,working_directory,graph_definition) VALUES('urc-test','lores://127.0.0.1:41337/test-project','main',$1,'build','server','linux',ARRAY['**'],'.','{\"stages\":[]}'::jsonb)")
        .bind("a".repeat(64)).execute(&pool).await.unwrap();
    let pipeline_path = format!(
        "/api/v1/repositories/test-project/pipelines?revision={}",
        "a".repeat(64)
    );
    assert_eq!(
        request(&app, Some(admin), "GET", &pipeline_path, Value::Null, false)
            .await
            .0,
        StatusCode::OK
    );
    let repositories = request(
        &app,
        Some(admin),
        "GET",
        "/api/v1/workspace-repositories",
        Value::Null,
        false,
    )
    .await;
    assert_eq!(repositories.0, StatusCode::OK);
    assert_eq!(repositories.1[0]["name"], "test-project");
    let token = request(
        &app,
        Some(admin),
        "POST",
        "/api/v1/lore-token",
        Value::Null,
        true,
    )
    .await;
    assert_eq!(token.0, StatusCode::OK);
    assert!(
        tokens
            .verify_access_token(token.1["access_token"].as_str().unwrap())
            .unwrap()
            .has_exact_resource("urc-test")
    );
    let submission =
        json!({"repository_url":"lores://127.0.0.1:41337/test-project", "revision":"a".repeat(64)});
    assert_eq!(
        request(
            &app,
            Some(admin),
            "POST",
            "/api/v1/pipelines",
            submission.clone(),
            false
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
            "/api/v1/pipelines",
            submission.clone(),
            true
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let view = request(
        &app,
        Some(admin),
        "POST",
        "/api/v1/sparse-views",
        json!({"name":"Admin view", "resource_id":"urc-test", "mode":"full", "rules":""}),
        true,
    )
    .await;
    assert_eq!(view.0, StatusCode::CREATED);
    let view_path = format!("/api/v1/sparse-views/{}", view.1["id"].as_str().unwrap());
    let views = request(
        &app,
        Some(admin),
        "GET",
        "/api/v1/sparse-views",
        Value::Null,
        false,
    )
    .await
    .1;
    assert_eq!(views[0]["can_manage"], true);
    let update = json!({"name":"Updated admin view", "mode":"full", "rules":""});
    assert_eq!(
        request(&app, Some(admin), "POST", &view_path, update.clone(), true)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let group = request(
        &app,
        Some(admin),
        "POST",
        "/api/v1/account-groups",
        json!({"name":"Admin group", "description":"", "member_ids":[]}),
        true,
    )
    .await;
    assert_eq!(group.0, StatusCode::CREATED);
    let group_path = format!(
        "/api/v1/account-groups/{}/views",
        group.1["id"].as_str().unwrap()
    );
    let selection_path = format!("{group_path}/urc-test");
    assert_eq!(
        request(
            &app,
            Some(admin),
            "POST",
            &selection_path,
            json!({"view_id":view.1["id"]}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, Some(admin), "GET", &group_path, Value::Null, false)
            .await
            .1[0]["can_manage"],
        true
    );

    sqlx::query("UPDATE users SET role='user' WHERE id=$1")
        .bind(admin)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, Some(admin), "GET", &pipeline_path, Value::Null, false)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(admin),
            "POST",
            "/api/v1/pipelines",
            submission,
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
            "GET",
            "/api/v1/workspace-repositories",
            Value::Null,
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let token = request(
        &app,
        Some(admin),
        "POST",
        "/api/v1/lore-token",
        Value::Null,
        true,
    )
    .await
    .1;
    assert!(
        !tokens
            .verify_access_token(token["access_token"].as_str().unwrap())
            .unwrap()
            .has_exact_resource("urc-test")
    );
    assert_eq!(
        request(&app, Some(admin), "POST", &view_path, update, true)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, Some(admin), "GET", &group_path, Value::Null, false)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(admin),
            "POST",
            &selection_path,
            json!({"view_id":view.1["id"]}),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, Some(owner), "GET", &pipeline_path, Value::Null, false)
            .await
            .0,
        StatusCode::OK
    );
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn account_groups_and_views_enforce_ownership_and_persist(pool: PgPool) {
    let auth = AuthService::new(
        pool.clone(),
        AuthConfig::new("test".into(), "test".into(), "http://127.0.0.1:8080").unwrap(),
    )
    .unwrap();
    let app = api::router(
        pool.clone(),
        auth,
        RepositoryService::new(
            "/usr/bin/false",
            "lores://127.0.0.1:41337",
            "lores://127.0.0.1:41337",
        )
        .unwrap(),
        None,
    );
    let owner = Uuid::new_v4();
    let member = Uuid::new_v4();
    let outsider = Uuid::new_v4();
    let other_admin = Uuid::new_v4();
    for id in [owner, member, outsider, other_admin] {
        sqlx::query("INSERT INTO users(id,google_sub,email,name) VALUES($1,$2,$3,'Google name')")
            .bind(id)
            .bind(id.to_string())
            .bind(format!("{id}@zenogrid.co.kr"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,csrf_hash,expires_at) VALUES($1,$2,$3,now()+interval '1 hour')")
            .bind(Sha256::digest(id.to_string().as_bytes()).to_vec()).bind(id).bind(Sha256::digest(b"test-csrf").to_vec()).execute(&pool).await.unwrap();
    }
    for (resource, owner_id) in [("urc-owned", owner), ("urc-other", outsider)] {
        sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES($1,$1,$2)")
            .bind(resource)
            .bind(owner_id.to_string())
            .execute(&pool)
            .await
            .unwrap();
    }
    assert_eq!(
        request(&app, None, "GET", "/api/v1/accounts", Value::Null, false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            "/api/v1/accounts/me",
            json!({"name":"New name"}),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            "/api/v1/accounts/me",
            json!({"name":"New name"}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let me = request(&app, Some(owner), "GET", "/api/v1/me", Value::Null, false)
        .await
        .1;
    assert_eq!(me["name"], "New name");
    assert_eq!(me["role"], "admin");
    // Google refresh must not overwrite the user's display-name preference.
    sqlx::query("UPDATE users SET name='Changed Google name' WHERE id=$1")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, Some(owner), "GET", "/api/v1/me", Value::Null, false)
            .await
            .1["name"],
        "New name"
    );
    let accounts = request(
        &app,
        Some(owner),
        "GET",
        "/api/v1/accounts",
        Value::Null,
        false,
    )
    .await
    .1;
    assert_eq!(accounts.as_array().unwrap().len(), 4);
    assert_eq!(
        accounts
            .as_array()
            .unwrap()
            .iter()
            .find(|account| account["id"] == owner.to_string())
            .unwrap()["role"],
        "admin"
    );
    let other_admin_role_path = format!("/api/v1/accounts/{other_admin}/role");
    assert_eq!(
        request(
            &app,
            Some(member),
            "POST",
            &other_admin_role_path,
            json!({"role":"admin"}),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &other_admin_role_path,
            json!({"role":"admin"}),
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &other_admin_role_path,
            json!({"role":"admin"}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(
            &app,
            Some(other_admin),
            "GET",
            "/api/v1/me",
            Value::Null,
            false,
        )
        .await
        .1["role"],
        "admin"
    );
    let owner_role_path = format!("/api/v1/accounts/{owner}/role");
    assert_eq!(
        request(
            &app,
            Some(other_admin),
            "POST",
            &owner_role_path,
            json!({"role":"user"}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(
            &app,
            Some(other_admin),
            "POST",
            &other_admin_role_path,
            json!({"role":"user"}),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            Some(other_admin),
            "POST",
            &owner_role_path,
            json!({"role":"admin"}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let input = json!({"name":"Engine", "description":"Team", "member_ids":[member, member]});
    let (status, group) = request(
        &app,
        Some(owner),
        "POST",
        "/api/v1/account-groups",
        input.clone(),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{group}");
    assert_eq!(group["member_ids"].as_array().unwrap().len(), 2);
    let id = group["id"].as_str().unwrap();
    let group_path = format!("/api/v1/account-groups/{id}");
    let access_path = "/api/v1/repository-group-access/urc-owned";
    assert_eq!(
        request(
            &app,
            Some(member),
            "POST",
            access_path,
            json!({"group_ids":[id]}),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            access_path,
            json!({"group_ids":[id, id]}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let repository_access = request(
        &app,
        Some(owner),
        "GET",
        "/api/v1/repository-group-access",
        Value::Null,
        false,
    )
    .await;
    assert_eq!(repository_access.0, StatusCode::OK);
    // Administrators see all repositories, sorted by name, not ownership.
    assert_eq!(
        repository_access.1["repositories"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let owned = repository_access.1["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|repository| repository["resource_id"] == "urc-owned")
        .unwrap();
    assert_eq!(owned["group_ids"], json!([id]));
    assert_eq!(repository_access.1["groups"][0]["id"], id);
    let member_grants: Vec<String> = sqlx::query_scalar("SELECT access.resource_id FROM repository_account_group_access access JOIN account_group_members members USING(group_id) WHERE members.user_id=$1 ORDER BY access.resource_id")
        .bind(member).fetch_all(&pool).await.unwrap();
    assert_eq!(member_grants, ["urc-owned"]);
    let outsider_granted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM repository_account_group_access access JOIN account_group_members members USING(group_id) WHERE access.resource_id='urc-owned' AND members.user_id=$1)")
        .bind(outsider).fetch_one(&pool).await.unwrap();
    assert!(!outsider_granted);
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            "/api/v1/account-groups",
            input.clone(),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let groups = request(
        &app,
        Some(member),
        "GET",
        "/api/v1/account-groups",
        Value::Null,
        false,
    )
    .await;
    assert_eq!(groups.0, StatusCode::FORBIDDEN);
    let administrator_groups = request(
        &app,
        Some(other_admin),
        "GET",
        "/api/v1/account-groups",
        Value::Null,
        false,
    )
    .await
    .1;
    assert_eq!(administrator_groups.as_array().unwrap().len(), 1);
    assert_eq!(administrator_groups[0]["id"], id);
    assert_eq!(
        request(
            &app,
            Some(outsider),
            "GET",
            "/api/v1/account-groups",
            Value::Null,
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, Some(member), "POST", &group_path, input.clone(), true)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, Some(member), "DELETE", &group_path, Value::Null, true)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(other_admin),
            "POST",
            &group_path,
            input.clone(),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let selection_path = format!("{group_path}/views/urc-owned");
    let view = json!({"name":"Backend", "resource_id":"urc-owned", "mode":"sparse", "rules":"**\n!/src/\n/src/generated/\n"});
    assert_eq!(
        request(
            &app,
            Some(member),
            "POST",
            "/api/v1/sparse-views",
            view.clone(),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(member),
            "POST",
            "/api/v1/sparse-views",
            json!({"name":"Other", "resource_id":"urc-other", "mode":"sparse", "rules":"**\n!/src/"}),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            "/api/v1/sparse-views",
            json!({"name":"Invalid", "resource_id":"urc-owned", "mode":"sparse","rules":"# comments"}),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, created_view) = request(
        &app,
        Some(owner),
        "POST",
        "/api/v1/sparse-views",
        view.clone(),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created_view}");
    let view_id = created_view["id"].as_str().unwrap();
    let view_path = format!("/api/v1/sparse-views/{view_id}");
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            "/api/v1/sparse-views",
            view.clone(),
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            Some(member),
            "GET",
            "/api/v1/sparse-views",
            Value::Null,
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let administrator_views = request(
        &app,
        Some(other_admin),
        "GET",
        "/api/v1/sparse-views",
        Value::Null,
        false,
    )
    .await
    .1;
    assert_eq!(administrator_views.as_array().unwrap().len(), 1);
    assert_eq!(administrator_views[0]["name"], "Backend");
    assert_eq!(administrator_views[0]["can_manage"], false);
    assert_eq!(
        request(
            &app,
            Some(member),
            "POST",
            &selection_path,
            json!({"view_id":view_id}),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &format!("{group_path}/views/urc-other"),
            json!({"view_id":view_id}),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &selection_path,
            json!({"view_id":view_id}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let views = request(
        &app,
        Some(member),
        "GET",
        &format!("{group_path}/views"),
        Value::Null,
        false,
    )
    .await;
    assert_eq!(views.0, StatusCode::FORBIDDEN);
    let administrator_group_views = request(
        &app,
        Some(other_admin),
        "GET",
        &format!("{group_path}/views"),
        Value::Null,
        false,
    )
    .await;
    assert_eq!(administrator_group_views.0, StatusCode::OK);
    assert_eq!(administrator_group_views.1[0]["rules"], view["rules"]);
    assert_eq!(administrator_group_views.1[0]["view_id"], view_id);
    assert_eq!(administrator_group_views.1[0]["can_manage"], false);
    assert_eq!(
        request(
            &app,
            Some(member),
            "GET",
            "/api/v1/sparse-views",
            Value::Null,
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(outsider),
            "GET",
            &format!("{group_path}/views"),
            Value::Null,
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, Some(member), "DELETE", &view_path, Value::Null, true)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(other_admin),
            "POST",
            &view_path,
            json!({"name":"Other admin edit", "mode":"full","rules":""}),
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &view_path,
            json!({"name":"Backend full", "mode":"full","rules":"**"}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "GET",
            &format!("{group_path}/views"),
            Value::Null,
            false
        )
        .await
        .1[0]["rules"],
        ""
    );
    // A failed membership edit rolls back the name and all membership changes.
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &group_path,
            json!({"name":"Broken", "description":"", "member_ids":[Uuid::new_v4()]}),
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
            "GET",
            "/api/v1/account-groups",
            Value::Null,
            false
        )
        .await
        .1[0]["name"],
        "Engine"
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &group_path,
            json!({"name":"Renamed", "description":"", "member_ids":[]}),
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    let member_granted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM repository_account_group_access access JOIN account_group_members members USING(group_id) WHERE access.resource_id='urc-owned' AND members.user_id=$1)")
        .bind(member).fetch_one(&pool).await.unwrap();
    assert!(!member_granted);
    assert_eq!(
        request(
            &app,
            Some(member),
            "GET",
            &format!("{group_path}/views"),
            Value::Null,
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "DELETE",
            &selection_path,
            Value::Null,
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(
            &app,
            Some(owner),
            "POST",
            &selection_path,
            json!({"view_id":view_id}),
            true
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, Some(owner), "DELETE", &group_path, Value::Null, true)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM account_group_view_selections")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM repository_account_group_access")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sparse_workspace_views")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        request(&app, Some(owner), "DELETE", &view_path, Value::Null, true)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sparse_workspace_views")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM lore_resources")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn operations_requires_live_admin_and_excludes_unregistered_execution_data(pool: PgPool) {
    let app = api::router(
        pool.clone(),
        AuthService::new(
            pool.clone(),
            AuthConfig::new("test".into(), "test".into(), "http://127.0.0.1:8080").unwrap(),
        )
        .unwrap(),
        RepositoryService::new(
            "/usr/bin/false",
            "lores://127.0.0.1:41337",
            "lores://127.0.0.1:41337",
        )
        .unwrap(),
        None,
    );
    let admin = Uuid::new_v4();
    let user = Uuid::new_v4();
    for (id, role) in [(admin, "admin"), (user, "user")] {
        sqlx::query("INSERT INTO users(id,google_sub,email,role) VALUES($1,$2,$3,$4)")
            .bind(id)
            .bind(id.to_string())
            .bind(format!("{id}@zenogrid.co.kr"))
            .bind(role)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,csrf_hash,expires_at) VALUES($1,$2,$3,now()+interval '1 hour')")
            .bind(Sha256::digest(id.to_string().as_bytes()).to_vec()).bind(id).bind(Sha256::digest(b"test-csrf").to_vec()).execute(&pool).await.unwrap();
    }
    let path = "/api/v1/operations";
    for (user, expected) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some(user), StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            request(&app, user, "GET", path, Value::Null, false).await.0,
            expected
        );
    }
    sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject,created_at) VALUES('ops','ops',$1,now()-interval '1 day')")
        .bind(user.to_string()).execute(&pool).await.unwrap();
    let (status, empty) = request(&app, Some(admin), "GET", path, Value::Null, false).await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["queue"]["queued"], 0);
    assert!(empty["queue"]["oldest_wait_seconds"].is_null());
    assert_eq!(empty["repositories"][0]["state"], "unknown");
    for (name, os, seen, stopped) in [
        ("linux", "linux", 0, false),
        ("macos", "macos", 0, false),
        ("offline", "windows", 60, false),
        ("stopped", "linux", 0, true),
    ] {
        sqlx::query("INSERT INTO runners(id,name,os,arch,version,last_seen,stopped_at) VALUES($1,$2,$3,'test','test',now()-make_interval(secs=>$4),CASE WHEN $5 THEN now() END)")
            .bind(Uuid::new_v4()).bind(name).bind(os).bind(f64::from(seen)).bind(stopped).execute(&pool).await.unwrap();
    }
    for (name, status, os) in [
        ("upstream", "running", "linux"),
        ("dependency", "queued", "linux"),
        ("ready", "queued", "macos"),
        ("busy", "queued", "linux"),
        ("no_runner", "queued", "windows"),
        ("canceling", "queued", "linux"),
    ] {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO pipelines(id,repository_url,revision,pipeline_name,status,runner_os,created_at) VALUES($1,'lores://127.0.0.1:41337/ops','rev',$2,$3,$4,now()-interval '5 minutes')")
            .bind(id).bind(name).bind(status).bind(os).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE pipelines SET worker_id=(SELECT id FROM runners WHERE name='linux'),lease_until=now()-interval '1 minute' WHERE pipeline_name='upstream'").execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE pipelines SET pipeline_needs=ARRAY['upstream'] WHERE pipeline_name='dependency'",
    )
    .execute(&pool)
    .await
    .unwrap();
    // Missing upstreams do not block claim(), so diagnostics must not invent one.
    sqlx::query("UPDATE pipelines SET pipeline_needs=ARRAY['missing'] WHERE pipeline_name='ready'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE pipelines SET cancel_requested=true WHERE pipeline_name='canceling'")
        .execute(&pool)
        .await
        .unwrap();
    for url in ["lores://elsewhere/ops", "lores://127.0.0.1:41337/deleted"] {
        sqlx::query("INSERT INTO pipelines(id,repository_url,revision) VALUES($1,$2,'hidden')")
            .bind(Uuid::new_v4())
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();
    }
    for (name, age, error) in [
        ("fresh", 0, None),
        ("failed", 0, Some("watch_failed")),
        ("stale", 180, None),
    ] {
        sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES($1,$1,$2)")
            .bind(name)
            .bind(user.to_string())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ci_repository_watch_health(resource_id,checked_at,last_success_at,error_code) VALUES($1,now()-make_interval(secs=>$2),now()-interval '1 hour',$3)")
            .bind(name).bind(f64::from(age)).bind(error).execute(&pool).await.unwrap();
    }
    let (status, snapshot) = request(&app, Some(admin), "GET", path, Value::Null, false).await;
    assert_eq!(status, StatusCode::OK, "{snapshot}");
    for (name, expected) in [
        ("ops", "unknown"),
        ("fresh", "ok"),
        ("failed", "error"),
        ("stale", "stale"),
    ] {
        let repo = snapshot["repositories"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == name)
            .unwrap();
        assert_eq!(repo["state"], expected);
    }
    assert_eq!(snapshot["queue"]["queued"], 5);
    assert_eq!(snapshot["queue"]["running"], 1);
    assert_eq!(snapshot["queue"]["expired_leases"], 1);
    assert!(snapshot["queue"]["oldest_wait_seconds"].as_f64().unwrap() >= 300.0);
    let waiting = snapshot["waiting"].as_array().unwrap();
    assert_eq!(waiting.len(), 5);
    for (name, expected) in [
        ("dependency", "dependencies"),
        ("ready", "ready"),
        ("busy", "busy"),
        ("no_runner", "no_runner"),
        ("canceling", "canceling"),
    ] {
        assert_eq!(
            waiting.iter().find(|p| p["pipeline_name"] == name).unwrap()["reason"],
            expected
        );
    }
    let linux = snapshot["runners"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["os"] == "linux")
        .unwrap();
    assert_eq!(linux["online"], 1);
    assert_eq!(linux["idle"], 0);
    assert_eq!(linux["stopped"], 1);
    assert!(snapshot["storage"]["database_bytes"].as_i64().unwrap() > 0);
    // Maintenance is separate from connectivity and never counts as idle.
    sqlx::query("UPDATE runners SET draining=true WHERE os='macos'")
        .execute(&pool)
        .await
        .unwrap();
    let paused = request(&app, Some(admin), "GET", path, Value::Null, false)
        .await
        .1;
    let macos = paused["runners"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["os"] == "macos")
        .unwrap();
    assert_eq!(macos["online"], 1);
    assert_eq!(macos["idle"], 0);
    assert_eq!(macos["draining"], 1);
    assert_eq!(
        paused["waiting"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["pipeline_name"] == "ready")
            .unwrap()["reason"],
        "draining"
    );
    sqlx::query("UPDATE runners SET draining=false WHERE os='macos'")
        .execute(&pool)
        .await
        .unwrap();
    let resumed = request(&app, Some(admin), "GET", path, Value::Null, false)
        .await
        .1;
    assert_eq!(
        resumed["waiting"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["pipeline_name"] == "ready")
            .unwrap()["reason"],
        "ready"
    );
    // Responses are bounded while totals include every accessible queued run.
    sqlx::query("INSERT INTO pipelines(id,repository_url,revision) SELECT gen_random_uuid(),'lores://127.0.0.1:41337/ops','bulk' FROM generate_series(1,110)").execute(&pool).await.unwrap();
    let snapshot = request(&app, Some(admin), "GET", path, Value::Null, false)
        .await
        .1;
    assert_eq!(snapshot["queue"]["queued"], 115);
    assert_eq!(snapshot["waiting"].as_array().unwrap().len(), 100);
    sqlx::query("UPDATE users SET role='user' WHERE id=$1")
        .bind(admin)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, Some(admin), "GET", path, Value::Null, false)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}
