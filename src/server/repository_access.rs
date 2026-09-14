//! Repository authorization uses the current account role, including for existing CLI tokens.
use sqlx::PgPool;

const ACCESS: &str =
    "(owner_subject = $1 OR EXISTS (SELECT 1 FROM users WHERE id::text = $1 AND role = 'admin'))";

pub(crate) async fn resource_ids(pool: &PgPool, subject: &str) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT resource_id FROM lore_resources WHERE {ACCESS} ORDER BY resource_id"
    ))
    .bind(subject)
    .fetch_all(pool)
    .await
}

pub(crate) async fn by_name(
    pool: &PgPool,
    subject: &str,
    name: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT resource_id FROM lore_resources WHERE {ACCESS} AND name = $2"
    ))
    .bind(subject)
    .bind(name)
    .fetch_optional(pool)
    .await
}

pub(crate) async fn can_access(
    pool: &PgPool,
    subject: &str,
    resource_id: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(&format!(
        "SELECT EXISTS(SELECT 1 FROM lore_resources WHERE {ACCESS} AND resource_id = $2)"
    ))
    .bind(subject)
    .bind(resource_id)
    .fetch_one(pool)
    .await
}

pub(crate) async fn delete(
    pool: &PgPool,
    subject: &str,
    resource_id: &str,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query(&format!(
        "DELETE FROM lore_resources WHERE {ACCESS} AND resource_id = $2"
    ))
    .bind(subject)
    .bind(resource_id)
    .execute(pool)
    .await?
    .rows_affected()
        > 0)
}
