-- Push-triggered runs always carry their previous revision (including the zero
-- revision for the first push). Manual runs have no previous_revision and may
-- be repeated at the same branch/revision without suppressing a future push.
DROP INDEX pipelines_push_once;
CREATE UNIQUE INDEX pipelines_push_once
    ON pipelines (repository_url, branch, revision, pipeline_name)
    WHERE pipeline_name IS NOT NULL AND previous_revision IS NOT NULL;
