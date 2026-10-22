# Roadmap

Planned work and known gaps. Ideas and pull requests are welcome: open an issue first for anything large.

## Known issues

- **Emails are fire-and-forget.** A failed SMTP send is logged but never retried, and the chef isn't told in the app.
- **Deletion isn't consistent.** Invoices soft-delete, recipes and clients are deactivated, and intakes and templates are hard-deleted. Deleting an intake loses that lead's history.
- **Allergen detection is advisory only.** It matches keywords in ingredient names (`detectAllergens`). It is not a certified check, and a chef must always confirm allergens with the client.

## Polish

- Dashboard: a month-to-date revenue trend arrow.
- Email reliability: an "email failed" banner on session and invoice cards, plus one retry with backoff.
- Soft-delete intakes so lead history is kept, and show `payment_events` as a payment timeline on the intake page.

## Features

| Feature | Notes |
|---|---|
| Online deposits, end to end | Stripe Checkout and webhooks are in place (`PAYMENTS_MODE=stripe`); the client-side success page and the payment link on the intake screen need finishing. |
| Client portal (read-only) | A signed-token link for each client: their upcoming session, current menu, past meal plans and invoices. No login needed. |
| Menu feedback loop | A post-session email asking for 1–5 stars per dish. Ratings are stored per client and fed into AI menu suggestions. |
| PWA | Manifest and service worker, so the last-viewed session and grocery list work in kitchens with poor reception. |
| Invoice payment links | A Stripe payment link per invoice, marked paid automatically by the existing webhook. |
| P&L / tax export | Monthly profit and loss (revenue, ingredients, mileage) for tax time. |

Done so far: session reminder emails, recurring sessions, time-aware calendar conflicts, a real "this week" dashboard stat, a share/print/copy grocery list, and consistent margin colors.
