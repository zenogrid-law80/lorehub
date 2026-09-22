-- Observed by the Coordinator for both legacy and request-ID claim routes.
ALTER TABLE runners ADD COLUMN last_claim_at timestamptz;
