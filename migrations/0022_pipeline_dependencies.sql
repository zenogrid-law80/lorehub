ALTER TABLE pipelines
    ADD COLUMN pipeline_needs TEXT[] NOT NULL DEFAULT '{}';

CREATE INDEX pipelines_dependency_lookup
    ON pipelines (repository_url, branch, revision, pipeline_name, created_at DESC)
    WHERE pipeline_name IS NOT NULL;
