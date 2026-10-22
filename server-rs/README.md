# server-rs — Platebook API in Rust

The Platebook API (Axum + Tokio + SQLx on Postgres). It replaced the old
Express + SQLite server, which has been removed; `npm run dev` at the
repository root runs it.

Every Express route was ported with the same URLs, status codes and JSON
shapes: auth, settings, onboarding (+ the per-account starter library),
public profile, templates, clients, costs (+ CSV export), grocery, planner,
sessions, recipes (+ URL and Claude text import), intake (+ HMAC edit
tokens), invoices, payments (manual + Stripe Checkout and webhooks), AI,
email (lettre), helmet's security headers and serving the React build.
The API types the client uses are generated from these structs (ts-rs).

## Running locally

Needs Rust (stable) and Postgres 14+ (or Docker). From the repository root:

```bash
npm run db:setup              # server-rs/.env + the database (local Postgres, or Docker)
npm run dev                   # this server on :3002 + Vite on :5174
```

`db:setup` (`scripts/db-setup.mjs`) copies `.env.example` to `.env` with
random secrets if there is no `.env`, then creates the database named by
`DATABASE_URL` when a Postgres server answers there; otherwise (or with
`npm run db:setup -- --docker`) it starts Postgres 18 from the root
`docker-compose.yml` (`127.0.0.1:5432`, database `chef_dev`, trust auth). It
never drops anything. The server applies the schema on start (`cargo run`
here does the same).

Or use the "Platebook — API (Rust)" entry in `.claude/launch.json`
(`scripts/dev.mjs` finds cargo in `~/.cargo/bin` if it isn't on the PATH,
via `scripts/cargo.mjs`), and "Platebook — Frontend (Vite)" (`npm run dev` in
`client/`), the Vite dev server with `/api` proxied to :3002
(`API_PROXY_TARGET` overrides it).

On Windows without Visual Studio, the GNU toolchain works
(`rustup default stable-x86_64-pc-windows-gnu`) with MinGW on the PATH
(`scoop install mingw`) for `dlltool` and `gcc`.

## Checks (all must pass before a commit; CI runs them too)

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
DATABASE_URL=postgres://postgres@localhost:5432/postgres cargo test
```

`cargo test` creates a throwaway database per test (`#[sqlx::test]`) on the
server named by `DATABASE_URL`, so that user needs `CREATEDB`. Test files:
`auth_routes.rs`, `data_routes.rs` (every data route group),
`integrations.rs` (intake tokens, email outbox, payments + Stripe and the
Claude API against local fakes, SSRF/rate-limit guards, headers, SPA),
`tenancy.rs` (cross-account checks over HTTP) and `rls.rs` (the same at the
database level). No test sends mail or calls a real external API.

### HTTP suites

```bash
npm run test:http         # at the repository root: node tests/http/run.mjs
```

Black-box suites in `tests/http/` (`api.smoke`, `auth`, `tenancy`; node:test +
supertest, their own `package.json`). The runner builds server-rs, starts it
on a throwaway database with test secrets (no mail, manual payments, no AI
key) and runs the suites with `API_BASE_URL` pointing at it. Needs `psql` on
the PATH and a Postgres user that may create databases
(`PARITY_ADMIN_DATABASE_URL`, default `postgres://postgres@localhost:5432/postgres`).
These suites were written against the Express server and passed against both
before it was removed; CI runs them.

### Generated client types

