-- Retain the request identity with the execution, including after lease expiry.
ALTER TABLE pipelines ADD COLUMN claim_request_id uuid;
CREATE UNIQUE INDEX pipelines_worker_claim_request
    ON pipelines (worker_id, claim_request_id)
    WHERE claim_request_id IS NOT NULL;
CREATE INDEX pipelines_running_worker ON pipelines (worker_id)
    WHERE status = 'running';
