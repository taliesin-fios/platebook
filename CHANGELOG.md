# Changelog

All notable changes to Platebook are documented here.
Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### Added
- **Clients can pay the booking deposit online** (`PAYMENTS_MODE=stripe`). The client's booking page (the link in their confirmation email) shows **Pay $X deposit**. So does the screen right after they book. Each press opens a fresh Stripe Checkout (`POST /api/intake/:id/checkout`, checked with their edit token and rate-limited), so their link never goes stale. Stripe brings them back to that page, which says whether the payment went through or was cancelled, and shows "✓ paid" once the webhook confirms it.
- **Copy client's booking link** on each inquiry (`GET /api/intake/:id/client-link`), to send a client their page again.
- **Clients can pay invoices online** (`PAYMENTS_MODE=stripe`). The invoice email gets a **Pay online** button that opens the client's invoice page (`/invoice/:id?token=…`; `GET /api/invoices/:id/public`). From there they pay by card (`POST /api/invoices/:id/checkout`, rate-limited, a fresh Checkout each time, the open one expired first). The webhook marks the invoice paid and logs the event against it. The link's token is specific to invoices (an intake token never opens one) and lasts 180 days. The chef can copy the link from the invoice's action bar (`GET /api/invoices/:id/client-link`).
- **Migration `0010_invoice_payments.sql`:** `invoices.stripe_session_id` and `payment_events.invoice_id` (nullable, additive; composite foreign key, added `NOT VALID` then validated). Deleting an unpaid invoice keeps its payment events, detached, and the data export includes `invoice_id`.
- The client's view of their booking includes their own deposit status (`payment_status`, `booking_fee_amount`, `booking_fee_paid_at`). The chef's payment notes stay private.
- **`TRUST_PROXY_HOPS`** sets how many reverse proxies are trusted in `X-Forwarded-For` when rate limits pick the client address. The default `1` keeps the current behaviour (one proxy). Set `0` when clients connect directly, so the header is ignored; set `2` or more for a proxy chain. See `docs/SELF_HOSTING.md`.

### Fixed
- After paying, Stripe sent clients to `/intake/success`, a page that didn't exist (they landed on the sign-in screen). Cancelling sent them to their booking page without its token, which then failed to load.
- A waived deposit could still be charged online.
- Starting a new checkout while one was pending was refused, so an abandoned checkout couldn't be retried. Now the open one is expired first, so only the newest can be paid, and one already paid gives "it can take a minute to show" instead of a second charge.
- A client with an invoice's payment page open could pay the old amount after the chef changed the total, or pay an invoice the chef had since voided, marked paid or deleted. These edits now close the open Stripe Checkout first, and the client's next **Pay** opens one for the current total. An edit that leaves the total alone keeps the page open. If the client has just paid, the edit is refused with "it can take a minute to show" until the webhook marks the invoice paid. If Stripe can't be reached, nothing is saved and the chef is asked to try again (`502 STRIPE_UNAVAILABLE`).
- An inquiry's detail sheet showed empty **Contact**, **Household**, **Dietary** and **Scheduling** cards (just a heading) when the client left those questions blank. Now each card shows only when the client filled in at least one of its answers. Serving Size no longer reads "null servings" when it's blank.
- **Active Clients on the dashboard no longer counts the sample client** that onboarding adds. The line under it says so ("Sample client not counted"), or "No clients yet". **Oven and Stovetop show blank when they were never set**: the client edit form used to highlight "Yes" and "Gas", and the client page showed a blank oven as "No". Converting an inquiry no longer records the oven as available, since the booking form doesn't ask. Clients converted before this keep the "Yes" already saved.
- **Household answers a client skips stay blank.** The booking form started with Couple, 2 adults, 0 children and 4 servings already chosen, and the server filled in the same values when they were missing. So a client who skipped the section looked as if they'd answered it. The form now starts blank (tap a selected household type again to clear it), and the server stores what was sent, or nothing. Inquiries saved before this keep their values. When an inquiry with no answers becomes a client, the client page no longer shows "null / null" for adults and children, and the session wizard no longer says "null adults". The client's serving size still falls back to 4 so sessions have a portion count; the chef can change it.
- **The rest of the booking form no longer answers for the client either.** Spice tolerance ("No Preference"), containers ("Yes, I will") and groceries ("Client shops") started already chosen, and the server filled in the last two when they were missing. Now they start blank and stay blank if skipped. Tapping a chosen option again clears it. On the inquiry sheet, Containers shows only when the client answered it, and "No, chef provides" still shows as "Chef provides (+ fee)". The client edit form no longer highlights an option for a blank field, and saving it keeps the field blank.
- **A new account's dashboard no longer looks like something is wrong.** Avg Margin showed a red **0% · ↓ Review costs** before there was anything to measure. It now shows **—** with "No sessions this month", or "No fees set this month" when this month's sessions have no fee yet. The line under Collected said **All invoices paid** with no invoices at all; it now says **No invoices yet** until something is invoiced.
- **Deleting a session no longer erases its void invoices.** A session can only be deleted once its invoices are voided, and deleting it then removed those void invoices for good, invoice number and all. They now stay in Invoices as void records, with their payment history, just no longer linked to the session. Paid and unpaid invoices still block the delete, as before.
- The recipe-import URL check refuses more non-public addresses: IPv4 addresses embedded in IPv6 forms (NAT64 `64:ff9b::/96`, 6to4 `2002::/16`, IPv4-compatible `::/96`), the local NAT64 range `64:ff9b:1::/48`, Teredo `2001::/32`, site-local `fec0::/10`, and the reserved IPv4 ranges `192.0.0.0/24`, `198.18.0.0/15` and `240.0.0.0/4`.

