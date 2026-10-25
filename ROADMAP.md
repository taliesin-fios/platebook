# Roadmap

Planned work, in priority order. Ideas and pull requests are welcome: open an issue first for anything large.

**How it's ordered:** first, nothing should fail silently or lose a chef's data. Next comes finishing what's half-built. New features come last. Each milestone is a release. Work on the next milestone starts only when the current one has shipped.

Sizes are rough: **S** is under a day, **M** is a few days, **L** is a week or more.

## Now: v0.1.1, ship what's already merged

| # | Item | Size | Status |
|---|---|---|---|
| 1 | Release the email-reliability and account time-zone fixes (see `CHANGELOG.md`) to self-hosters: align the app and package versions, tag `v0.1.1`, publish the image. | S | In progress |
| 2 | Small doc fixes (the default branch is `main`). | S | In progress |

## Next: v0.2, nothing fails silently, nothing is lost

| # | Item | Size | Notes |
|---|---|---|---|
| 1 | **Alerts for failed booking emails** | M | Today the booking confirmation to a client and the new-booking alert to the chef are retried once, then only logged. Remember the failure on the intake and show it with **Try again**, the same way meal-plan and invoice emails already work. Likely a small additive migration. |
| 2 | **Soft-delete intakes and add a payment timeline** | M | Deleting an intake removes it for good, along with that lead's history. Keep it instead and hide it, and show `payment_events` as a timeline on the intake page. Write down the deletion rule for each table in `ARCHITECTURE.md` (invoices soft-delete, recipes and clients are deactivated, templates are hard-deleted). Additive migration. |
| 3 | Dashboard: a month-to-date revenue trend arrow | S | Polish. |

## Then: v0.3, payments end to end

| # | Item | Size | Notes |
|---|---|---|---|
| 1 | **Online deposits** | M | Stripe Checkout and webhooks are in place (`PAYMENTS_MODE=stripe`). Still needed: the client-side success page and the payment link on the intake screen. Builds on the payment timeline from v0.2. |
| 2 | Invoice payment links | M | A Stripe payment link per invoice, marked paid automatically by the existing webhook. |

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

- **v0.1.1 (unreleased):** failed meal-plan and invoice emails are remembered and shown with **Try again**, every email gets one automatic retry, and "today", "this month" and "overdue" follow the account's time zone.
- **v0.1.0:** session reminder emails, recurring sessions, time-aware calendar conflicts, a real "this week" dashboard stat, a share/print/copy grocery list, and consistent margin colors.
