# Changelog

All notable changes to Platebook are documented here.
Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### Added
- **Booking email failures are remembered.** The confirmation to a client who books through your booking page, and your new-booking alert, were sent in the background and a failure only reached the server log. Now the Client Intakes page shows a note when either failed, and the inquiry shows which one and when. **Try again** on the inquiry resends the client's confirmation (`POST /api/intake/:id/resend-confirmation`, signed-in chef only).
- **Migration `0008_intake_email_failures.sql`:** `client_intakes.confirmation_emailed_at`, `confirmation_email_failed_at` and `chef_alert_failed_at` (nullable, additive).

### Security
- **A client's booking link no longer shows the chef's private notes.** The link a client gets to review or update their booking request (`/intake/:id?token=…`) returned the whole intake record, including the chef's **Internal Notes** (labelled "not visible to client"), payment notes and Stripe ids. Clients now get only what they filled in, plus their booking status (`ClientIntakeView`). The chef's own view is unchanged. If you wrote anything sensitive in Internal Notes, a client with their link could have seen it before this fix.

---

## [0.1.1] — 2026-10-25

**Upgrading:** this release adds migration `0007_email_failures.sql`, which the server applies on start. It only adds nullable columns, but take a backup first as usual.

### Added
- **Email failures are remembered.** If emailing a meal plan or an invoice fails, the session and invoice pages show a banner with **Try again**, and the Sessions and Invoices lists show "⚠ Email failed", even after you leave the page. The next successful send clears it. Invoices also record when they were last emailed.
- **Migration `0007_email_failures.sql`:** `sessions.pdf_email_failed_at`, `invoices.emailed_at` and `invoices.email_failed_at` (nullable, additive).

### Changed
- Every outgoing email gets one automatic retry, 2 seconds after a transient SMTP failure (a 4xx reply, a timeout or a dropped connection). Permanent rejections (5xx) aren't retried.
- Emailing a meal plan or invoice no longer holds a database transaction open while the PDF renders and the email sends.

### Fixed
- "Today", "this month" and "overdue" follow the account's time zone (`settings.timezone`, set in Settings → Reminder Emails; UTC when unset), not the server's UTC date. In US evenings the month-to-date totals no longer roll over early, invoices no longer turn overdue a day early, and new invoices are no longer dated tomorrow. This covers:
  - the cost summary: MTD, YTD, overdue count and collected this month
  - the `/api/costs/sessions` window
  - the `upcoming` sessions fallback
  - new-invoice issue and due dates
  - the planner PDF's default date
- New helper `accounts::local_clock`. An unknown stored zone falls back to UTC. No migration.

---

## [0.1.0] — First public release

Platebook is a self-hostable kitchen office for home and personal chefs, released under the PolyForm Shield License 1.0.0.

### Features
- **Accounts:** many chefs per server, each with their own private data (Postgres Row-Level Security). Sign in with a password or a magic link. A five-step onboarding wizard.
- **Clients:** household details, allergies and preferences.
- **Booking:** a public booking page per chef (`/book/<slug>`) with an intake form, optional deposits (manual or Stripe), and conversion of an intake into a client.
- **Sessions:** scheduling with a calendar and conflict detection, recurring sessions (every 1–4 weeks), menus, and feasibility checks.
- **Reminders (opt-in):** about 24 hours before a session, a reminder email to the client and a prep sheet to the chef, in the chef's time zone.
- **Recipes:** a library with scaling by servings, allergen hints, import from a URL or pasted text (optional Claude API), and an optional starter library.
- **Grocery lists:** built from session menus and grouped by store section; share, print or copy them.
- **Documents:** meal-plan and invoice PDFs rendered on the server and emailed to clients from the chef's own business details.
- **Money:** invoices (with mileage lines), ingredient costs, margins and profitability, plus CSV export.
- **Self-hosting:** `docker-compose.selfhost.yml` (app + Postgres 18), a guide in [docs/SELF_HOSTING.md](docs/SELF_HOSTING.md), and a Docker image published to GHCR on every release tag.
- **Who can sign up:** `SIGNUP_MODE=open` (the default), `first` (only the first account, for one chef on their own server) or `closed`. `GET /api/auth/signup` tells the client, which hides its sign-up links.
- **Operations:** one Docker image, migrations on start, optional daily `pg_dump` backups, and per-account rate limits.

### Changed
- With `NODE_ENV=production` the server refuses to start without `INTAKE_SECRET`, as it already did for `JWT_SECRET`. Without one, client intake links were signed with a public default.
- The starter recipe library is a new set of 20 recipes and 4 menu templates.

See [ROADMAP.md](ROADMAP.md) for known issues and what's next.
