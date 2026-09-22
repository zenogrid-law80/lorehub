use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use lorehub::{
    ci::{
        config::{PipelineConfig, SubmitPipeline},
        db,
    },
    runner::CoordinatorClient,
    server::{
        api,
        auth::{AuthConfig, AuthService},
        repositories::RepositoryService,
        tokens::TokenIssuer,
    },
};
use sqlx::PgPool;
use uuid::Uuid;

fn issuer() -> TokenIssuer {
    TokenIssuer::from_files(
        "tests/fixtures/test-private.pem",
        "tests/fixtures/test-jwks.json",
        "http://127.0.0.1:8080",
        "zenogrid.co.kr",
    )
    .unwrap()
}

fn input() -> SubmitPipeline {
    SubmitPipeline {
        repository_url: "lores://127.0.0.1:41337/example".into(),
        revision: "a".repeat(64),
        branch: None,
        pipeline_name: None,
    }
}

async fn serve(app: Router, worker: Uuid) -> CoordinatorClient {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    CoordinatorClient::new(&format!("http://{address}"), worker, issuer()).unwrap()
}

fn router(pool: &PgPool) -> Router {
    let auth = AuthService::new(
        pool.clone(),
        AuthConfig::new("fixture".into(), "fixture".into(), "http://127.0.0.1:8080").unwrap(),
    )
    .unwrap();
    let repositories = RepositoryService::new(
        "unused-lore",
        "lores://127.0.0.1:41337",
        "lores://127.0.0.1:41337",
    )
    .unwrap();
    api::router(pool.clone(), auth, repositories, Some(issuer()))
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn draining_finishes_existing_work_and_survives_registration(pool: PgPool) {
    let worker = Uuid::new_v4();
    let request = Uuid::new_v4();
    let client = serve(router(&pool), worker).await;
    client.register("maintenance", false).await.unwrap();
    let first = db::submit(&pool, &input()).await.unwrap();
    let second = db::submit(&pool, &input()).await.unwrap();
    let claimed = db::claim_with_request(&pool, worker, request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, first.id);
    for _ in 0..2 {
        assert!(db::set_runner_draining(&pool, worker, true).await.unwrap());
    }
    let runners = db::list_runners(&pool).await.unwrap();
    assert!(runners[0].draining && runners[0].busy);
    assert_eq!(runners[0].status, "online");
    assert!(client.claim().await.unwrap().is_none());
    assert!(db::claim(&pool, worker).await.unwrap().is_none());
    // A lost acknowledgement from before drain can still reach the worker.
    let replay = db::claim_with_request(&pool, worker, request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replay.id, first.id);
    assert_eq!(replay.lease_until, claimed.lease_until);
    assert!(client.heartbeat(first.id).await.unwrap());
    client.finish(first.id, "succeeded", None).await.unwrap();
    let runners = db::list_runners(&pool).await.unwrap();
    assert!(runners[0].draining && !runners[0].busy);
    assert!(runners[0].current_pipeline_id.is_none());
    assert!(
        db::claim_with_request(&pool, worker, request)
            .await
            .unwrap()
            .is_none()
    );
    client.stop().await.unwrap();
    client
        .register("maintenance-restarted", true)
        .await
        .unwrap();
    assert!(db::list_runners(&pool).await.unwrap()[0].draining);
    assert!(client.claim().await.unwrap().is_none());
    assert!(db::claim(&pool, worker).await.unwrap().is_none());
    let other = Uuid::new_v4();
    db::register_runner(&pool, other, "available", "linux", "test", "test", false)
        .await
        .unwrap();
    assert_eq!(
        db::claim(&pool, other).await.unwrap().unwrap().id,
        second.id
    );
    let third = db::submit(&pool, &input()).await.unwrap();
    assert!(db::set_runner_draining(&pool, worker, false).await.unwrap());
    assert_eq!(client.claim().await.unwrap().unwrap().id, third.id);
    // Drain does not weaken cancellation fencing.
    db::set_runner_draining(&pool, worker, true).await.unwrap();
    db::cancel(&pool, third.id).await.unwrap();
    assert!(!client.heartbeat(third.id).await.unwrap());
    client.finish(third.id, "succeeded", None).await.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM pipelines WHERE id=$1")
        .bind(third.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "canceled");
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn maintenance_commit_fences_in_flight_legacy_and_keyed_claims(pool: PgPool) {
    let worker = Uuid::new_v4();
    db::register_runner(&pool, worker, "race", "linux", "test", "test", false)
        .await
        .unwrap();
    let queued = db::submit(&pool, &input()).await.unwrap();
    for keyed in [false, true] {
        db::set_runner_draining(&pool, worker, false).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        // Hold the same lock acquired by the maintenance API until the claim
        // is in flight, then commit. An unlocked read would miss this drain.
        sqlx::query("UPDATE runners SET draining=true WHERE id=$1")
            .bind(worker)
            .execute(&mut *tx)
            .await
            .unwrap();
        let claim_pool = pool.clone();
        let mut claim = tokio::spawn(async move {
            if keyed {
                db::claim_with_request(&claim_pool, worker, Uuid::new_v4()).await
            } else {
                db::claim(&claim_pool, worker).await
            }
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(150), &mut claim)
                .await
                .is_err()
        );
        tx.commit().await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(5), claim)
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .is_none()
        );
    }
    let status: String = sqlx::query_scalar("SELECT status FROM pipelines WHERE id=$1")
        .bind(queued.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "queued");
    assert!(
        !db::set_runner_draining(&pool, Uuid::new_v4(), true)
            .await
            .unwrap()
    );
}

