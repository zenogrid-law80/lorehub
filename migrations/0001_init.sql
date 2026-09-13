CREATE TABLE pipelines (
    id UUID PRIMARY KEY,
    repository_url TEXT NOT NULL,
    revision TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'canceled')),
    worker_id UUID,
    lease_until TIMESTAMPTZ,
    cancel_requested BOOLEAN NOT NULL DEFAULT FALSE,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ
);
CREATE INDEX pipelines_queue ON pipelines (created_at, id) WHERE status = 'queued';
CREATE INDEX pipelines_leases ON pipelines (lease_until) WHERE status = 'running';

CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    pipeline_id UUID NOT NULL REFERENCES pipelines(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    name TEXT NOT NULL,
    stage TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'canceled', 'skipped')),
    exit_code INTEGER,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    UNIQUE (pipeline_id, position),
    UNIQUE (pipeline_id, name)
);
CREATE TABLE logs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    pipeline_id UUID NOT NULL REFERENCES pipelines(id) ON DELETE CASCADE,
    job_id UUID REFERENCES jobs(id) ON DELETE CASCADE,
    stream TEXT NOT NULL CHECK (stream IN ('stdout', 'stderr', 'system')),
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX logs_pipeline_cursor ON logs (pipeline_id, id);
