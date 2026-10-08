CREATE TABLE ci_configs (
    id UUID PRIMARY KEY,
    resource_id TEXT REFERENCES lore_resources(resource_id) ON DELETE SET NULL,
    branch TEXT NOT NULL,
    source_mode TEXT NOT NULL CHECK (source_mode IN ('file', 'db')),
    active_revision_id UUID,
    lock_version BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (resource_id, branch)
);

CREATE TABLE ci_config_revisions (
    id UUID PRIMARY KEY,
    config_id UUID NOT NULL REFERENCES ci_configs(id),
    version BIGINT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version = 1),
    definition JSONB NOT NULL,
    source_toml TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    source_revision TEXT,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (config_id, version),
    UNIQUE (config_id, id)
);
ALTER TABLE ci_configs ADD CONSTRAINT ci_configs_active_revision
    FOREIGN KEY (id, active_revision_id) REFERENCES ci_config_revisions(config_id, id);
ALTER TABLE ci_configs ADD CONSTRAINT ci_configs_db_revision
    CHECK (source_mode <> 'db' OR active_revision_id IS NOT NULL);

CREATE TABLE ci_config_events (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    config_id UUID NOT NULL REFERENCES ci_configs(id),
    action TEXT NOT NULL CHECK (action IN ('save', 'restore', 'file')),
    previous_revision_id UUID REFERENCES ci_config_revisions(id),
    revision_id UUID REFERENCES ci_config_revisions(id),
    actor UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE ci_run_groups (
    id UUID PRIMARY KEY,
    resource_id TEXT REFERENCES lore_resources(resource_id) ON DELETE SET NULL,
    branch TEXT NOT NULL,
    code_revision TEXT NOT NULL,
    config_revision_id UUID NOT NULL REFERENCES ci_config_revisions(id),
    trigger_kind TEXT NOT NULL CHECK (trigger_kind IN ('push', 'manual', 'rerun')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX ci_run_groups_push_once
    ON ci_run_groups(resource_id, branch, code_revision) WHERE trigger_kind = 'push';

ALTER TABLE pipelines ADD COLUMN config_revision_id UUID REFERENCES ci_config_revisions(id);
ALTER TABLE pipelines ADD COLUMN run_group_id UUID REFERENCES ci_run_groups(id);
ALTER TABLE pipelines ADD COLUMN execution_spec JSONB;
ALTER TABLE pipelines ADD CONSTRAINT pipelines_config_snapshot CHECK (
    (config_revision_id IS NULL AND run_group_id IS NULL AND execution_spec IS NULL) OR
    (config_revision_id IS NOT NULL AND run_group_id IS NOT NULL AND execution_spec IS NOT NULL)
);
CREATE UNIQUE INDEX pipelines_group_name ON pipelines(run_group_id, pipeline_name)
    WHERE run_group_id IS NOT NULL AND pipeline_name IS NOT NULL;

CREATE FUNCTION preserve_ci_config_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'CI configuration revisions are immutable';
END;
$$;
CREATE TRIGGER ci_config_revision_immutable BEFORE UPDATE OR DELETE ON ci_config_revisions
    FOR EACH ROW EXECUTE FUNCTION preserve_ci_config_revision();

CREATE FUNCTION preserve_ci_execution_spec() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.execution_spec IS NOT NULL AND (
        NEW.execution_spec IS DISTINCT FROM OLD.execution_spec OR
        NEW.config_revision_id IS DISTINCT FROM OLD.config_revision_id OR
        NEW.run_group_id IS DISTINCT FROM OLD.run_group_id OR
        NEW.revision IS DISTINCT FROM OLD.revision OR
        NEW.repository_url IS DISTINCT FROM OLD.repository_url OR
        NEW.branch IS DISTINCT FROM OLD.branch OR
        NEW.pipeline_name IS DISTINCT FROM OLD.pipeline_name OR
        NEW.pipeline_needs IS DISTINCT FROM OLD.pipeline_needs OR
        NEW.runner_os IS DISTINCT FROM OLD.runner_os OR
        NEW.working_directory IS DISTINCT FROM OLD.working_directory OR
        NEW.sparse_view_name IS DISTINCT FROM OLD.sparse_view_name OR
        NEW.sparse_view_rules IS DISTINCT FROM OLD.sparse_view_rules
    ) THEN
        RAISE EXCEPTION 'Queued CI execution specifications are immutable';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER ci_execution_spec_immutable BEFORE UPDATE ON pipelines
    FOR EACH ROW EXECUTE FUNCTION preserve_ci_execution_spec();