#[derive(Default)]
struct LostResponses {
    claims: AtomicUsize,
    finishes: AtomicUsize,
    logs: AtomicUsize,
}

async fn lose_response(
    State(faults): State<Arc<LostResponses>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    let response = next.run(request).await;
    assert!(
        response.status().is_success(),
        "unexpected fixture response on {path}: {}",
        response.status()
    );
    let lose = if path.starts_with("/api/v1/runner/claim/") {
        faults.claims.fetch_add(1, Ordering::SeqCst) == 0
    } else if path.ends_with("/finish") {
        faults.finishes.fetch_add(1, Ordering::SeqCst) == 0
    } else if path.ends_with("/logs") {
        faults.logs.fetch_add(1, Ordering::SeqCst) == 0
    } else {
        false
    };
    // The mutation already committed; only its acknowledgement is lost.
    if lose {
        StatusCode::SERVICE_UNAVAILABLE.into_response()
    } else {
        response
    }
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn lost_assignment_and_completion_responses_do_not_duplicate_work(pool: PgPool) {
    let first = db::submit(&pool, &input()).await.unwrap();
    let second = db::submit(&pool, &input()).await.unwrap();
    let faults = Arc::new(LostResponses::default());
    let client = serve(
        router(&pool).layer(middleware::from_fn_with_state(
            faults.clone(),
            lose_response,
        )),
        Uuid::new_v4(),
    )
    .await;
    client.register("recovery", false).await.unwrap();
    let claimed = client.claim().await.unwrap().unwrap();
    assert_eq!(claimed.id, first.id);
    assert_eq!(faults.claims.load(Ordering::SeqCst), 2);
    assert!(
        client.claim().await.unwrap().is_none(),
        "another request must not allocate concurrent work"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM pipelines WHERE id = $1")
            .bind(second.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "queued"
    );
    // Append-only logs are not automatically replayed after an ambiguous failure.
    assert!(
        client
            .log(first.id, None, "system", "one record")
            .await
            .is_err()
    );
    assert_eq!(faults.logs.load(Ordering::SeqCst), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM logs WHERE pipeline_id = $1")
            .bind(first.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    client.finish(first.id, "succeeded", None).await.unwrap();
    assert_eq!(faults.finishes.load(Ordering::SeqCst), 2);
    let finished: (String, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as("SELECT status, finished_at FROM pipelines WHERE id = $1")
            .bind(first.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    client
        .finish(first.id, "failed", Some("late report"))
        .await
        .unwrap();
    let repeated: (String, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as("SELECT status, finished_at FROM pipelines WHERE id = $1")
            .bind(first.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(finished.0, "succeeded");
    assert_eq!(finished, repeated);
    assert_eq!(client.claim().await.unwrap().unwrap().id, second.id);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn concurrent_claim_retries_are_fenced_after_completion_expiry_and_cancel(pool: PgPool) {
    let first = db::submit(&pool, &input()).await.unwrap();
    let second = db::submit(&pool, &input()).await.unwrap();
    let worker = Uuid::new_v4();
    let request = Uuid::new_v4();
    let (a, b) = tokio::join!(
        db::claim_with_request(&pool, worker, request),
        db::claim_with_request(&pool, worker, request)
    );
    let (a, b) = (a.unwrap().unwrap(), b.unwrap().unwrap());
    assert_eq!(a.id, first.id);
    assert_eq!(a.id, b.id);
    assert_eq!(a.started_at, b.started_at);
    assert_eq!(
        a.lease_until, b.lease_until,
        "replay does not extend the lease"
    );
    assert!(
        db::claim_with_request(&pool, worker, Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
    // Identities are scoped to authenticated Runners, not supplied worker IDs.
    let other = Uuid::new_v4();
    assert_eq!(
        db::claim_with_request(&pool, other, request)
            .await
            .unwrap()
            .unwrap()
            .id,
        second.id
    );
    db::finish(&pool, first.id, worker, "succeeded", None)
        .await
        .unwrap();
    let third = db::submit(&pool, &input()).await.unwrap();
    assert!(
        db::claim_with_request(&pool, worker, request)
            .await
            .unwrap()
            .is_none()
    );
    let expired_request = Uuid::new_v4();
    assert_eq!(
        db::claim_with_request(&pool, worker, expired_request)
            .await
            .unwrap()
            .unwrap()
            .id,
        third.id
    );
    sqlx::query("UPDATE pipelines SET lease_until = now() - interval '1 second' WHERE id = $1")
        .bind(third.id)
        .execute(&pool)
        .await
        .unwrap();
    let fourth = db::submit(&pool, &input()).await.unwrap();
    assert!(
        db::claim_with_request(&pool, worker, expired_request)
            .await
            .unwrap()
            .is_none()
    );
    let cancel_request = Uuid::new_v4();
    assert_eq!(
        db::claim_with_request(&pool, worker, cancel_request)
            .await
            .unwrap()
            .unwrap()
            .id,
        fourth.id
    );
    db::cancel(&pool, fourth.id).await.unwrap();
    assert!(
        db::claim_with_request(&pool, worker, cancel_request)
            .await
            .unwrap()
            .is_none()
    );
    db::finish(&pool, fourth.id, worker, "succeeded", None)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM pipelines WHERE id = $1")
            .bind(fourth.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "canceled"
    );
}

#[tokio::test]
async fn permanent_errors_and_non_idempotent_requests_are_not_retried() {
    let requests = Arc::new(AtomicUsize::new(0));
    let app = Router::new().fallback({
        let requests = requests.clone();
        move |request: Request| {
            requests.fetch_add(1, Ordering::SeqCst);
            async move {
                if request.uri().path().ends_with("register") {
                    StatusCode::FORBIDDEN
                } else if request.uri().path().contains("/claim/") {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            }
        }
    });
    let client = serve(app, Uuid::new_v4()).await;
    assert!(client.register("denied", false).await.is_err());
    assert!(
        client.claim().await.is_err(),
        "old coordinator must fail rather than silently use the non-idempotent route"
    );
    assert!(
        client
            .log(Uuid::new_v4(), None, "system", "once")
            .await
            .is_err()
    );
    let config = PipelineConfig::parse(
        "stages=['test']\n[[jobs]]\nname='test'\nstage='test'\nscript=['echo test']",
    )
    .unwrap();
    assert!(client.create_jobs(Uuid::new_v4(), &config).await.is_err());
    assert_eq!(requests.load(Ordering::SeqCst), 4);
}

#[cfg(unix)]
#[derive(Default)]
struct Reconnect {
    registrations: AtomicUsize,
    claims: Mutex<Vec<String>>,
    stop: tokio_util::sync::CancellationToken,
}

#[tokio::test]
async fn disconnected_claim_response_reuses_the_request_identity() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut paths = Vec::new();
        for attempt in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            while !data.windows(4).any(|window| window == b"\r\n\r\n") {
                let mut chunk = [0; 1024];
                let size = socket.read(&mut chunk).await.unwrap();
                assert!(size > 0);
                data.extend_from_slice(&chunk[..size]);
            }
            paths.push(
                String::from_utf8(data)
                    .unwrap()
                    .lines()
                    .next()
                    .unwrap()
                    .to_owned(),
            );
            if attempt == 1 {
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnull").await.unwrap();
            }
            // The first connection closes without acknowledging the request.
        }
        paths
    });
    let client =
        CoordinatorClient::new(&format!("http://{address}"), Uuid::new_v4(), issuer()).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(10), client.claim())
            .await
            .unwrap()
            .unwrap()
            .is_none()
    );
    let paths = server.await.unwrap();
    assert_eq!(paths[0], paths[1]);
    assert!(paths[0].starts_with("POST /api/v1/runner/claim/"));
}

#[cfg(unix)]
#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn worker_executes_script_once_when_assignment_and_finish_acknowledgements_are_lost(
    pool: PgPool,
) {
    use lorehub::runner::Worker;
    use std::os::unix::fs::PermissionsExt;
    let first = db::submit(&pool, &input()).await.unwrap();
    let second = db::submit(&pool, &input()).await.unwrap();
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("lore-fixture");
    let marker = root.path().join("executions");
    let script = format!(
        r#"#!/bin/sh
set -eu
if [ "$1" = '--version' ]; then echo fixture; exit 0; fi
mkdir -p "$8"
cat > "$8/.lore-ci.toml" <<'CONFIG'
stages = ['test']
[[jobs]]
name = 'once'
stage = 'test'
script = ["echo executed >> '{}'", "echo done"]
CONFIG
"#,
        marker.display()
    );
    std::fs::write(&bin, script).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
    let faults = Arc::new(LostResponses::default());
    // Logs must succeed here; their no-retry behavior is covered separately.
    faults.logs.store(1, Ordering::SeqCst);
    let id = Uuid::new_v4();
    let worker = Worker {
        coordinator: serve(
            router(&pool).layer(middleware::from_fn_with_state(
                faults.clone(),
                lose_response,
            )),
            id,
        )
        .await,
        id,
        work_dir: root.path().join("work"),
        lore_bin: bin.to_str().unwrap().into(),
        token_issuer: None,
    };
    tokio::time::timeout(
        Duration::from_secs(20),
        worker.run(tokio_util::sync::CancellationToken::new(), true),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "executed\n");
    assert_eq!(faults.claims.load(Ordering::SeqCst), 2);
    assert_eq!(faults.finishes.load(Ordering::SeqCst), 2);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM pipelines WHERE id = $1")
            .bind(first.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "succeeded"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM pipelines WHERE id = $1")
            .bind(second.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "queued"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM jobs WHERE pipeline_id = $1")
            .bind(first.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[cfg(unix)]
#[tokio::test]
async fn worker_recovers_after_retry_exhaustion_and_keeps_the_claim_identity() {
    use lorehub::runner::Worker;
    use std::os::unix::fs::PermissionsExt;
    let state = Arc::new(Reconnect::default());
    let app = Router::new()
        .route(
            "/api/v1/runner/register",
            post(|State(state): State<Arc<Reconnect>>| async move {
                if state.registrations.fetch_add(1, Ordering::SeqCst) < 4 {
                    StatusCode::SERVICE_UNAVAILABLE.into_response()
                } else {
                    Json(serde_json::json!({})).into_response()
                }
            }),
        )
        .route(
            "/api/v1/runner/claim/{request}",
            post(
                |State(state): State<Arc<Reconnect>>, request: Request| async move {
                    let failed = {
                        let mut claims = state.claims.lock().unwrap();
                        claims.push(request.uri().path().to_owned());
                        claims.len() <= 4
                    };
                    if failed {
                        StatusCode::SERVICE_UNAVAILABLE.into_response()
                    } else {
                        state.stop.cancel();
                        Json(serde_json::Value::Null).into_response()
                    }
                },
            ),
        )
        .route(
            "/api/v1/runner/touch",
            post(|| async { Json(serde_json::json!({"active":true})) }),
        )
        .route(
            "/api/v1/runner/stop",
            post(|| async { StatusCode::NO_CONTENT }),
        )
        .with_state(state.clone());
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("lore-fixture");
    std::fs::write(&bin, "#!/bin/sh\necho fixture-version\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
    let id = Uuid::new_v4();
    let worker = Worker {
        coordinator: serve(app, id).await,
        id,
        work_dir: root.path().join("work"),
        lore_bin: bin.to_str().unwrap().into(),
        token_issuer: None,
    };
    tokio::time::timeout(
        Duration::from_secs(25),
        worker.run(state.stop.clone(), false),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(state.registrations.load(Ordering::SeqCst), 5);
    let claims = state.claims.lock().unwrap();
    assert_eq!(claims.len(), 5);
    assert!(claims.iter().all(|path| path == &claims[0]));
}
