CREATE TABLE ci_repository_pipeline_branches (
    resource_id TEXT PRIMARY KEY REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    branches TEXT[] NOT NULL DEFAULT ARRAY['main']::TEXT[]
        CHECK (cardinality(branches) <= 200 AND array_position(branches, NULL) IS NULL),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO ci_repository_pipeline_branches(resource_id, branches)
SELECT resource_id, ARRAY['main']::TEXT[]
FROM lore_resources
ON CONFLICT DO NOTHING;
