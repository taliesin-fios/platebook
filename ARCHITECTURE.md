# Platebook — Architecture

> A technical reference for contributors and code reviewers.

---

## Stack at a Glance

| Layer | Technology |
|-------|-----------|
| Frontend | React 19 + TypeScript (strict) + Vite 7 + Tailwind CSS 4 |
| Routing (client) | React Router 7 |
| Backend | Rust: Axum + Tokio (`server-rs/`) |
| Database | PostgreSQL 14+ via SQLx, Row-Level Security per account |
| Auth | Accounts: email + password (bcrypt) or magic link → httpOnly JWT cookie `{ accountId, email }` |
| AI | Anthropic Claude API (reqwest) |
| PDF | Typst, rendered on the server (`server-rs/src/pdf/`); the client only downloads them |
| Email | lettre (SMTP); logged when `EMAIL_HOST` is empty |
| Payments | Mode-gated: `manual` (default) / `stripe` / `disabled` |
| Deploy | Docker image (`server-rs/Dockerfile`); self-hosting with `docker-compose.selfhost.yml` (`docs/SELF_HOSTING.md`) |
| Data migration | `tools/migrate-sqlite/`: one-time copy of the old Express server's SQLite file into Postgres |

---

## Repository Layout

```
platebook/
├── client/                         # Vite SPA
│   ├── index.html                  # viewport-fit=cover for iOS safe-area
│   └── src/
│       ├── main.tsx                # React root, BrowserRouter
│       ├── App.tsx                 # Route table, auth gate, BottomNav/SideNav layout
│       ├── constants.ts            # Shared enums, allergen data, feasibility calc
│       ├── types/                  # API response shapes, one file per domain
│       ├── pages/                  # Route-level components (see Pages below)
│       ├── components/             # Shared: Toast, SideNav, BottomNav, PageHeader, …
│       ├── hooks/useApi.ts         # typed fetch wrapper (apiCall<T>) — throws on !ok
│       └── utils/
│           ├── serverPdf.ts        # download / attach the server-rendered PDFs
│           └── quantityScale.ts    # Recipe scaling math
│
├── server-rs/                      # Rust API (see server-rs/README.md)
│   ├── src/
│   │   ├── main.rs                 # Boot: config, pool, migrations, listen, SIGTERM
│   │   ├── app.rs                  # Router: /health, middleware, /api routes, SPA
│   │   ├── auth.rs                 # JWT cookie auth, is_public_route allowlist, one-time tokens
│   │   ├── db.rs                   # Pool, MIGRATOR, Tenant extractor / begin_tenant (RLS)
│   │   ├── ownership.rs            # owned / owned_ids (ids from the request)
│   │   ├── accounts.rs             # accounts, slugs, get_settings
│   │   ├── routes/                 # One module per resource (same URLs as the old Express routers)
│   │   ├── pdf/                    # Typst engine, data builders, templates/*.typ
│   │   ├── email.rs                # lettre wrappers (magic link, intake, invoice, meal plan)
│   │   ├── stripe.rs, anthropic.rs # External APIs over reqwest
│   │   ├── money.rs                # integer-cents money helpers
│   │   ├── rate_limit.rs           # Postgres-backed rate limits (per IP / per account)
│   │   └── error.rs                # AppError, err_msg (prod-safe errors)
│   ├── migrations/                 # 0001 schema … 0005 recurring sessions, 0006 session reminders, 0007 email failures
│   ├── tests/                      # cargo integration tests (HTTP, RLS, PDFs, tenancy)
│   └── scripts/                    # cargo/dev launchers, db-setup
│
├── tests/http/                     # Black-box HTTP suites (api.smoke, auth, tenancy) against server-rs
└── tools/migrate-sqlite/           # SQLite → Postgres data migration (+ end-to-end test with a legacy fixture)

---

## Entry Points

### Server — `server-rs/src/main.rs`

Boot sequence:

1. Read config from the environment (and `server-rs/.env` via dotenvy)
2. Connect the Postgres pool and run pending migrations (`db::MIGRATOR`)
3. Start the SMTP check, the optional `pg_dump` scheduler and hourly pruning of rate-limit / token rows
4. Build the router (`app::build`): `GET /health` (no database), security headers, CORS (gated to `CLIENT_URL` in production), auth middleware with the public allowlist, the `/api` routes (the Stripe webhook reads the raw body), JSON 404 for unknown `/api/*`, and the React build when `CLIENT_DIST` is set
5. Listen on `0.0.0.0:PORT`; SIGTERM lets in-flight requests finish

### Client — `client/src/main.tsx` → `App.tsx`

`App.tsx` wraps everything in `<BrowserRouter>`. It checks for an authenticated session (stored as a cookie; the client verifies via `GET /api/auth/verify`). Unauthenticated users are redirected to `/login`. Authenticated users see the full app with `<SideNav>` (desktop) and `<BottomNav>` (mobile).

---

## Data Flow

### Typical API Request

```
React page
  └── fetch('/api/resource')          # .ok check before .json()
        └── auth middleware           # verify httpOnly JWT cookie + account row (or public allowlist)
              └── handler (Tenant)    # transaction: SET LOCAL ROLE chef_app + app.account_id
                    └── Json(...)     # AppError → { error, code }; err_msg() hides internals in prod
```

### Accounts & Authentication

Every chef business is an `accounts` row (`email`, bcrypt `password_hash`, public `slug`, `onboarding_completed_at`).

```
Sign up   POST /api/auth/signup {email, password, business_name} → account + settings row, session cookie
          SIGNUP_MODE=open (default) | first (only while no account exists) | closed → 403 SIGNUP_CLOSED;
          GET /api/auth/signup → { open } so the client hides its sign-up links
Sign in   POST /api/auth/login  {email, password}                → session cookie
Magic     POST /api/auth/request-link → emailed single-use link (15 min) → GET /api/auth/verify
Reset     POST /api/auth/forgot-password → emailed single-use link (30 min) → POST /api/auth/reset-password
Session   httpOnly authToken JWT { accountId, email }, 7 days; POST /api/auth/logout clears it.
          Every request also checks the account still exists and the token isn't older than
          accounts.sessions_valid_after (set by a password reset, which ends every other session).
```

The client `AuthGate` (App.tsx) sends signed-out visitors to `/welcome` (or `/login` for deep links) and signed-in accounts that haven't finished setup to `/onboarding`.

### Multi-tenancy

- Top-level tables carry `account_id NOT NULL`: `clients`, `recipes`, `sessions`, `session_templates`, `client_intakes`, `invoices`, `recipe_import_log`, `payment_events`, plus `settings` (one row per account, `account_id` primary key, including booking policy: `deposit_cents`, `cancellation_hours`, `payment_methods`, `booking_terms`, `response_time_text`, `service_types`, `invoice_prefix`).
- Child tables (`ingredients`, `session_dishes`, `grocery_items`, `invoice_line_items`) are scoped through their parent row.
- **Row-Level Security** (`migrations/0002_row_level_security.sql`): handlers run inside a `db::Tenant` transaction that switches to the `chef_app` role and sets `app.account_id`, so a query only ever sees (and can only write) that account's rows. Cross-table references are composite foreign keys on `(account_id, id)`.
- Handlers still filter on `account_id` explicitly, and ids from the request are checked with `ownership::owned` / `owned_ids` (404 / 400), so RLS is the backstop.
- Pre-tenant lookups (login, the auth middleware's session check, public profile and intake by slug, intake by token, Stripe webhook, and the reminder job's scan for due sessions) use the pool as the connecting user for that one lookup only.
- Background jobs (`main.rs`): hourly pruning of rate-limit windows and used token ids; every 5 minutes `reminders::run_once`, which sends due reminders one at a time: each in a transaction that locks the session (`FOR UPDATE SKIP LOCKED`, safe with several instances), switches to the owning tenant (`db::enter_tenant`), marks it sent, sends, and commits only after the send. A crash or failed send rolls back for a retry (at-least-once).
- Invoice numbers (`<PREFIX>-<YEAR>-0001`) and template names are unique per account.
- AI, import, CSV, PDF and client-email rate limits are keyed by account (the Anthropic key is platform-wide). Public intake submission is limited per IP.
- Emails to clients attach PDFs the server renders itself; the email routes never accept file content from the request.


### Client Intake Flow (Public)

```
1. Client visits /book/<slug> (public; legacy /intake/new = account #1)
2. POST /api/intake { slug, … } — stored under that chef's account; emails chef + client
3. Server returns { id, edit_token } — HMAC-signed, 30-day expiry
4. Client can edit via /intake/:id?token=... (PUT /api/intake/:id — token verified server-side).
   Token holders get `ClientIntakeView` (their answers and booking status), never the
   chef's internal/payment notes, Stripe ids or bookkeeping; the owning chef gets the full row.
5. Chef reviews in IntakeAdmin → converts to Client record if accepted
```

### Invoice State Machine

```
draft ──→ sent ──→ paid
  │         │
  └────────→┴──→ void
```

- `paid` and `void` are **terminal states** — the server rejects status changes with `409`.
- The email-invoice action auto-transitions `draft → sent` as a side-effect.
- Sessions with a non-void invoice cannot be deleted (returns `409 INVOICE_ATTACHED`).

---

## Database Schema (key tables)

| Table | Purpose |
|-------|---------|
| `clients` | Client profiles; soft-deleted via `is_active = false` |
| `sessions` | Cook sessions; linked to one client |
| `session_dishes` | Many-to-many: sessions ↔ recipes |
| `recipes` | Recipe library with prep/cook times, allergen flags; ingredients in `ingredients` |
| `invoices` | Invoice records; linked to one session |
| `invoice_line_items` | Line items for each invoice |
| `client_intakes` | Intake form submissions |
| `payment_events` | Append-only audit log for payment actions |
| `session_templates` | Saved recipe sets for quick session creation |
| `grocery_items` | Generated grocery lists per session |
| `accounts` | One row per chef business: login email, password hash, booking slug, onboarding state |
| `settings` | Per-account business config and booking policy (`account_id` primary key) |
| `used_jtis`, `rate_limit_hits` | Single-use token ids and rate-limit windows shared across instances (no tenant data) |

List columns (`allergens`, `dietary_styles`, `liked_proteins`, `equipment`, `recipe_ids`, …) are `JSONB`. Flags are `BOOLEAN` (sent as `0`/`1` in JSON), dates `DATE`, timestamps `TIMESTAMPTZ`, dollar amounts `NUMERIC(12,2)`. The full SQLite → Postgres mapping is in [`server-rs/README.md`](server-rs/README.md#schema-differences-from-sqlite).

---

## Key Dependencies

### Server (`server-rs/Cargo.toml`)

| Crate | Role |
|---------|------|
| `axum` + `tokio` + `tower-http` | HTTP framework, runtime, CORS / tracing layers |
| `sqlx` (Postgres) | Async queries, migrations, compile-time checked SQL (`.sqlx/` cache) |
| `jsonwebtoken` | JWT sign/verify for auth cookies (same format as the old Express server) |
| `bcrypt` | Password hashes (reads the old bcryptjs `$2a$` hashes) |
| `lettre` | SMTP email |
| `reqwest` | Claude API, Stripe REST, recipe URL import (SSRF-guarded) |
| `typst` | Server-side PDFs |
| `rust_decimal` | Money |
| `ts-rs` | Generates the client's API types |

### Client

| Package | Role |
|---------|------|
| `react` + `react-dom` | UI framework |
| `react-router` | Client-side routing |
| `recharts` | Cost dashboard charts |
| `date-fns` | Date formatting |
| `tailwindcss` + `@tailwindcss/vite` | Utility-first CSS; theme in the `@theme` block of `index.css` |
| `typescript` | Strict type-checking (`npm run typecheck`, also run by `npm run build`) |

---

## Authentication Guard — Public Route Allowlist

The following routes bypass authentication (see `auth::is_public_route` in `server-rs/src/auth.rs`, unit-tested).
Public (no cookie required; `req.account` is still set when signed in):

| Method | Path |
|---|---|
| POST | `/api/auth/signup`, `/login`, `/request-link`, `/forgot-password`, `/reset-password`, `/logout` |
| GET | `/api/auth/verify`, `/api/auth/signup` (`{ open }`: may an account be created now, per `SIGNUP_MODE`) |
| GET | `/api/public/:slug/settings` |
| GET | `/api/settings/public` (legacy: account #1) |
| POST | `/api/intake` |
| GET, PUT | `/api/intake/:id` (owner's session **or** HMAC token) |
| POST | `/api/payments/webhook` |
| * | non-`/api/*` paths (SPA + static files) |

Change the function and this table together.

---

## Error Handling

Handlers return `Result<_, AppError>`. Errors are JSON `{ "error", "code" }`; internal errors go through `error::err_msg` (`server-rs/src/error.rs`):

```rust
// In production (NODE_ENV=production): "Internal server error"
// Otherwise: the error's text (full detail)
pub fn err_msg(err: &dyn std::fmt::Display) -> String
```

Every handler runs in one transaction, so a failed request never leaves partial writes.

On the client, every `fetch()` call checks `r.ok` before calling `r.json()`, preventing the app from trying to parse HTML error pages as JSON.

---

## Payments Architecture

The payments system is mode-gated via `PAYMENTS_MODE` env var:

```
PAYMENTS_MODE=manual   → chef records offline; no checkout UI (default)
PAYMENTS_MODE=disabled → status display only; checkout returns 503
PAYMENTS_MODE=stripe   → Stripe Checkout; requires STRIPE_SECRET_KEY
```

Mode logic is in `server-rs/src/routes/payments.rs`; Stripe REST calls and webhook signature checks are in `server-rs/src/stripe.rs`. Switching modes requires only an env var change — no code changes.

---

## Mobile / iOS Considerations

- `client/index.html` includes `viewport-fit=cover` — required for `env(safe-area-inset-bottom)` to return non-zero values on iOS.
- Bottom action bars use the `.fixed-action-bar` CSS class (`client/src/index.css`) which applies `bottom: calc(60px + env(safe-area-inset-bottom))` on mobile and `bottom: 0` on desktop (≥768px).
- The `<BottomNav>` component applies `padding-bottom: env(safe-area-inset-bottom)` directly.

---

## Verification Checklist

Manual tests to run after changes:

1. **Error leakage** — set `NODE_ENV=production`, send an invalid request to any `/api/*` route; response body must say `"Internal server error"`, not a SQL message.
2. **Invoice terminal state** — attempt a status change on a `paid` or `void` invoice → expect `409 STATUS_LOCKED`.
3. **Session idempotency** — POST `/api/sessions/:id/complete` twice → second call returns `409 ALREADY_COMPLETED`.
4. **Invoice-attached delete** — try to DELETE a session with a non-void invoice → expect `409 INVOICE_ATTACHED`.
5. **NaN guard** — send `days_offset=abc` to `POST /api/sessions/:id/clone` → expect `400 VALIDATION_ERROR`.
6. **Fetch resilience** — kill the API server, navigate the React app → should show error states, not blank pages or JS exceptions.
7. **iOS safe area** — open on an iPhone with notch; action bars must clear the home indicator bar.
8. **Auth gate** — request any authenticated API endpoint without a cookie → expect `401 UNAUTHORIZED`.
