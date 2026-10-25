-- Booking email reliability (roadmap v0.2): remember what happened to the two
-- emails sent when a client submits the booking form, so the intake screens
-- can say so instead of the failure only reaching the server log.
--
-- client_intakes.confirmation_emailed_at / confirmation_email_failed_at: last
--   successful / failed confirmation email to the client; a success (the
--   automatic one or a chef's "Try again") clears confirmation_email_failed_at.
-- client_intakes.chef_alert_failed_at: the new-booking alert to the chef
--   failed. It is sent once, so nothing clears it.
--
-- Additive, nullable columns: safe on a live database. The existing RLS
-- policy on client_intakes covers them.
ALTER TABLE client_intakes
    ADD COLUMN confirmation_emailed_at      TIMESTAMPTZ,
    ADD COLUMN confirmation_email_failed_at TIMESTAMPTZ,
    ADD COLUMN chef_alert_failed_at         TIMESTAMPTZ;
