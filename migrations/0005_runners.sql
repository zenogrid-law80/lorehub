CREATE TABLE runners (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    os TEXT NOT NULL,
    arch TEXT NOT NULL,
    version TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
    stopped_at TIMESTAMPTZ
);

CREATE INDEX runners_last_seen ON runners (last_seen DESC);
