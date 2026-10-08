-- Presentation state is independent of immutable CI definitions and executions.
CREATE TABLE ci_graph_layouts (
    resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    branch TEXT NOT NULL,
    graph TEXT NOT NULL,
    positions JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(positions) = 'object' AND octet_length(positions::text) <= 262144),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (resource_id, branch, graph)
);
