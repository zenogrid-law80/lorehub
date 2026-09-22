//! Execution reads use the same live repository permissions as web and CLI access.
use axum::http::StatusCode;
use sqlx::{
    Executor, FromRow, Postgres,
    postgres::{PgArguments, PgRow},
    query::QueryAs,
};
use uuid::Uuid;

use super::{
    api::ApiError,
    repositories::{RepositoryService, StorageBackend},
    repository_access,
};
use crate::ci::db::Pipeline;

pub(super) struct PipelineAccess {
    subject: String,
    primary_prefix: String,
    local_prefix: Option<String>,
}

impl PipelineAccess {
    pub(super) fn new(repositories: &RepositoryService, user: Uuid) -> Self {
        Self {
            subject: user.to_string(),
            primary_prefix: repositories.public_repository_url(""),
            local_prefix: repositories
                .public_repository_url_for(StorageBackend::LocalFile, "")
                .ok(),
        }
    }

    /// Parameters $1..$3 are reserved for the current user and configured URL prefixes.
    /// Match the full URL and backend, never just the last path component. The
    /// creation boundary keeps a recreated repository from inheriting old runs.
    pub(super) fn sql(statement: &str) -> String {
        format!(
            "WITH accessible_repositories AS (\
                SELECT resource_id, created_at, \
                    (CASE storage_backend WHEN 'dynamodb_s3' THEN $2::text WHEN 'local_file' THEN $3::text END) || name AS repository_url \
                FROM lore_resources WHERE {}\
            ), accessible_pipelines AS (\
                SELECT pipeline.* FROM pipelines pipeline \
                JOIN accessible_repositories repository \
                  ON pipeline.repository_url = repository.repository_url \
                 AND pipeline.created_at >= repository.created_at\
            ) {statement}",
            repository_access::ACCESS
        )
    }

    pub(super) fn query_as<'q, O>(&'q self, sql: &'q str) -> QueryAs<'q, Postgres, O, PgArguments>
    where
        O: for<'r> FromRow<'r, PgRow>,
    {
        sqlx::query_as(sql)
            .bind(&self.subject)
            .bind(&self.primary_prefix)
            .bind(&self.local_prefix)
    }

    pub(super) async fn pipeline<'e>(
        &self,
        executor: impl Executor<'e, Database = Postgres>,
        id: Uuid,
    ) -> Result<Pipeline, ApiError> {
        self.query_as(&Self::sql(
            "SELECT * FROM accessible_pipelines WHERE id = $4",
        ))
        .bind(id)
        .fetch_optional(executor)
        .await?
        // Do not distinguish an inaccessible ID from an unknown one.
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "pipeline not found".into()))
    }
}