Response structs derive `ts_rs::TS`; `cargo test` writes them to
`client/src/types/generated/` (`.cargo/config.toml`), and the client's
`types/*.ts` re-export them. After changing a response, run `cargo test` and
commit the regenerated files (CI fails if they're stale). Annotate `i64`
fields with `#[ts(type = "number")]` — a client test rejects `bigint`.

## Compile-time checked SQL

`sqlx::query!` / `query_as!` are checked against the real schema when the
crate compiles. With `DATABASE_URL` set, the check runs against that
database (it must already be migrated — `cargo run` once does that). Without
it, or with `SQLX_OFFLINE=true` (CI), the committed `.sqlx/` cache is used.

After adding or changing a checked query, refresh the cache and commit it:

```bash
DATABASE_URL=postgres://postgres@localhost:5432/chef_dev SQLX_OFFLINE_DIR="$PWD/.sqlx" cargo check --all-targets
```

(or `cargo sqlx prepare` if you have `sqlx-cli`).

## Tenancy: Row-Level Security

`migrations/0002_row_level_security.sql` turns on RLS for every tenant table
(`clients`, `recipes`, `sessions`, `session_templates`, `client_intakes`,
`invoices`, `recipe_import_log`, `payment_events`, `settings`). The child
tables (`ingredients`, `session_dishes`, `grocery_items`,
`invoice_line_items`) are visible only through a visible parent row.

Handlers that touch tenant data take the `db::Tenant` extractor (or call
`db::begin_tenant`). That opens a transaction which runs
`SET LOCAL ROLE chef_app` and sets `app.account_id`, so:

- a query that forgets `WHERE account_id = …` still only sees the signed-in
  account's rows;
- inserting or updating a row with another account's `account_id` fails;
- a transaction that never set a tenant sees nothing.

Cross-table references between tenant tables are composite foreign keys on
`(account_id, id)`, so a row can't point at another account's row either
(FK checks bypass RLS, so this closes that gap). Keep writing the explicit
`account_id` filters anyway: they document intent and help the planner.

The RLS role switch matters because Railway's `postgres` user is a
superuser, and superusers bypass RLS. `chef_app` is a `NOLOGIN` role created
by the migration. If the app ever connects as a non-superuser, that user must
be a member of `chef_app` (`GRANT chef_app TO <user>`; the migration tries).

Code that runs before there is a tenant uses the pool directly as the
connecting user, which on Railway is a superuser and so bypasses RLS:
signup/login, the public profile and intake submission (account by slug),
intake read/edit by token (`client_intakes.account_id` by id) and the Stripe
webhook (owner of the intake in the event metadata, plus the
`stripe_event_id` idempotency check). Each does only that lookup and then
switches to `begin_tenant` for the account it found. If the app ever
connects as a non-superuser, those lookups need a role with `BYPASSRLS` (or
`SECURITY DEFINER` functions).

Ids from params or the body are still checked explicitly
(`ownership::owned` / `owned_ids`, the ports of `ownedRow()` / `ownedIds()`),
so the routes answer with the same 404/400 as Node; RLS is the backstop.

## Schema differences from SQLite

Column names are unchanged so the JSON API keeps its shape. Types are real
Postgres types, and route ports convert where the JSON differs:

| SQLite | Postgres | JSON note |
|---|---|---|
| `INTEGER` 0/1 flags | `BOOLEAN` | serialize with `serde_util::bool_as_int` to keep `0`/`1` |
| `TEXT '[]'` | `JSONB` | Node returned the raw string; decide per route |
| `TEXT` dates | `DATE` | format as `YYYY-MM-DD` |
| `TEXT datetime('now')` | `TIMESTAMPTZ` | Node sent `YYYY-MM-DD HH:MM:SS` (UTC) |
| `REAL` dollars | `NUMERIC(12,2)` | `rust_decimal`, serialized as a number |
| `deposit_cents INTEGER` | `BIGINT` | unchanged |

Also new: `account_id` is `NOT NULL`; email uniqueness is on `lower(email)`;
`db_meta` is gone (`_sqlx_migrations` tracks migrations).

## Compatibility with the old Express server's data

- Same `authToken` cookie and JWT (`HS256`, `{ accountId, email }`, 7 days):
  with the same `JWT_SECRET`, sessions issued by the old server stay valid.
- `bcrypt` verifies the existing `$2a$` hashes from bcryptjs (new hashes are
  `$2b$`), so migrated accounts sign in with their passwords.
- Same public-route allowlist (`auth::is_public_route`, unit-tested). Keep it
  and the table in `ARCHITECTURE.md` in sync.
- Errors are `{ "error", "code" }`; 500s go through `error::err_msg`, which
  hides details when `NODE_ENV=production`.

## Deliberate differences from the Express server

Found by the (since removed) Express-vs-Rust shape diff and kept on purpose:

- Invoice line items with a non-numeric quantity/price answer 400
  `VALIDATION_ERROR` (Node threw into its generic catch: 500, after already
  inserting the invoice). Every handler runs in one transaction, so a failed
  request never leaves partial writes.
- `DELETE /api/intake/:id` detaches the intake's `payment_events` (keeping
  the audit trail) instead of failing on the foreign key (Node: 500).
