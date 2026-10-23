-- Email reliability (roadmap "Polish"): remember when a meal-plan or invoice
-- email failed, so the session and invoice screens can say so and offer a
-- retry after the chef has left the page.
--
-- sessions.pdf_email_failed_at: last failed meal-plan email; cleared when one
--   succeeds (which sets pdf_emailed_at, as before).
-- invoices.emailed_at / email_failed_at: last successful / failed invoice
--   email; a success clears email_failed_at.
--
-- Additive, nullable columns: safe on a live database. The existing RLS
-- policies on sessions and invoices cover them.
ALTER TABLE sessions
    ADD COLUMN pdf_email_failed_at TIMESTAMPTZ;
ALTER TABLE invoices
    ADD COLUMN emailed_at      TIMESTAMPTZ,
    ADD COLUMN email_failed_at TIMESTAMPTZ;
