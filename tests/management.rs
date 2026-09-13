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
    .await
    .1;
    assert_eq!(groups.as_array().unwrap().len(), 1);
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
    assert!(
        request(
            &app,
            Some(outsider),
            "GET",
            "/api/v1/account-groups",
            Value::Null,
            false
        )
        .await
        .1
        .as_array()
        .unwrap()
        .is_empty()
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
            Some(owner),
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
        .1
        .as_array()
        .unwrap()
        .len(),
        0
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
    .await
    .1;
    assert_eq!(views[0]["rules"], view["rules"]);
    assert_eq!(views[0]["can_manage"], false);
    assert_eq!(views[0]["view_id"], view_id);
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
        .1[0]["name"],
        "Backend"
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
        StatusCode::NOT_FOUND
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