- Three responses that echoed raw rows now return parsed arrays, as the
  client types already declared: `PUT /api/grocery/:id` (`source_dishes`),
  `POST /api/payments/manual/action` (intake arrays) and intake convert
  (`client.equipment`).
- Unknown `/api/*` paths always answer JSON 404 (Express did that only in
  production). Dates must be real `YYYY-MM-DD` values (400 otherwise);
  SQLite stored any text.
- Stripe events that name no known intake are acknowledged but not logged
  (`payment_events.account_id` is NOT NULL).
- The recipe URL importer also refuses hostnames that *resolve* to private,
  loopback or link-local addresses (custom DNS resolver), not only literal
  ones, and caps pages at 5 MB.

## Backups

Many managed Postgres hosts (Railway, Render, Neon, …) take their own
backups. `AUTO_BACKUP=true` also runs a daily `pg_dump --format=custom` into `BACKUP_DIR` (keeps 30); restore
with `pg_restore --clean --if-exists -d "$DATABASE_URL" <file>`.

The Docker image only contains `pg_dump` when it is built with
`AUTO_BACKUP=true` (on Railway a service variable of that name is also passed
to the build; `docker-compose.selfhost.yml` sets it); set `PG_MAJOR` to the Postgres server's major version (default
17), since `pg_dump` refuses to dump a newer server. Mount a volume at
`/app/backups` and set `RAILWAY_RUN_UID=0`: the server runs as a non-root
user and Railway volumes are owned by root.

## Shared state across instances

Used one-time token ids (`used_jtis`) and rate-limit counters
(`rate_limit_hits`) are in Postgres (migration 0003), so magic and reset
links are single-use and the auth / per-account limits hold across any
number of instances. Neither table holds tenant data (no RLS). Expired rows
are deleted hourly. `tests/shared_limits.rs` runs two app instances on one
database.

## SQLite → Postgres data migration

`tools/migrate-sqlite/` (a small standalone Node package: `better-sqlite3`
+ `pg`) copies a legacy SQLite file into a server-rs database. See its
[README](../tools/migrate-sqlite/README.md).

```bash
cd tools/migrate-sqlite && npm ci
node migrate-to-postgres.js --sqlite <backup.db> --database-url <postgres-url> [--dry-run] [--force]
```

- The target must already be migrated by server-rs (start the server against
  it once) and must be empty. A non-empty target is refused; `--force`
  TRUNCATEs the app tables first (for staging re-runs only). The Postgres
  user must be a superuser or have `BYPASSRLS` (Railway's `postgres` user is
  a superuser).
- Everything runs in **one transaction** with the app tables locked: either
  every row is copied and verified, or nothing is written. `--dry-run` does
  the whole run, then rolls back.
- Ids are kept, so cookies, links and intake edit tokens keep working. Every
  identity sequence is moved past the highest id ever used, including
  SQLite's AUTOINCREMENT high-water mark, so ids of deleted rows aren't reused.
- Conversions follow the Postgres column types: 0/1 → BOOLEAN, JSON text →
  JSONB, `YYYY-MM-DD HH:MM:SS` (UTC) or ISO-8601 → TIMESTAMPTZ, `YYYY-MM-DD`
  → DATE, dollars → NUMERIC. Lossless clean-ups are applied and listed:
  blank strings in typed columns → NULL or `'[]'`, date-only timestamps →
  midnight UTC, sub-cent amounts rounded.
- Anything that can't be converted (invalid JSON, impossible dates, NULL
  `account_id`) or that Postgres rejects (orphaned rows, duplicate emails) is
  **reported by table and id**, and the run fails. No row is silently
  dropped. Fix those rows in a *copy* of the SQLite file and run again.
- At the end it prints and checks row counts (and id sums) per table per
  account; any mismatch rolls everything back.

