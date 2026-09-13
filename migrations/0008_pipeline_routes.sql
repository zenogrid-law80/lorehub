CREATE TABLE ci_pipeline_route_snapshots (
    resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    branch TEXT NOT NULL,
    revision TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (resource_id, branch)
);

CREATE TABLE ci_pipeline_routes (
    resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    repository_url TEXT NOT NULL,
    branch TEXT NOT NULL,
    revision TEXT NOT NULL,
    pipeline_name TEXT NOT NULL,
    runner_os TEXT NOT NULL CHECK (runner_os IN ('windows', 'macos', 'linux')),
    trigger_patterns TEXT[] NOT NULL,
    working_directory TEXT NOT NULL,
    graph_definition TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (resource_id, branch, pipeline_name)
);

CREATE INDEX ci_pipeline_routes_updated ON ci_pipeline_routes (updated_at DESC);
