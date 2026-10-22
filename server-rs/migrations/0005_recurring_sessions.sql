-- Recurring sessions (roadmap 3.1).
--
-- repeat_every_weeks: NULL = one-off; 1–8 = when this session is marked
-- complete, book the next one that many weeks later.
-- repeat_of: on a session booked that way, the session it follows. The
-- partial unique index means a session can only ever spawn one follow-up,
-- however often it is completed. No foreign key: deleting the source just
-- leaves a plain id behind, which is harmless.
--
-- Additive, nullable columns: safe on a live database.
ALTER TABLE sessions
    ADD COLUMN repeat_every_weeks SMALLINT CHECK (repeat_every_weeks BETWEEN 1 AND 8),
    ADD COLUMN repeat_of BIGINT;
CREATE UNIQUE INDEX idx_sessions_repeat_of
    ON sessions (account_id, repeat_of) WHERE repeat_of IS NOT NULL;
