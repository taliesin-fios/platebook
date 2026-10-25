-- Soft-deleted intakes (roadmap v0.2): deleting an intake used to remove the
-- row, losing that lead's history. Now it sets deleted_at: the intake leaves
-- the inbox, the client's edit link and every action, but it (and its
-- payment_events) is kept and can be restored. A completed Stripe checkout
-- for a deleted intake restores it, so a payment is never hidden.
--
-- Additive, nullable column: safe on a live database. The existing RLS
-- policy on client_intakes covers it.
ALTER TABLE client_intakes
    ADD COLUMN deleted_at TIMESTAMPTZ;
