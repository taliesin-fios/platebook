# CLAUDE.md

Platebook: a self-hostable, multi-account web app for home and personal chefs. Source-available under PolyForm Shield 1.0.0 (`LICENSE`).
Rust API in `server-rs/` (Axum + Tokio + SQLx on Postgres with Row-Level Security), React + TypeScript + Vite + Tailwind SPA in `client/`.
The old Express + SQLite server is gone. Data from an old SQLite install moves over with `tools/migrate-sqlite/` (standalone Node package).
Deeper docs: `server-rs/README.md` (running, tests, RLS, data migration, deploy runbook), `ARCHITECTURE.md` (auth, tenancy, public-route allowlist), `README.md` (env vars, setup), `CHANGELOG.md`.

## Commands

```bash
npm run db:setup                  # once: server-rs/.env + local Postgres database (or Docker)
npm --prefix client run lint
npm --prefix client test          # vitest
npm --prefix client run build     # type-checks (tsc), then vite build; no warnings (incl. the 500 kB chunk warning)
# in server-rs/ (node server-rs/scripts/cargo.mjs <args> finds cargo when it isn't on the PATH):
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                        # throwaway Postgres DB per test; regenerates client/src/types/generated/ (commit it)
npm run test:http                 # tests/http: api.smoke / auth / tenancy against a fresh server-rs
npm run test:migrate              # tools/migrate-sqlite: conversion tests + end-to-end migration into server-rs
```

Run all of them before every commit (`npm run lint` and `npm test` at the root cover them). `cargo test`, `test:http` and `test:migrate` need `psql` and a Postgres user that may create databases (default `postgres://postgres@localhost:5432/postgres`). Lint warnings are a pre-existing client baseline (~20): don't add new ones.
Dev servers: use the `.claude/launch.json` entries "Platebook — API (Rust)" (:3002) and "Platebook — Frontend (Vite)" (:5174, proxies `/api` to :3002, or to `API_PROXY_TARGET`). Never start them with Bash.
Local Node is 24; CI uses Node 20 for the client (`engines: 20.x`). Don't use Node-22+-only APIs. Rust: stable (MSRV 1.92); on Windows the GNU toolchain with scoop's MinGW.

## Git, PRs and deploys

- Never push to `main` directly. Work on a branch, open a PR, merge only when the maintainer says so. If a change adds a migration in `server-rs/migrations/`, say so in the PR.
- CI (`.github/workflows/ci.yml`) lints, tests and builds the client on Node 20 and runs fmt, clippy, cargo test and the HTTP suites. Still run them locally before pushing.
- `.db` files and `.env` files (`server-rs/.env`, and `server/.env` in older checkouts) are gitignored and may hold real data and secrets. Never commit them, never delete them, never print `.env` values. Open a legacy SQLite file read-only, and only through the migrator. Test fixtures are SQL text with synthetic data only.
- `CLAUDE.local.md` (gitignored) holds a maintainer's own notes and rules; read it when present.

## Multi-tenancy rules (most important)

Every chef business is a row in `accounts`. In server-rs the auth middleware puts the account in the request; data handlers take the `db::Tenant` extractor (or call `db::begin_tenant`), which opens a transaction with `SET LOCAL ROLE chef_app` and `app.account_id`, so Postgres RLS only shows that account's rows.

- Still filter or insert `account_id` explicitly on `clients`, `recipes`, `sessions`, `session_templates`, `client_intakes`, `invoices`, `recipe_import_log`, `payment_events` (RLS is the backstop, not the only check).
- Child tables (`ingredients`, `session_dishes`, `grocery_items`, `invoice_line_items`) have no `account_id`: they're visible only through a visible parent; check the parent's ownership first.
- Any id from the request body or params (client, recipe, session, template, invoice) must be checked with `ownership::owned` / `owned_ids` (404 / 400 like before).
- `settings` is one row per account (`account_id` primary key). Read it with `accounts::get_settings`.
- Code that runs before there is a tenant (signup/login, public profile and intake by slug, intake by token, Stripe webhook) uses the pool directly for that one lookup, then switches to `begin_tenant`. Keep it that way.
- Per-account rate limits (AI, import, export, PDF) key on `AccountLimits::key(account_id)` (`acct:<id>`), not the IP.
- Every new data route needs a cross-account test (`server-rs/tests/tenancy.rs`, and `tests/http/tenancy.test.js` using `makeAccount()` / `as(account)` from `tests/http/helpers.js`).
- Adding a public (no-auth) route means updating `auth::is_public_route` in `server-rs/src/auth.rs` (unit-tested) **and** the allowlist table in `ARCHITECTURE.md`.

