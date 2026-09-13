ALTER TABLE pipelines ADD COLUMN pipeline_name TEXT;
ALTER TABLE pipelines ADD COLUMN runner_os TEXT CHECK (runner_os IN ('windows', 'macos', 'linux'));
ALTER TABLE pipelines ADD COLUMN branch TEXT;
ALTER TABLE pipelines ADD COLUMN previous_revision TEXT;
CREATE UNIQUE INDEX pipelines_push_once ON pipelines (repository_url, branch, revision, pipeline_name)
    WHERE pipeline_name IS NOT NULL;

CREATE TABLE ci_branch_cursors (
    resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    branch TEXT NOT NULL,
    revision TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (resource_id, branch)
);

CREATE TABLE ci_watched_repositories (
    resource_id TEXT PRIMARY KEY REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    initialized_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Also fence pre-upgrade workers: their unfiltered claim must never start a
-- targeted pipeline on the wrong OS.
CREATE FUNCTION enforce_pipeline_runner_os() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.status = 'running' AND OLD.status = 'queued' AND NEW.runner_os IS NOT NULL THEN
        IF NOT EXISTS (SELECT 1 FROM runners WHERE id = NEW.worker_id AND os = NEW.runner_os) THEN
            RAISE EXCEPTION 'pipeline requires a matching runner OS';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER pipeline_runner_os BEFORE UPDATE ON pipelines
    FOR EACH ROW EXECUTE FUNCTION enforce_pipeline_runner_os();