Exit codes: `0` done (or dry run passed); `1` problems found or verification
failed (nothing written); `2` refused (bad arguments, target not migrated or
not empty, missing privileges).

`npm run test:migrate` (root; also in CI) runs the conversion unit tests and
`tools/migrate-sqlite/test/e2e.mjs`: a legacy database recorded from the last
Express server (two accounts, starter library, clients, sessions, grocery,
invoices, intake and payments, `test/fixtures/legacy.sql`) is migrated into a
fresh server-rs database, both accounts sign in with their migrated password
hashes, and every GET must answer the JSON Express answered
(`test/fixtures/expected.json`): ids, money, dates and flags. `*_at`
timestamps are compared as instants, because server-rs always sends
`YYYY-MM-DD HH:MM:SS`. It also checks the sequences, the non-empty refusal,
`--dry-run`, and that bad rows are reported.

## Deploying

Self-hosting with Docker Compose (app + Postgres, HTTPS, mail, backups,
upgrades) is covered in [`docs/SELF_HOSTING.md`](../docs/SELF_HOSTING.md).

### The image

`server-rs/Dockerfile` builds one image with the React client (built with
Node 20) and the release binary (`SQLX_OFFLINE=true`, from the committed
`.sqlx/`), on `debian:bookworm-slim`, running as a non-root user. The build
context is the **repository root**. `Dockerfile.dockerignore` limits it to
`client/` and `server-rs/` (never `.db` files or `.env`).

```bash
docker build -f server-rs/Dockerfile -t platebook .
docker run -p 3000:3000 -e DATABASE_URL=... -e JWT_SECRET=... -e INTAKE_SECRET=... platebook
```

Defaults baked in: `NODE_ENV=production` (so `JWT_SECRET` is required and
error details are hidden), `CLIENT_DIST=/app/client/dist` and `PORT=3000`.
On start the server runs any pending migrations, then listens. `GET /health`
doesn't touch the database. SIGTERM lets in-flight requests finish.

### On a platform (Railway and similar)

The root `railway.json` selects that Dockerfile, health-checks `/health` and
restarts on failure. Leave the service's root directory at `/`, because the
build needs `client/`. Add a Postgres service, set `DATABASE_URL` to it, and
set the variables from the [README](../README.md#environment-variables).
Keep `EMAIL_*` empty on a staging copy of real data, so no client gets mail
from it, and use Stripe **test mode** keys there.

### Moving data from the old Express server

Copy the old SQLite file off its volume (back the volume up first), start
the new server once against an **empty** Postgres so it creates the schema,
then run `tools/migrate-sqlite` against that database: first with
`--dry-run`, then for real (see above). Keep `JWT_SECRET` and
`INTAKE_SECRET` from the old server so sessions and emailed intake links
keep working. `DB_PATH`, `CHEF_EMAIL` and `MAINTENANCE_MODE` belonged to the
Express server; server-rs ignores them.

## PDFs

The meal plan, grocery list, planner list and invoice PDFs are rendered here
with [Typst](https://typst.app) (`src/pdf/`): one template per document in
`src/pdf/templates/*.typ`, compiled into the binary, reading a `data.json`
the route builds with every string already formatted (dates, `$` amounts
through `money.rs`). Fonts are embedded (Liberation Sans, OFL, in
`assets/fonts/`), so rendering needs no system fonts or network.

| Route | Document |
| --- | --- |
| `GET /api/sessions/:id/pdf` | client meal plan |
| `GET /api/sessions/:id/grocery/pdf` | grocery list (stored, else generated) |
| `GET /api/planner/pdf?recipe_ids=1,2&serving_size=4&date=YYYY-MM-DD` | planner ingredient list |
| `GET /api/invoices/:id/pdf` | invoice (the saved invoice, laid out like the builder's preview) |

All four are tenant-scoped like every other route and share a per-account
limit (`acct.pdf`, 60 per 15 minutes). `PDF_OUT_DIR=<dir> cargo test --test pdfs`
also writes the test PDFs to `<dir>` for eyeballing. The client
(`client/src/utils/serverPdf.ts`) only downloads these; the browser renderer
(jsPDF + html2canvas) is gone.
