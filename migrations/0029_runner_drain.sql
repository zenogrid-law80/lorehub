-- Maintenance intent survives heartbeats and registration after a restart.
ALTER TABLE runners ADD COLUMN draining boolean NOT NULL DEFAULT false;
