# Changelog

All notable changes to Platebook are documented here.
Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

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
