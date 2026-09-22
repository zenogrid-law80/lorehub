-- Run only against the disposable restored database. Require the application
-- schema and successful migrations, not merely a syntactically valid archive.
\set ON_ERROR_STOP on
BEGIN READ ONLY;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM _sqlx_migrations)
       OR EXISTS (SELECT 1 FROM _sqlx_migrations WHERE NOT success) THEN
        RAISE EXCEPTION 'Missing or failed LoreHub migrations';
    END IF;
END
$$;
SELECT max(version) AS latest_migration FROM _sqlx_migrations;
SELECT 'pipelines' AS relation, count(*) AS rows FROM pipelines
UNION ALL SELECT 'jobs', count(*) FROM jobs
UNION ALL SELECT 'logs', count(*) FROM logs
UNION ALL SELECT 'lore_resources', count(*) FROM lore_resources
UNION ALL SELECT 'users', count(*) FROM users;
-- Touch dependent queue metadata and restored content as a basic read check.
SELECT count(*) AS named_executions FROM pipelines WHERE pipeline_name IS NOT NULL;
SELECT COALESCE(sum(octet_length(content)), 0) AS log_content_bytes FROM logs;
COMMIT;
