ALTER TABLE pipelines ADD COLUMN category TEXT;

ALTER TABLE ci_pipeline_routes
    ADD COLUMN category TEXT NOT NULL DEFAULT 'uncategorized';
