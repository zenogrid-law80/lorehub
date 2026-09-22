CREATE INDEX pipelines_repository_history
    ON pipelines (repository_url, created_at DESC, id DESC);
