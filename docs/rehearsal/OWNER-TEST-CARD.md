# Owner test card

This card is for testing locally. Nothing in it reaches a real inbox or takes a payment.

## Setup (one time, about 5 minutes)

```bash
npm run install:all
npm --prefix client run build
(cd server-rs && SQLX_OFFLINE=true cargo build)
# Postgres running; a user that may create databases:
export ADMIN_DATABASE_URL=postgres://postgres:<password>@localhost:5432/postgres
```

**Automated version (about 3 minutes):**

```bash
node docs/rehearsal/demo/run-all.mjs --out /tmp/platebook-demo
```

It ends with `29/29 checks passed` followed by four `PASS … clicks` lines, and exits with code 0.
One line reading `KNOWN DEFECT` (meal-plan PDF text) is expected. Results, screenshots and the video
are written to the `--out` folder.

**Hands-on version:** a fresh environment with the fictional week seeded and nothing else done to it,
left running until you press Ctrl-C.

```bash
node docs/rehearsal/demo/run-all.mjs --seed-only
```

| Address | What it is |
|---|---|
| http://localhost:4320 | Platebook with email, sent to the local mail sink |
| http://localhost:4321 | The same data, on a server with **no email configured** |
| http://127.0.0.1:2590 | Mail sink inbox. Nothing is relayed anywhere. |

Sign in as `robin.hale@example.com` with password `rehearsal-only-pass-2026`.

To make the mail server fail, run one of these in a second terminal:

```bash
curl -X POST 'http://127.0.0.1:2590/mode?set=down'     # unreachable
curl -X POST 'http://127.0.0.1:2590/mode?set=reject'   # refuses the recipient (550)
curl -X POST 'http://127.0.0.1:2590/mode?set=accept'   # back to normal
```

## Normal path (about 10 minutes)

| # | Do | You should see |
|---|---|---|
| 1 | Open :4320 and sign in | Dashboard showing Okafor (Mon), Lindqvist (Thu) and Castellano (two Fridays). Okafor shows a Peanuts / Tree Nuts warning. |
| 2 | Open the Okafor session, then the **Grocery** tab | garlic — 7 cloves, onion — 2, olive oil — 3 tbsp, each with the dishes it comes from |
| 3 | Tick an item and reload | It is still ticked |
| 4 | Overview → **📄 PDF** | The meal plan names the household, both dishes and "Allergies on file: Peanuts, Tree Nuts". It also contains invented reheating text (known defect P1-3). |
| 5 | Open the Okafor invoice (session → invoice). Add "Meal Prep Session" at $240 and "Grocery Shopping" at $86.40, then **Save** | Total $326.40 |
| 6 | **📧 Email** | "Invoice sent to ada.okafor@example.com (accepted by the mail server)". The status changes to Sent. A new message with the PDF appears at :2590. |
| 7 | **✅ Paid**, then reload | The invoice is Paid and has no Email button |
| 8 | Open the booking link from the dashboard in a private window and submit an inquiry for a made-up household | "We'll email a confirmation to …". The confirmation appears at :2590. |

## Deliberate failure checks (about 5 minutes)

| # | Do | Pass if |
|---|---|---|
| F1 | `set=down`, then email the Okafor meal plan | The message says it **was not sent**. A red banner offers **Download PDF** and **Try again**. After a reload the banner is still there and "Last sent" hasn't changed. :2590 has no new message. |
| F2 | `set=accept`, then press **Try again** in the banner | "…(accepted by the mail server)". Exactly **one** new message arrives at :2590. The banner is gone. |
| F3 | Click **✉️ Email** five times quickly | One message at :2590. A second send within 10 seconds says "was sent a few seconds ago, so it wasn't sent again". |
| F4 | Open the same session on **:4321**, then Email | "Email isn't set up on this Platebook server…". The banner offers **Download PDF only**, with no Try again. Nothing arrives at :2590. |
| F5 | On :4321, sign out and use "Email me a sign-in link" for `robin.hale@example.com`, then for `nobody@example.com` | Both pages are identical and say no link was sent. Nothing arrives at :2590. |
| F6 | On :4320, do the same as F5 | Both pages say "we've asked the mail server…" and are otherwise identical. Only Robin's address gets a "login link" message at :2590. |
| F7 | `set=reject`, then email a Draft invoice | "refused… check the email address". After a reload the status is still **Draft**. |
| F8 | Sign up a second chef in a private window. Paste one of Robin's `/sessions/<id>` or `/invoices/<id>` URLs | Not found, and none of Robin's data appears. (The session page may show a loading skeleton forever; that's defect P2-11.) |

Remember to run `set=accept` when you're done. Press Ctrl-C in the `run-all` terminal to stop
everything.

## What this card does not prove

It does not prove delivery to a real inbox, a real SMTP provider's TLS and login, behaviour on a
physical phone, or demand from real chefs.

For a first real-inbox check after deploying, send one invoice to an address you own. Then confirm
the server's startup log says "SMTP transporter verified", not "Email is not configured".
