CREATE TABLE cli_auth_sessions (
    session_hash BYTEA PRIMARY KEY,
    client_state TEXT NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX cli_auth_sessions_expiry ON cli_auth_sessions (expires_at);
