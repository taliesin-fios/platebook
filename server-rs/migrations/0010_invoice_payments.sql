-- Invoice payment links (roadmap v0.3): clients pay an invoice online from
-- its link (Stripe Checkout) and the webhook marks it paid.
--
-- invoices.stripe_session_id: the latest Checkout session opened for the
--   invoice, so a new one can expire it first (only the newest is payable).
-- payment_events.invoice_id: the invoice a payment event belongs to (deposit
--   events keep using intake_id). Composite key like intake_id, so an event
--   can only point at an invoice of its own account. Code that hard-deletes
--   invoices detaches their events first, as intake deletion used to.
--
-- Additive, nullable columns: safe on a live database. The constraint is
-- added NOT VALID and validated separately so existing rows (all NULL) don't
-- hold a long lock. The existing RLS policies cover both tables.
ALTER TABLE invoices
    ADD COLUMN stripe_session_id TEXT;
ALTER TABLE payment_events
    ADD COLUMN invoice_id BIGINT;
ALTER TABLE payment_events
    ADD CONSTRAINT payment_events_invoice_fkey
    FOREIGN KEY (account_id, invoice_id) REFERENCES invoices (account_id, id) NOT VALID;
ALTER TABLE payment_events
    VALIDATE CONSTRAINT payment_events_invoice_fkey;
