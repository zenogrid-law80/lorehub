#![cfg(unix)]
use std::{os::unix::fs::PermissionsExt, process::Command};

use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and pg_dump/pg_restore/psql tools"]
async fn backup_restores_in_isolation_and_rejects_corrupt_archives(pool: PgPool) {
    let root = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO pipelines (id,repository_url,revision) VALUES ($1,'lores://test/repo','restore-check')")
        .bind(id).execute(&pool).await.unwrap();
    lorehub::ci::db::log(&pool, id, None, "stdout", "복원 확인\n")
        .await
        .unwrap();
    // SQLx's URL serializer adds driver-only parameters that libpq rejects.
    let mut source_url = url::Url::parse(&std::env::var("DATABASE_URL").unwrap()).unwrap();
    source_url.set_path(pool.connect_options().get_database().unwrap());
    let backup = Command::new("sh")
        .arg("scripts/backup-postgres.sh")
        .arg(root.path())
        .env("DATABASE_URL", source_url.as_str())
        .output()
        .unwrap();
    assert!(
        backup.status.success(),
        "{}",
        String::from_utf8_lossy(&backup.stderr)
    );
    let archive = String::from_utf8(backup.stdout).unwrap().trim().to_owned();
    assert_eq!(
        std::fs::metadata(&archive).unwrap().permissions().mode() & 0o077,
        0
    );
    let verify = |path: &str| {
        Command::new("sh")
            .args(["scripts/verify-postgres-backup.sh", path])
            // The verification must ignore all inherited operational connection settings.
            .env(
                "DATABASE_URL",
                "postgres://invalid:invalid@127.0.0.1:1/never-connect",
            )
            .env("PGHOSTADDR", "192.0.2.1")
            .output()
            .unwrap()
    };
    let restored = verify(&archive);
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    let output = String::from_utf8(restored.stdout).unwrap();
    assert!(
        output
            .lines()
            .any(|line| line.split('|').map(str::trim).collect::<Vec<_>>() == ["pipelines", "1"]),
        "{output}"
    );
    assert!(
        output
            .lines()
            .any(|line| line.split('|').map(str::trim).collect::<Vec<_>>() == ["logs", "1"]),
        "{output}"
    );
    let original: String = sqlx::query_scalar("SELECT content FROM logs WHERE pipeline_id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(original, "복원 확인\n");
    // A readable archive directory alone does not prove the data can be restored.
    let bytes = std::fs::read(&archive).unwrap();
    let corrupt = root.path().join("truncated.dump");
    std::fs::write(&corrupt, &bytes[..bytes.len() / 2]).unwrap();
    assert!(!verify(corrupt.to_str().unwrap()).status.success());
}
