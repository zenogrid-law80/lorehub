CREATE TABLE lore_resources (
    resource_id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    owner_subject TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX lore_resources_owner ON lore_resources (owner_subject, resource_id);

ALTER TABLE pipelines ADD COLUMN submitted_by UUID REFERENCES users(id) ON DELETE SET NULL;
