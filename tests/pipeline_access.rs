use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use lorehub::{
    ci::db,
    server::{
        api,
        auth::{AuthConfig, AuthService},
        repositories::RepositoryService,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PRIMARY: &str = "lores://primary.example:41337";
const LOCAL: &str = "lores://local.example:41338";

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn history_search_filters_all_runs_before_pagination_and_keeps_live_permissions(
    pool: PgPool,
) {
    let f = fixture(&pool).await;
    let older = run(&pool, &format!("{PRIMARY}/main"), "Archive_100%", 2000).await;
    sqlx::query("UPDATE pipelines SET branch='release',status='failed' WHERE id=$1")
        .bind(older)
        .execute(&pool)
        .await
        .unwrap();
    let newer = run(&pool, &format!("{PRIMARY}/main"), "Archive_100%", 1000).await;
    sqlx::query("UPDATE pipelines SET branch='release',status='failed' WHERE id=$1")
        .bind(newer)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO pipelines(id,repository_url,revision,branch,pipeline_name) SELECT gen_random_uuid(),$1,'recent','main','build' FROM generate_series(1,120)")
        .bind(format!("{PRIMARY}/main")).execute(&pool).await.unwrap();
    let filters = "branch=release&pipeline_name=Archive_100%25&status=failed&q=ARCHIVE_100%25";
    let path = format!("/api/v1/pipeline-history?limit=1&{filters}");
    let (status, first) = request(&f.app, Some(f.member), "GET", &path).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["pipelines"][0]["id"], json!(newer));
    assert_eq!(first["next_before"], json!(newer));
    // A completed/canceled cursor may change state between pages. It remains an
    // ordering boundary rather than requiring its current status to match.
    sqlx::query("UPDATE pipelines SET status='succeeded' WHERE id=$1")
        .bind(newer)
        .execute(&pool)
        .await
        .unwrap();
    let (status, second) = request(
        &f.app,
        Some(f.member),
        "GET",
        &format!("{path}&before={newer}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["pipelines"][0]["id"], json!(older));
    assert!(second["next_before"].is_null());
    for (query, count) in [
        ("status=active", 122),
        ("status=finished", 2),
        ("status=failed", 1),
        ("q=%25", 2),
        ("q=%5F", 2),
        ("q=%27%20OR%201%3D1--", 0),
        ("q=hidden", 0),
        ("branch=release&pipeline_name=build", 0),
    ] {
        let (status, body) = request(
            &f.app,
            Some(f.member),
            "GET",
            &format!("/api/v1/pipeline-history?limit=500&{query}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["pipelines"].as_array().unwrap().len(),
            count,
            "{query}: {body}"
        );
    }
    for query in [
        "status=unknown".to_owned(),
        "q=%00".to_owned(),
        format!("q={}", "a".repeat(257)),
        format!("branch={}", "a".repeat(513)),
    ] {
        assert_eq!(
            request(
                &f.app,
                Some(f.member),
                "GET",
                &format!("/api/v1/pipeline-history?{query}")
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    sqlx::query("DELETE FROM repository_account_group_access WHERE group_id=$1")
        .bind(f.group)
        .execute(&pool)
        .await
        .unwrap();
    let (_, body) = request(&f.app, Some(f.member), "GET", &path).await;
    assert_eq!(body["pipelines"], json!([]));
    assert_eq!(
        request(
            &f.app,
            Some(f.member),
            "GET",
            &format!("{path}&before={newer}")
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn history_branch_search_matches_displayed_legacy_branch_and_literal_unicode(pool: PgPool) {
    let f = fixture(&pool).await;
    let legacy = "b".repeat(32);
    sqlx::query("UPDATE pipelines SET branch=$1 WHERE id=$2")
        .bind(&legacy)
        .bind(f.main)
        .execute(&pool)
        .await
        .unwrap();
    let path = format!("/api/v1/pipeline-history?repository_url={PRIMARY}/main&branch=main&q=main");
    let (status, body) = request(&f.app, Some(f.owner), "GET", &path).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["pipelines"][0]["id"], json!(f.main));
    assert_eq!(body["pipelines"][0]["branch"], "main");
    sqlx::query("UPDATE ci_pipeline_routes SET branch='release', revision='different' WHERE resource_id='main'").execute(&pool).await.unwrap();
    let (_, body) = request(
        &f.app,
        Some(f.owner),
        "GET",
        "/api/v1/pipeline-history?branch=release",
    )
    .await;
    assert_eq!(body["pipelines"][0]["branch"], "release");
    // Ambiguous route names must not invent a displayed branch.
    sqlx::query("INSERT INTO ci_pipeline_routes(resource_id,repository_url,branch,revision,pipeline_name,runner_os,trigger_patterns,working_directory,graph_definition) VALUES('main',$1,'other','different','build','linux','{}','.','{\"stages\":[]}' )")
        .bind(format!("{PRIMARY}/main")).execute(&pool).await.unwrap();
    let (_, body) = request(
        &f.app,
        Some(f.owner),
        "GET",
        "/api/v1/pipeline-history?branch=release",
    )
    .await;
    assert_eq!(body["pipelines"], json!([]));
    let (_, body) = request(
        &f.app,
        Some(f.owner),
        "GET",
        &format!("/api/v1/pipeline-history?branch={legacy}"),
    )
    .await;
    assert_eq!(body["pipelines"][0]["branch"], legacy);
    sqlx::query("UPDATE pipelines SET pipeline_name='한글 검색' WHERE id=$1")
        .bind(f.main)
        .execute(&pool)
        .await
        .unwrap();
    let (_, body) = request(
        &f.app,
        Some(f.owner),
        "GET",
        "/api/v1/pipeline-history?q=%ED%95%9C%EA%B8%80",
    )
    .await;
    assert_eq!(body["pipelines"][0]["id"], json!(f.main));
}

fn app(pool: &PgPool, local: bool) -> Router {
    let auth = AuthService::new(
        pool.clone(),
        AuthConfig::new("test".into(), "test".into(), "http://127.0.0.1:8080").unwrap(),
    )
    .unwrap();
    let mut repositories = RepositoryService::new("/usr/bin/false", PRIMARY, PRIMARY).unwrap();
    if local {
        repositories = repositories.with_local_backend(LOCAL, LOCAL).unwrap();
    }
    api::router(pool.clone(), auth, repositories, None)
}

async fn user(pool: &PgPool, role: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,google_sub,email,role) VALUES($1,$2,$3,$4)")
        .bind(id)
        .bind(id.to_string())
        .bind(format!("{id}@zenogrid.co.kr"))
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions(token_hash,user_id,csrf_hash,expires_at) VALUES($1,$2,$3,now()+interval '1 hour')")
        .bind(Sha256::digest(id.to_string().as_bytes()).to_vec()).bind(id)
        .bind(Sha256::digest(b"test-csrf").to_vec()).execute(pool).await.unwrap();
    id
}

async fn request(
    app: &Router,
    user: Option<Uuid>,
    method: &str,
    path: &str,
) -> (StatusCode, Value) {
    let mut request = Request::builder().uri(path).method(method);
    if let Some(user) = user {
        request = request
            .header(
                "cookie",
                format!("lorehub_session={user}; lorehub_csrf=test-csrf"),
            )
            .header("x-csrf-token", "test-csrf");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

async fn run(pool: &PgPool, url: &str, name: &str, seconds_ago: i32) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO pipelines(id,repository_url,revision,branch,pipeline_name,created_at,graph_definition) VALUES($1,$2,$3,'main',$4,now()-make_interval(secs => $5::double precision),'{\"stages\":[]}')")
        .bind(id).bind(url).bind("a".repeat(64)).bind(name).bind(seconds_ago as f64)
        .execute(pool).await.unwrap();
    let job = Uuid::new_v4();
    sqlx::query("INSERT INTO jobs(id,pipeline_id,position,name,stage) VALUES($1,$2,0,'private-job','build')")
        .bind(job).bind(id).execute(pool).await.unwrap();
    db::log(pool, id, Some(job), "stdout", "private build output")
        .await
        .unwrap();
    id
}

struct Fixture {
    app: Router,
    admin: Uuid,
    owner: Uuid,
    member: Uuid,
    group_owner: Uuid,
    outsider: Uuid,
    group: Uuid,
    main: Uuid,
    local: Uuid,
    hidden: Uuid,
}

async fn fixture(pool: &PgPool) -> Fixture {
    // Create the administrator first, so initial-admin assignment cannot change the matrix.
    let admin = user(pool, "admin").await;
    let owner = user(pool, "user").await;
    let member = user(pool, "user").await;
    let group_owner = user(pool, "user").await;
    let outsider = user(pool, "user").await;
    for (name, subject, backend, prefix) in [
        ("main", owner, "dynamodb_s3", PRIMARY),
        ("local", owner, "local_file", LOCAL),
        ("hidden", outsider, "dynamodb_s3", PRIMARY),
    ] {
        sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject,storage_backend,created_at) VALUES($1,$1,$2,$3,now()-interval '1 day')")
            .bind(name).bind(subject.to_string()).bind(backend).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO ci_pipeline_routes(resource_id,repository_url,branch,revision,pipeline_name,runner_os,trigger_patterns,working_directory,graph_definition) VALUES($1,$2,'main',$3,'build','linux','{}','.','{\"stages\":[]}')")
            .bind(name).bind(format!("{prefix}/{name}")).bind("a".repeat(64))
            .execute(pool).await.unwrap();
    }
    let group = Uuid::new_v4();
    sqlx::query("INSERT INTO account_groups(id,name,owner_id) VALUES($1,'Readers',$2)")
        .bind(group)
        .bind(group_owner)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO account_group_members(group_id,user_id) VALUES($1,$2)")
        .bind(group)
        .bind(member)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO repository_account_group_access(resource_id,group_id) VALUES('main',$1),('local',$1)")
        .bind(group).execute(pool).await.unwrap();
    let main = run(pool, &format!("{PRIMARY}/main"), "build", 30).await;
    let local = run(pool, &format!("{LOCAL}/local"), "build", 20).await;
    let hidden = run(pool, &format!("{PRIMARY}/hidden"), "build", 10).await;
    Fixture {
        app: app(pool, true),
        admin,
        owner,
        member,
        group_owner,
        outsider,
        group,
        main,
        local,
        hidden,
    }
}

async fn assert_reads(app: &Router, user: Uuid, id: Uuid, allowed: bool) {
    for suffix in ["", "/logs", "/insights"] {
        let path = format!("/api/v1/pipelines/{id}{suffix}");
        let (status, body) = request(app, Some(user), "GET", &path).await;
        assert_eq!(
            status,
            if allowed {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            },
            "{path}: {body}"
        );
        if !allowed {
            assert_eq!(body, json!({"error":"pipeline not found"}));
        }
    }
}

async fn listed_ids(app: &Router, user: Uuid) -> Vec<String> {
    let (status, body) = request(app, Some(user), "GET", "/api/v1/pipelines").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array()
        .unwrap()
        .iter()
        .map(|run| run["id"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn execution_reads_follow_owner_group_and_admin_repository_permissions(pool: PgPool) {
    let f = fixture(&pool).await;
    for (user, expected) in [
        (f.owner, vec![f.local, f.main]),
        (f.member, vec![f.local, f.main]),
        (f.group_owner, vec![f.local, f.main]),
        (f.outsider, vec![f.hidden]),
        (f.admin, vec![f.hidden, f.local, f.main]),
    ] {
        assert_eq!(
            listed_ids(&f.app, user).await,
            expected.iter().map(Uuid::to_string).collect::<Vec<_>>()
        );
        let (status, history) =
            request(&f.app, Some(user), "GET", "/api/v1/pipeline-history").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            history["pipelines"].as_array().unwrap().len(),
            expected.len()
        );
        assert!(history["next_before"].is_null());
        let (status, graphs) = request(&f.app, Some(user), "GET", "/api/v1/pipeline-graphs").await;
        assert_eq!(status, StatusCode::OK, "{graphs}");
        let actual: std::collections::HashSet<_> = graphs
            .as_array()
            .unwrap()
            .iter()
            .map(|graph| graph["latest_pipeline_id"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(actual, expected.iter().map(Uuid::to_string).collect());
        for id in [f.main, f.local, f.hidden] {
            assert_reads(&f.app, user, id, expected.contains(&id)).await;
        }
    }
    for path in [
        "/api/v1/pipelines".into(),
        "/api/v1/pipeline-history".into(),
        "/api/v1/pipeline-graphs".into(),
        format!("/api/v1/pipelines/{}", f.main),
        format!("/api/v1/pipelines/{}/logs", f.main),
        format!("/api/v1/pipelines/{}/insights", f.main),
    ] {
        assert_eq!(
            request(&f.app, None, "GET", &path).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_reads(&f.app, f.owner, Uuid::new_v4(), false).await;
    // A runner remains organization-visible, but must not disclose a private run ID.
    let runner = Uuid::new_v4();
    db::register_runner(&pool, runner, "runner", "linux", "x86_64", "test", false)
        .await
        .unwrap();
    sqlx::query("UPDATE pipelines SET status='running',worker_id=$1,started_at=now() WHERE id=$2")
        .bind(runner)
        .bind(f.hidden)
        .execute(&pool)
        .await
        .unwrap();
    for (user, expected) in [(f.owner, Value::Null), (f.outsider, json!(f.hidden))] {
        let (status, runners) = request(&f.app, Some(user), "GET", "/api/v1/runners").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(runners[0]["current_pipeline_id"], expected);
    }
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn revocation_applies_to_existing_sessions_and_cancel_responses(pool: PgPool) {
    let f = fixture(&pool).await;
    // Submitting a run must not preserve read or cancellation access after revocation.
    sqlx::query("UPDATE pipelines SET submitted_by=$1 WHERE id=$2")
        .bind(f.member)
        .bind(f.main)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM account_group_members WHERE group_id=$1 AND user_id=$2")
        .bind(f.group)
        .bind(f.member)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET role='user' WHERE id=$1")
        .bind(f.admin)
        .execute(&pool)
        .await
        .unwrap();
    for user in [f.member, f.admin] {
        assert!(listed_ids(&f.app, user).await.is_empty());
        assert_reads(&f.app, user, f.main, false).await;
        assert_eq!(
            request(&f.app, Some(user), "GET", "/api/v1/pipeline-graphs").await,
            (StatusCode::OK, json!([]))
        );
        assert_eq!(
            request(
                &f.app,
                Some(user),
                "POST",
                &format!("/api/v1/pipelines/{}/cancel", f.main)
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    let canceled: bool = sqlx::query_scalar("SELECT cancel_requested FROM pipelines WHERE id=$1")
        .bind(f.main)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!canceled);
    assert_reads(&f.app, f.group_owner, f.main, true).await;
    sqlx::query("DELETE FROM repository_account_group_access WHERE group_id=$1")
        .bind(f.group)
        .execute(&pool)
        .await
        .unwrap();
    assert_reads(&f.app, f.group_owner, f.main, false).await;
    assert!(listed_ids(&f.app, f.group_owner).await.is_empty());
    assert_reads(&f.app, f.owner, f.main, true).await;
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn pagination_filters_before_limit_and_rejects_inaccessible_cursors(pool: PgPool) {
    let f = fixture(&pool).await;
    let third = run(&pool, &format!("{PRIMARY}/main"), "test", 15).await;
    let mut cursor = None;
    for (index, id) in [third, f.local, f.main].into_iter().enumerate() {
        let path = format!(
            "/api/v1/pipeline-history?limit=1{}",
            cursor.map(|id| format!("&before={id}")).unwrap_or_default()
        );
        let (status, body) = request(&f.app, Some(f.member), "GET", &path).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["pipelines"].as_array().unwrap().len(), 1);
        assert_eq!(body["pipelines"][0]["id"], id.to_string());
        if index == 2 {
            assert!(body["next_before"].is_null());
        } else {
            assert_eq!(body["next_before"], id.to_string());
        }
        cursor = Some(id);
    }
    for id in [f.hidden, Uuid::new_v4()] {
        assert_eq!(
            request(
                &f.app,
                Some(f.member),
                "GET",
                &format!("/api/v1/pipeline-history?limit=1&before={id}")
            )
            .await,
            (
                StatusCode::BAD_REQUEST,
                json!({"error":"unknown pipeline cursor"})
            )
        );
    }
    for limit in [0, 501] {
        assert_eq!(
            request(
                &f.app,
                Some(f.owner),
                "GET",
                &format!("/api/v1/pipeline-history?limit={limit}")
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn repository_recreation_cannot_expose_old_runs_or_analysis_relations(pool: PgPool) {
    let f = fixture(&pool).await;
    let url = format!("{PRIMARY}/main");
    let upstream = run(&pool, &url, "tools", 40).await;
    let downstream = run(&pool, &url, "deploy", 10).await;
    sqlx::query("UPDATE pipelines SET pipeline_needs=ARRAY['build'] WHERE id=$1")
        .bind(downstream)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE pipelines SET status='succeeded',finished_at=now()-interval '5 seconds' WHERE id=$1")
        .bind(f.main).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM lore_resources WHERE resource_id='main'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES('recreated','main',$1)",
    )
    .bind(f.outsider.to_string())
    .execute(&pool)
    .await
    .unwrap();
    let current = run(&pool, &url, "build", 0).await;
    sqlx::query("UPDATE pipelines SET pipeline_needs=ARRAY['tools'] WHERE id=$1")
        .bind(current)
        .execute(&pool)
        .await
        .unwrap();
    for user in [f.outsider, f.admin] {
        for id in [f.main, upstream, downstream] {
            assert_reads(&f.app, user, id, false).await;
        }
        let (status, insights) = request(
            &f.app,
            Some(user),
            "GET",
            &format!("/api/v1/pipelines/{current}/insights"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{insights}");
        assert!(insights["previous"].is_null());
        assert!(insights["upstream"][0]["run_id"].is_null());
        assert_eq!(insights["downstream"], json!([]));
        assert!(!listed_ids(&f.app, user).await.contains(&f.main.to_string()));
    }
    assert_reads(&f.app, f.owner, current, false).await;
    assert_reads(&f.app, f.member, current, false).await;
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
async fn url_origin_backend_and_missing_repository_fail_closed(pool: PgPool) {
    let f = fixture(&pool).await;
    for url in [
        "lores://untrusted.example:41337/main",
        "lores://primary.example:41337/nested/main",
        "lores://primary.example:41338/main",
        "lores://local.example:41338/main",
        "lores://primary.example:41337/local",
        "lores://primary.example:41337/missing",
    ] {
        let id = run(&pool, url, "build", 0).await;
        for user in [f.owner, f.admin] {
            assert_reads(&f.app, user, id, false).await;
        }
    }
    assert_eq!(
        listed_ids(&f.app, f.owner).await,
        vec![f.local.to_string(), f.main.to_string()]
    );
    let no_local = app(&pool, false);
    assert_reads(&no_local, f.owner, f.local, false).await;
    assert_eq!(
        listed_ids(&no_local, f.owner).await,
        vec![f.main.to_string()]
    );
    let (_, graphs) = request(&no_local, Some(f.owner), "GET", "/api/v1/pipeline-graphs").await;
    assert_eq!(graphs.as_array().unwrap().len(), 1);
    // Even a route row must agree with both the authorized resource ID and URL.
    sqlx::query("UPDATE ci_pipeline_routes SET repository_url=$1 WHERE resource_id='main'")
        .bind(format!("{PRIMARY}/hidden"))
        .execute(&pool)
        .await
        .unwrap();
    let (_, graphs) = request(&no_local, Some(f.owner), "GET", "/api/v1/pipeline-graphs").await;
    assert_eq!(graphs, json!([]));
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn repository_history_filters_before_paging_and_rejects_other_repository_cursors(
    pool: PgPool,
) {
    let f = fixture(&pool).await;
    let older = run(&pool, &format!("{PRIMARY}/main"), "older", 40).await;
    sqlx::query("INSERT INTO pipelines(id,repository_url,revision,created_at) SELECT gen_random_uuid(),$1,'recent',now() FROM generate_series(1,120)")
        .bind(format!("{LOCAL}/local")).execute(&pool).await.unwrap();
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("repository_url", &format!("{PRIMARY}/main"))
        .finish();
    let path = format!("/api/v1/pipeline-history?limit=1&{query}");
    let (status, page) = request(&f.app, Some(f.owner), "GET", &path).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["pipelines"][0]["id"], f.main.to_string());
    assert_eq!(page["next_before"], f.main.to_string());
    let (status, page) = request(
        &f.app,
        Some(f.owner),
        "GET",
        &format!("{path}&before={}", f.main),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["pipelines"][0]["id"], older.to_string());
    assert!(page["next_before"].is_null());
    assert_eq!(
        request(
            &f.app,
            Some(f.owner),
            "GET",
            &format!("{path}&before={}", f.local)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    for target in [
        format!("{PRIMARY}/hidden"),
        "lores://different.example/main".into(),
        format!("{PRIMARY}/unknown"),
    ] {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("repository_url", &target)
            .finish();
        let (status, page) = request(
            &f.app,
            Some(f.owner),
            "GET",
            &format!("/api/v1/pipeline-history?{query}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["pipelines"], json!([]));
        assert!(page["next_before"].is_null());
    }
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn repository_filters_preserve_graph_and_history_permissions_after_revocation(pool: PgPool) {
    let f = fixture(&pool).await;
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("repository_url", &format!("{PRIMARY}/main"))
        .finish();
    for endpoint in ["pipeline-history", "pipeline-graphs"] {
        let (status, data) = request(
            &f.app,
            Some(f.member),
            "GET",
            &format!("/api/v1/{endpoint}?{query}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{data}");
        let rows = if endpoint == "pipeline-history" {
            &data["pipelines"]
        } else {
            &data
        };
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["repository_url"], format!("{PRIMARY}/main"));
        assert_eq!(
            request(
                &f.app,
                Some(f.member),
                "GET",
                &format!("/api/v1/{endpoint}?repository_url=")
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    sqlx::query(
        "DELETE FROM repository_account_group_access WHERE resource_id='main' AND group_id=$1",
    )
    .bind(f.group)
    .execute(&pool)
    .await
    .unwrap();
    for endpoint in ["pipeline-history", "pipeline-graphs"] {
        let (status, data) = request(
            &f.app,
            Some(f.member),
            "GET",
            &format!("/api/v1/{endpoint}?{query}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            if endpoint == "pipeline-history" {
                &data["pipelines"]
            } else {
                &data
            },
            &json!([])
        );
    }
}
