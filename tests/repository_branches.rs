#![cfg(unix)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use lorehub::server::{
    api,
    auth::{AuthConfig, AuthService},
    repositories::RepositoryService,
    tokens::TokenIssuer,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::os::unix::fs::PermissionsExt;
use tower::ServiceExt;
use uuid::Uuid;

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn branch_creation_checks_access_csrf_and_inherits_manual_sync(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("lore");
    std::fs::write(&binary, include_str!("fixtures/branch-lore.sh")).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let app = api::router(
        pool.clone(),
        AuthService::new(
            pool.clone(),
            AuthConfig::new("test".into(), "test".into(), "http://127.0.0.1:8080").unwrap(),
        )
        .unwrap(),
        RepositoryService::new(binary, "lores://fixture:41337", "lores://fixture:41337").unwrap(),
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
            .bind(Sha256::digest(id.to_string().as_bytes()).to_vec()).bind(id).bind(Sha256::digest(b"csrf").to_vec()).execute(&pool).await.unwrap();
    }
    let resource = "urc-11111111111111111111111111111111";
    sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES($1,'root',$2)")
        .bind(resource)
        .bind(owner.to_string())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO repository_link_policies(root_resource_id,root_branch,link_path,auto_update,last_error) VALUES($1,'release','Source',false,'old error')")
        .bind(resource).execute(&pool).await.unwrap();
    for (user, csrf, expected) in [
        (None, true, StatusCode::UNAUTHORIZED),
        (Some(owner), false, StatusCode::FORBIDDEN),
        (Some(outsider), true, StatusCode::FORBIDDEN),
        (Some(owner), true, StatusCode::CREATED),
        (Some(owner), true, StatusCode::CONFLICT),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/repositories/root/branches")
            .header("content-type", "application/json");
        if let Some(user) = user {
            request = request.header(
                "cookie",
                format!("lorehub_session={user}; lorehub_csrf=csrf"),
            );
        }
        if csrf {
            request = request.header("x-csrf-token", "csrf");
        }
        let response = app.clone().oneshot(request.body(Body::from(json!({"name":"feature/release","from_branch":"release","expected_revision":"a".repeat(64)}).to_string())).unwrap()).await.unwrap();
        let status = response.status();
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(status, expected, "{body}");
        if expected == StatusCode::CREATED {
            assert_eq!(body["name"], "feature/release");
            let policy: (bool, Option<String>) = sqlx::query_as("SELECT auto_update,last_error FROM repository_link_policies WHERE root_resource_id=$1 AND root_branch='feature/release' AND link_path='Source'")
                .bind(resource).fetch_one(&pool).await.unwrap();
            assert_eq!(policy, (false, None));
        } else if expected != StatusCode::CONFLICT {
            assert!(!root.path().join("commands").exists());
        }
    }
}
