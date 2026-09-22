use lorehub::ci::{db, retention::prune_logs};
use sqlx::PgPool;
use uuid::Uuid;

async fn pipeline(pool: &PgPool, status: &str, age: Option<i32>) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO pipelines (id, repository_url, revision, status, finished_at) \
         VALUES ($1, 'lores://test/repo', $1::text, $2, now() - make_interval(days => $3))",
    )
    .bind(id)
    .bind(status)
    .bind(age)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn log(pool: &PgPool, id: Uuid) {
    db::log(pool, id, None, "stdout", "한글\n").await.unwrap();
}

async fn count(pool: &PgPool, id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM logs WHERE pipeline_id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn preview_and_batches_preserve_active_recent_and_incomplete_runs(pool: PgPool) {
    let mut expired = Vec::new();
    for status in ["succeeded", "failed", "canceled"] {
        let id = pipeline(&pool, status, Some(31)).await;
        log(&pool, id).await;
        log(&pool, id).await;
        expired.push(id);
    }
    let mut protected = Vec::new();
    for (status, age) in [
        ("queued", Some(40)),
        ("running", Some(40)),
        ("succeeded", Some(29)),
        ("failed", None),
    ] {
        let id = pipeline(&pool, status, age).await;
        log(&pool, id).await;
        protected.push(id);
    }
    let empty = pipeline(&pool, "succeeded", Some(40)).await;
    let preview = prune_logs(&pool, 30, 1, false).await.unwrap();
    assert!(!preview.applied);
    assert_eq!(preview.eligible.pipelines, 3);
    assert_eq!(preview.eligible.log_rows, 6);
    assert_eq!(preview.eligible.content_bytes, 42);
    assert_eq!(preview.deleted.log_rows, 0);
    for id in &expired {
        assert_eq!(count(&pool, *id).await, 2);
    }

    let first = prune_logs(&pool, 30, 1, true).await.unwrap();
    assert_eq!(first.deleted.log_rows, 1);
    assert_eq!(first.deleted.content_bytes, 7);
    let second = prune_logs(&pool, 30, 100, true).await.unwrap();
    assert_eq!(second.deleted.log_rows, 5);
    assert_eq!(
        prune_logs(&pool, 30, 100, true)
            .await
            .unwrap()
            .deleted
            .log_rows,
        0
    );
    for id in expired {
        assert_eq!(count(&pool, id).await, 0);
        let p: db::Pipeline = sqlx::query_as("SELECT * FROM pipelines WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(p.logs_pruned_at.is_some());
    }
    protected.push(empty);
    for id in protected {
        assert_eq!(count(&pool, id).await, i64::from(id != empty));
        let marker: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT logs_pruned_at FROM pipelines WHERE id=$1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(marker.is_none());
    }
    assert!(prune_logs(&pool, 0, 10, true).await.is_err());
    assert!(prune_logs(&pool, 30, 10001, true).await.is_err());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn retention_keeps_dependency_failure_and_push_deduplication(pool: PgPool) {
    let upstream = pipeline(&pool, "failed", Some(90)).await;
    sqlx::query("UPDATE pipelines SET pipeline_name='build', previous_revision='old', branch='main' WHERE id=$1")
        .bind(upstream).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO jobs (id,pipeline_id,position,name,stage,status) VALUES ($1,$2,0,'build','build','failed')")
        .bind(Uuid::new_v4()).bind(upstream).execute(&pool).await.unwrap();
    log(&pool, upstream).await;
    let dependent = pipeline(&pool, "queued", None).await;
    sqlx::query("UPDATE pipelines SET pipeline_needs=ARRAY['build'], revision=$2, branch='main' WHERE id=$1")
        .bind(dependent).bind(upstream.to_string()).execute(&pool).await.unwrap();
    prune_logs(&pool, 30, 100, true).await.unwrap();
    assert!(db::claim(&pool, Uuid::new_v4()).await.unwrap().is_none());
    let status: String = sqlx::query_scalar("SELECT status FROM pipelines WHERE id=$1")
        .bind(dependent)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE pipeline_id=$1")
        .bind(upstream)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1);
    let duplicate = sqlx::query("INSERT INTO pipelines (id,repository_url,revision,pipeline_name,previous_revision,branch) VALUES ($1,'lores://test/repo',$2,'build','old','main')")
        .bind(Uuid::new_v4()).bind(upstream.to_string()).execute(&pool).await.unwrap_err();
    assert_eq!(
        duplicate.as_database_error().unwrap().constraint(),
        Some("pipelines_push_once")
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn concurrent_maintenance_skips_locked_logs_and_rolls_back_failed_batches(pool: PgPool) {
    let id = pipeline(&pool, "succeeded", Some(31)).await;
    log(&pool, id).await;
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM logs WHERE pipeline_id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *lock)
        .await
        .unwrap();
    assert_eq!(
        prune_logs(&pool, 30, 100, true)
            .await
            .unwrap()
            .deleted
            .log_rows,
        0
    );
    lock.rollback().await.unwrap();

    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(714275036921)")
        .execute(&mut *lock)
        .await
        .unwrap();
    assert!(
        prune_logs(&pool, 30, 100, true)
            .await
            .unwrap_err()
            .to_string()
            .contains("another log retention")
    );
    assert_eq!(
        prune_logs(&pool, 30, 100, false)
            .await
            .unwrap()
            .eligible
            .log_rows,
        1
    );
    lock.rollback().await.unwrap();

    sqlx::raw_sql("CREATE FUNCTION reject_prune() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test failure'; END $$; CREATE TRIGGER reject_prune BEFORE UPDATE OF logs_pruned_at ON pipelines FOR EACH ROW EXECUTE FUNCTION reject_prune();")
        .execute(&pool).await.unwrap();
    assert!(prune_logs(&pool, 30, 100, true).await.is_err());
    assert_eq!(count(&pool, id).await, 1);
    sqlx::query("DROP TRIGGER reject_prune ON pipelines")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        prune_logs(&pool, 30, 100, true)
            .await
            .unwrap()
            .deleted
            .log_rows,
        1
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn cli_defaults_to_preview_and_requires_explicit_apply(pool: PgPool) {
    use sqlx::ConnectOptions;
    let id = pipeline(&pool, "succeeded", Some(31)).await;
    log(&pool, id).await;
    let run = |apply: bool| {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_lorehub"));
        cmd.args(["prune-logs", "--keep-days", "30", "--batch-size", "1"])
            .env(
                "DATABASE_URL",
                pool.connect_options().to_url_lossy().as_str(),
            );
        if apply {
            cmd.arg("--apply");
        }
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    assert_eq!(run(false)["deleted"]["log_rows"], 0);
    assert_eq!(count(&pool, id).await, 1);
    assert_eq!(run(true)["deleted"]["log_rows"], 1);
    assert_eq!(count(&pool, id).await, 0);
}
