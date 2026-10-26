# Roadmap

Planned work, in priority order. Ideas and pull requests are welcome: open an issue first for anything large.

**How it's ordered:** first, nothing should fail silently or lose a chef's data. Next comes finishing what's half-built. New features come last. Each milestone is a release. Work on the next milestone starts only when the current one has shipped.

Sizes are rough: **S** is under a day, **M** is a few days, **L** is a week or more.

## Now: ship what's already merged

| # | Item | Size | Status |
|---|---|---|---|
| 1 | Release v0.2.0 to self-hosters: everything under v0.2 and the unpublished v0.1.1 in `CHANGELOG.md` (including a privacy fix and migrations 0007–0009). Versions are set to 0.2.0; tag `v0.2.0` on `main` to publish the image. | S | Tag and image pending |
| 2 | Release v0.3.0: online deposits and invoice payments (migration 0010). First, one end-to-end Stripe test-mode run of both (book and pay a deposit, email and pay an invoice), then bump versions and tag. | S | After v0.2.0 |

## Later: features, ordered by what chefs ask for

| Feature | Size | Notes |
|---|---|---|
| P&L / tax export | M | Monthly profit and loss (revenue, ingredients, mileage) for tax time. Mostly reads existing data, so it's the lowest-risk feature. |
| Client portal (read-only) | L | A signed-token link for each client: their upcoming session, current menu, past meal plans and invoices. No login needed. Adds public routes, so it needs a security review. |
| Menu feedback loop | M | A post-session email asking for 1–5 stars per dish. Ratings are stored per client and fed into AI menu suggestions. |
| PWA | L | Manifest and service worker, so the last-viewed session and grocery list work in kitchens with poor reception. |

## Known limitations

- **Allergen detection is advisory only.** It matches keywords in ingredient names (`detectAllergens`). It is not a certified check, and a chef must always confirm allergens with the client.

## Done

- **v0.3 (unreleased):** clients pay the booking deposit online from their booking page and invoices from their invoice link (Stripe), with working return pages, retries that can't double-charge, the webhook marking them paid, and copyable client links for the chef.
- **v0.2.0 (tag pending):** failed booking emails (the client's confirmation and the chef's new-booking alert) are remembered and shown on the Client Intakes page, with **Try again** for the confirmation. A client's booking link no longer exposes the chef's private notes. Inquiries can be deleted and restored without losing their history, and each shows its payment history. The deletion rule for every table is in `ARCHITECTURE.md`. The dashboard's Collected (MTD) tile shows a trend against the same days of last month.
- **v0.1.1 (never published; part of v0.2.0):** failed meal-plan and invoice emails are remembered and shown with **Try again**, every email gets one automatic retry, and "today", "this month" and "overdue" follow the account's time zone.
- **v0.1.0:** session reminder emails, recurring sessions, time-aware calendar conflicts, a real "this week" dashboard stat, a share/print/copy grocery list, and consistent margin colors.
