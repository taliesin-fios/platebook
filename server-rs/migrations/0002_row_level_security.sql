-- Row-Level Security: tenant isolation enforced by Postgres.
--
-- Every request that touches tenant data runs inside a transaction that does
--   SET LOCAL ROLE chef_app;
--   SELECT set_config('app.account_id', '<id>', true);
-- (see db::begin_tenant). chef_app is a NOLOGIN role with no BYPASSRLS, so
-- even when the app connects as a superuser (Railway's default `postgres`
-- user) the policies below apply to every tenant query. A query that forgets
-- `WHERE account_id = ...` sees only the signed-in account's rows, and a
-- transaction that never set app.account_id sees nothing at all.

-- The role is cluster-wide, so creating it must tolerate an earlier run (or a
-- concurrent one, e.g. parallel #[sqlx::test] databases on one server).
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'chef_app') THEN
        CREATE ROLE chef_app NOLOGIN NOSUPERUSER NOBYPASSRLS;
    END IF;
EXCEPTION WHEN duplicate_object OR unique_violation THEN
    NULL;
END
$$;

-- A non-superuser connection must be a member of chef_app to SET ROLE to it.
-- Superusers can always SET ROLE; ignore the error when we lack the right to
-- grant (the deploy notes say to grant it by hand in that case).
DO $$
BEGIN
    IF NOT (SELECT rolsuper FROM pg_roles WHERE rolname = current_user) THEN
        EXECUTE format('GRANT chef_app TO %I', current_user);
    END IF;
EXCEPTION WHEN insufficient_privilege OR duplicate_object THEN
    NULL;
END
$$;

GRANT USAGE ON SCHEMA public TO chef_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO chef_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO chef_app;
-- Tenant code never touches the migration bookkeeping table.
REVOKE ALL ON _sqlx_migrations FROM chef_app;

-- Current tenant, or NULL when unset (NULL never equals anything, so an unset
-- tenant matches no rows and passes no WITH CHECK).
CREATE FUNCTION app_account_id() RETURNS BIGINT
    LANGUAGE sql STABLE
    AS $$ SELECT NULLIF(current_setting('app.account_id', true), '')::BIGINT $$;

GRANT EXECUTE ON FUNCTION app_account_id() TO chef_app;

-- ── Tenant tables: account_id must be the current tenant ────────────────────
DO $$
DECLARE
    t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'clients', 'recipes', 'sessions', 'session_templates', 'client_intakes',
        'invoices', 'recipe_import_log', 'payment_events', 'settings'
    ] LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        -- FORCE applies the policies to the table owner too (superusers still
        -- bypass, which is why tenant transactions SET ROLE chef_app).
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY tenant_isolation ON %I
                USING (account_id = app_account_id())
                WITH CHECK (account_id = app_account_id())',
            t
        );
    END LOOP;
END
$$;

-- ── Child tables: visible only through a visible parent ─────────────────────
-- The subqueries run under the same role, so they are themselves filtered by
-- the parent table's policy.
ALTER TABLE ingredients ENABLE ROW LEVEL SECURITY;
ALTER TABLE ingredients FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON ingredients
    USING (EXISTS (SELECT 1 FROM recipes p WHERE p.id = ingredients.recipe_id))
    WITH CHECK (EXISTS (SELECT 1 FROM recipes p WHERE p.id = ingredients.recipe_id));

ALTER TABLE session_dishes ENABLE ROW LEVEL SECURITY;
ALTER TABLE session_dishes FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON session_dishes
    USING (EXISTS (SELECT 1 FROM sessions p WHERE p.id = session_dishes.session_id))
    -- A dish must also point at one of this account's recipes.
    WITH CHECK (
        EXISTS (SELECT 1 FROM sessions p WHERE p.id = session_dishes.session_id)
        AND (recipe_id IS NULL OR EXISTS (SELECT 1 FROM recipes r WHERE r.id = session_dishes.recipe_id))
    );

ALTER TABLE grocery_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE grocery_items FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON grocery_items
    USING (EXISTS (SELECT 1 FROM sessions p WHERE p.id = grocery_items.session_id))
    WITH CHECK (EXISTS (SELECT 1 FROM sessions p WHERE p.id = grocery_items.session_id));

ALTER TABLE invoice_line_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE invoice_line_items FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON invoice_line_items
    USING (EXISTS (SELECT 1 FROM invoices p WHERE p.id = invoice_line_items.invoice_id))
    WITH CHECK (EXISTS (SELECT 1 FROM invoices p WHERE p.id = invoice_line_items.invoice_id));

-- ── accounts: a tenant sees only its own row ────────────────────────────────
-- Not FORCEd: login/signup run as the connecting user (no tenant yet) and
-- must be able to look accounts up by email. Tenant code (chef_app) is
-- limited to its own row and cannot create or delete accounts.
ALTER TABLE accounts ENABLE ROW LEVEL SECURITY;
CREATE POLICY own_account ON accounts TO chef_app
    USING (id = app_account_id())
    WITH CHECK (id = app_account_id());
REVOKE INSERT, DELETE ON accounts FROM chef_app;