---

## [0.2.0] — 2026-10-26

**Upgrading from 0.1.0:** 0.1.1 was prepared but never published, so its changes are part of this release. On start, the server applies three migrations: `0007_email_failures.sql`, `0008_intake_email_failures.sql` and `0009_intake_soft_delete.sql`. They only add nullable columns, so they're safe on a database with data, but take a backup first as usual.

### Security
- **A client's booking link no longer shows the chef's private notes.** The link a client gets to review or update their booking request (`/intake/:id?token=…`) returned the whole intake record, including the chef's **Internal Notes** (labelled "not visible to client"), payment notes and Stripe ids. Clients now get only what they filled in, plus their booking status (`ClientIntakeView`). The chef's own view is unchanged. If you wrote anything sensitive in Internal Notes, a client with their link could have seen it before this fix.

### Added
- **Email failures are remembered.** If emailing a meal plan or an invoice fails, the session and invoice pages show a banner with **Try again**, and the Sessions and Invoices lists show "⚠ Email failed", even after you leave the page. The next successful send clears it. Invoices also record when they were last emailed.
- **Booking email failures are remembered.** The confirmation to a client who books through your booking page, and your new-booking alert, were sent in the background and a failure only reached the server log. Now the Client Intakes page shows a note when either failed, and the inquiry shows which one and when. **Try again** on the inquiry resends the client's confirmation (`POST /api/intake/:id/resend-confirmation`, signed-in chef only).
- **Deleting an inquiry keeps it.** A new **Delete inquiry** button on the Client Intakes page moves it to a **Deleted** filter, with its payment history, where **Restore** brings it back. Before, there was no delete button, and the API's delete erased the inquiry. The API now soft-deletes (`DELETE /api/intake/:id` sets `deleted_at`; `POST /api/intake/:id/restore`; `GET /api/intake?deleted=1`). A deleted inquiry's client link and actions answer 404, and a completed Stripe payment for one restores it.
- **Payment history on each inquiry** (`GET /api/intake/:id/payments`): every manual payment action and Stripe event, with what happened, the amount and when.
- **Month-to-date trend on the dashboard.** The **Collected (MTD)** tile shows ▲/▼ and the percentage against the same days of last month (the 1st up to today's day number, in your time zone). It shows nothing when last month had nothing to compare with. `GET /api/costs/summary` gains `collected_prev_mtd`.
- **Migrations** (all nullable, additive):
  - `0007_email_failures.sql`: `sessions.pdf_email_failed_at`, `invoices.emailed_at` and `invoices.email_failed_at`.
  - `0008_intake_email_failures.sql`: `client_intakes.confirmation_emailed_at`, `confirmation_email_failed_at` and `chef_alert_failed_at`.
  - `0009_intake_soft_delete.sql`: `client_intakes.deleted_at`.

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
