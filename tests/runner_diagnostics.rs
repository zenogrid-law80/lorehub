use lorehub::ci::{config::SubmitPipeline, db};
use sqlx::PgPool;
use uuid::Uuid;

async fn register(pool: &PgPool, worker: Uuid) {
    db::register_runner(
        pool,
        worker,
        "diagnostics",
        "linux",
        "x86_64",
        "0.2.21",
        false,
    )
    .await
    .unwrap();
}

async fn runner(pool: &PgPool) -> db::Runner {
    db::list_runners(pool).await.unwrap().remove(0)
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn distinguishes_presence_polling_and_explicit_shutdown(pool: PgPool) {
    let worker = Uuid::new_v4();
    register(&pool, worker).await;
    let initial = runner(&pool).await;
    assert_eq!(initial.diagnostic, "starting");
    assert!(initial.last_claim_at.is_none());
    sqlx::query("UPDATE runners SET started_at=now()-interval '2 minutes'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(runner(&pool).await.diagnostic, "poll_stalled");
    assert!(db::claim(&pool, worker).await.unwrap().is_none());
    let ready = runner(&pool).await;
    assert_eq!(ready.diagnostic, "ready");
    assert!(ready.last_claim_at.unwrap() <= ready.observed_at);
    sqlx::query("UPDATE runners SET last_claim_at=now()-interval '2 minutes'")
        .execute(&pool)
        .await
        .unwrap();
    db::touch_runner(&pool, worker).await.unwrap();
    let stale_poll = runner(&pool).await;
    assert_eq!(stale_poll.status, "online");
    assert_eq!(stale_poll.diagnostic, "poll_stalled");
    sqlx::query("UPDATE runners SET last_seen=now()-interval '20 seconds'")
        .execute(&pool)
        .await
        .unwrap();
    let offline = runner(&pool).await;
    assert_eq!(offline.status, "offline");
    assert_eq!(offline.diagnostic, "heartbeat_lost");
    assert!(offline.stopped_at.is_none());
    db::stop_runner(&pool, worker).await.unwrap();
    let stopped = runner(&pool).await;
    assert_eq!(stopped.diagnostic, "stopped");
    assert!(stopped.stopped_at.is_some());
    register(&pool, worker).await;
    let restarted = runner(&pool).await;
    assert_eq!(restarted.diagnostic, "starting");
    assert!(restarted.last_claim_at.is_none());
    assert!(restarted.stopped_at.is_none());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn polling_is_recorded_for_busy_paused_replayed_and_empty_claims(pool: PgPool) {
    let worker = Uuid::new_v4();
    register(&pool, worker).await;
    for keyed in [false, true] {
        db::set_runner_draining(&pool, worker, true).await.unwrap();
        sqlx::query("UPDATE runners SET last_claim_at=NULL")
            .execute(&pool)
            .await
            .unwrap();
        let result = if keyed {
            db::claim_with_request(&pool, worker, Uuid::new_v4()).await
        } else {
            db::claim(&pool, worker).await
        };
        assert!(result.unwrap().is_none());
        let paused = runner(&pool).await;
        assert_eq!(paused.diagnostic, "paused");
        assert!(paused.last_claim_at.is_some());
    }
    db::set_runner_draining(&pool, worker, false).await.unwrap();
    assert!(
        db::claim_with_request(&pool, worker, Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(runner(&pool).await.diagnostic, "ready");
    let submitted = db::submit(
        &pool,
        &SubmitPipeline {
            repository_url: "lores://fixture/project".into(),
            revision: "a".repeat(64),
            branch: None,
            pipeline_name: None,
        },
    )
    .await
    .unwrap();
    let request = Uuid::new_v4();
    assert_eq!(
        db::claim_with_request(&pool, worker, request)
            .await
            .unwrap()
            .unwrap()
            .id,
        submitted.id
    );
    sqlx::query("UPDATE runners SET last_claim_at=now()-interval '10 minutes'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        runner(&pool).await.diagnostic,
        "busy",
        "long jobs must not appear as stalled polling"
    );
    assert!(
        db::claim_with_request(&pool, worker, Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
    let busy = runner(&pool).await;
    assert!(busy.last_claim_at.unwrap() > busy.observed_at - chrono::Duration::seconds(5));
    db::set_runner_draining(&pool, worker, true).await.unwrap();
    assert_eq!(runner(&pool).await.diagnostic, "draining");
    sqlx::query("UPDATE runners SET last_claim_at=NULL")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        db::claim_with_request(&pool, worker, request)
            .await
            .unwrap()
            .unwrap()
            .id,
        submitted.id
    );
    assert!(runner(&pool).await.last_claim_at.is_some());
    db::finish(&pool, submitted.id, worker, "succeeded", None)
        .await
        .unwrap();
    assert_eq!(runner(&pool).await.diagnostic, "paused");
    assert!(
        db::claim_with_request(&pool, worker, request)
            .await
            .unwrap()
            .is_none()
    );
}
