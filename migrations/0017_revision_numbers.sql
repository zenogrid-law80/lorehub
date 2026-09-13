CREATE TABLE ci_revision_numbers (
    revision_number BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    repository_url TEXT NOT NULL,
    revision TEXT NOT NULL,
    UNIQUE (repository_url, revision)
);

INSERT INTO ci_revision_numbers (repository_url, revision)
SELECT repository_url, revision
FROM (
    SELECT repository_url, revision FROM pipelines
    UNION
    SELECT repository_url, revision FROM ci_pipeline_routes
) revisions
ORDER BY repository_url, revision;

ALTER TABLE pipelines ADD COLUMN revision_number BIGINT;
UPDATE pipelines pipeline
SET revision_number = numbered.revision_number
FROM ci_revision_numbers numbered
WHERE numbered.repository_url = pipeline.repository_url
  AND numbered.revision = pipeline.revision;
ALTER TABLE pipelines ALTER COLUMN revision_number SET NOT NULL;

ALTER TABLE ci_pipeline_routes ADD COLUMN revision_number BIGINT;
UPDATE ci_pipeline_routes route
SET revision_number = numbered.revision_number
FROM ci_revision_numbers numbered
WHERE numbered.repository_url = route.repository_url
  AND numbered.revision = route.revision;
ALTER TABLE ci_pipeline_routes ALTER COLUMN revision_number SET NOT NULL;

CREATE FUNCTION assign_ci_revision_number() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO ci_revision_numbers (repository_url, revision)
    VALUES (NEW.repository_url, NEW.revision)
    ON CONFLICT (repository_url, revision)
    DO UPDATE SET revision = EXCLUDED.revision
    RETURNING revision_number INTO NEW.revision_number;
    RETURN NEW;
END;
$$;

CREATE TRIGGER pipelines_assign_revision_number
    BEFORE INSERT OR UPDATE OF repository_url, revision ON pipelines
    FOR EACH ROW EXECUTE FUNCTION assign_ci_revision_number();

CREATE TRIGGER ci_pipeline_routes_assign_revision_number
    BEFORE INSERT OR UPDATE OF repository_url, revision ON ci_pipeline_routes
    FOR EACH ROW EXECUTE FUNCTION assign_ci_revision_number();
