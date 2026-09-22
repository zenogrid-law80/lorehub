-- Completion observations, independent of whether a branch revision changed.
CREATE TABLE ci_repository_watch_health (
    resource_id TEXT PRIMARY KEY REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_success_at TIMESTAMPTZ,
    error_code TEXT CHECK (error_code IN ('watch_failed', 'link_index_failed', 'backend_unavailable')),
    consecutive_failures BIGINT NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0)
);
