-- Keep execution metadata: dependencies and push deduplication rely on it.
ALTER TABLE pipelines ADD COLUMN logs_pruned_at TIMESTAMPTZ;

CREATE INDEX pipelines_log_retention
    ON pipelines (finished_at, id)
    WHERE status IN ('succeeded', 'failed', 'canceled');
