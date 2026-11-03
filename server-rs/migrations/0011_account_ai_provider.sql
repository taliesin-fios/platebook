-- Each account brings its own AI: a provider, an optional model and base URL,
-- and its own API key, so chefs on a shared server don't run on the
-- operator's key. The key is stored encrypted (`ai_api_key_enc`, see
-- `server-rs/src/ai_keys.rs`) and never returned to the browser; `ai_key_hint`
-- holds its last four characters for display.
--
-- NULL `ai_provider` means "not set up": the account falls back to the
-- server's ANTHROPIC_API_KEY when there is one, as before.
--
-- Additive, nullable columns: safe on a live database. The existing RLS
-- policy and grants on settings cover them.
ALTER TABLE settings
    ADD COLUMN ai_provider TEXT,
    ADD COLUMN ai_model TEXT,
    ADD COLUMN ai_base_url TEXT,
    ADD COLUMN ai_api_key_enc TEXT,
    ADD COLUMN ai_key_hint TEXT;
