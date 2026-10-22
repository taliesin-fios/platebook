-- Server-side session revocation.
--
-- Session cookies are signed JWTs checked on every request. A token issued
-- (`iat`) before `sessions_valid_after` is rejected, so a password reset ends
-- every other session for the account. NULL (every existing row) means no
-- cutoff. Additive and nullable: safe on a live database.
ALTER TABLE accounts ADD COLUMN sessions_valid_after TIMESTAMPTZ;
