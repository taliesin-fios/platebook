-- Opt-in session reminder emails (roadmap 3.2).
--
-- settings.remind_client / remind_chef: off by default; each chef turns them
-- on in Settings. settings.timezone: IANA name (e.g. America/Chicago) used to
-- turn a session's local date + time into an instant; NULL means UTC.
-- sessions.*_reminder_sent_at: set when a reminder is claimed for sending, so
-- each goes out once; cleared when the session's date or time changes.
--
-- Additive columns (NOT NULL ones with constant defaults need no table
-- rewrite): safe on a live database.
ALTER TABLE settings
    ADD COLUMN remind_client BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN remind_chef   BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN timezone      TEXT;

ALTER TABLE sessions
    ADD COLUMN client_reminder_sent_at TIMESTAMPTZ,
    ADD COLUMN chef_reminder_sent_at   TIMESTAMPTZ;

-- The reminder job scans upcoming, active sessions by date.
CREATE INDEX idx_sessions_reminder_due
    ON sessions (session_date) WHERE status IN ('Pending', 'Confirmed');