## Schema and migrations

- Schema lives in `server-rs/migrations/` (`0001_schema.sql`, `0002_row_level_security.sql`, …), applied by the server on start (`db::MIGRATOR`). Add a new numbered file; never edit one that has shipped.
- Migrations must be safe on a live database with data. Prefer additive columns. New tenant tables need `account_id`, an RLS policy and composite `(account_id, id)` foreign keys like the others.
- After changing a `sqlx::query!` / `query_as!` macro, refresh `server-rs/.sqlx/` (see `server-rs/README.md`); CI builds with `SQLX_OFFLINE=true`.
- Starter content is per account (`server-rs/src/seed/`, called from onboarding). Nothing is seeded globally.
- SQLite → Postgres: `tools/migrate-sqlite/migrate-to-postgres.js` (read-only on the SQLite file, one transaction, verified counts). A schema change that affects migrated tables needs the tool (and `test/fixtures/expected.json` if a response changes) updated, and `npm run test:migrate` to pass.

## Conventions

- Money: use the helpers in `server-rs/src/money.rs` (`rust_decimal`, integer-cents math). `deposit_cents` is stored in cents; other fee columns are dollars (`NUMERIC(12,2)`).
- Postgres types vs JSON: booleans serialize as `0`/`1` (`serde_util::bool_as_int`), `DATE` as `YYYY-MM-DD`, `*_at` timestamps as `YYYY-MM-DD HH:MM:SS` UTC.
- Dates (server): "today" / "this month" / "overdue" use the account's clock, `accounts::local_clock` (its `settings.timezone`, else UTC), never the server's UTC date.
- Dates (client): `YYYY-MM-DD` fields go through `parseLocalDate` / `fmtSessionDate` in `client/src/utils/dateUtils.ts`, and `*_at` timestamps through `fmtUtcTimestamp`. `parseISO` is lint-banned outside `dateUtils.ts` because of past timezone off-by-one bugs.
- Errors: handlers return `AppError`; internal errors go through `error::err_msg` (hides details when `NODE_ENV=production`), never the raw error text. JSON errors are `{ "error", "code" }`.
- Branding: the product name is `APP_NAME` / `APP_TAGLINE` in `client/src/constants.ts`. Each chef's business name, logo and booking policy come from their settings. Don't hardcode any business name, price, deposit or policy text.
- Email (`server-rs/src/email.rs`, lettre) is per account. Client-facing mail sets `Reply-To` to the chef's business email. With `EMAIL_HOST` empty, mail goes to the log/outbox (dev and tests).
- PDFs are rendered only on the server with Typst (`server-rs/src/pdf/`, templates in `templates/*.typ`, data passed as JSON, fonts embedded); the client downloads them through `client/src/utils/serverPdf.ts`.
- Client is TypeScript (strict). API response shapes are generated from the Rust structs (`ts-rs`) into `client/src/types/generated/` and re-exported from `client/src/types/*.ts`; type `apiCall<T>()` / `useApi<T>()` with them. Annotate `i64` fields with `#[ts(type = "number")]`. No `any`.
- Tailwind 4: theme tokens are in the `@theme` block of `client/src/index.css` (there is no `tailwind.config.js`).
- Every source file (`.ts/.tsx/.js/.mjs/.rs/.typ/.css`, not generated files or shipped migrations) starts with the SPDX + copyright header; `npm run lint` checks it (`scripts/license-headers.mjs`, `--fix` adds it). Keep the Platebook credits (`PoweredBy`, PDF footers, Settings → About, NOTICE).
- Files use LF line endings (`.gitattributes`). When editing with scripts on Windows, write with `newline=''` so CRLF isn't introduced.
- Tests must never send real email, take payments or call the Claude API: cargo tests use the outbox and local fakes, and `tests/http/run.mjs` starts the server with `EMAIL_HOST=''`, `PAYMENTS_MODE=manual` and no `ANTHROPIC_API_KEY`.

## Verification

For UI changes, check the flow in the preview browser against the Rust API (welcome → signup → onboarding → dashboard, or the affected page), including phone width (375px). For server-only changes, cargo tests plus `npm run test:http` are enough.
