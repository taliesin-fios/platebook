-- State that must be shared by every server instance (was in process memory,
-- which is only correct with a single instance).
--
-- Neither table holds tenant data, so neither has RLS: the server reads and
-- writes them through the pool as the connecting user, never as chef_app.

-- Single-use magic-link / password-reset token ids. A row lives until the
-- token itself would have expired; after that the JWT `exp` check rejects
-- the token before this table is consulted, so expired rows can be pruned.
CREATE TABLE used_jtis (
    jti        TEXT PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_used_jtis_expires ON used_jtis (expires_at);

-- Fixed-window rate-limit counters, one row per (limiter, key) — the key is
-- an IP for the auth limiters and `acct:{id}` for the per-account ones.
CREATE TABLE rate_limit_hits (
    bucket       TEXT NOT NULL,
    key          TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    hits         INTEGER NOT NULL,
    PRIMARY KEY (bucket, key)
);
CREATE INDEX idx_rate_limit_hits_window ON rate_limit_hits (window_start);
