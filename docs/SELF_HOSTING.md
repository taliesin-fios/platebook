# Self-hosting Platebook

This guide runs Platebook on your own server with Docker Compose: one container for the app and one for Postgres, behind a reverse proxy that handles HTTPS.

## What you need

- A Linux server or VPS (1 vCPU and 1 GB RAM is plenty for a few chefs) with Docker and the Compose plugin.
- A domain or subdomain pointing at it, e.g. `platebook.example.com`.
- Optional:
  - an SMTP mailbox, to send meal plans, invoices and sign-in links;
  - a Stripe account, for online deposits;
  - nothing for AI: each chef adds their own provider and key in Settings. You can add a shared Anthropic key.

## Quick start

```bash
git clone https://github.com/taliesin-fios/platebook.git
cd platebook
cp .env.selfhost.example .env
```

Fill in `.env`. Generate each secret with `openssl rand -hex 32`.

| Variable | Notes |
|---|---|
| `POSTGRES_PASSWORD` | Letters and digits only: it goes inside the database URL. |
| `JWT_SECRET` | Signs sign-in cookies and one-time links. Changing it signs everyone out. |
| `INTAKE_SECRET` | Signs the edit links emailed to clients after they book. Changing it breaks old links. |
| `CLIENT_URL` | The public `https://` address. It's used in emailed links and Stripe redirects. |
| `SIGNUP_MODE` | `first` (default in this file) lets only the first person sign up: you. Use `open` to let anyone create an account, or `closed` to allow no new accounts. |

Then start it:

```bash
docker compose -f docker-compose.selfhost.yml up -d --build
docker compose -f docker-compose.selfhost.yml logs -f app   # wait for "listening"
```

The app now answers on `http://127.0.0.1:3000` on the server. It isn't reachable from outside until you add the reverse proxy below. Open your domain, create your account, and the onboarding wizard takes it from there.

To use the published image instead of building, replace the `build:` block in `docker-compose.selfhost.yml` with `image: ghcr.io/taliesin-fios/platebook:latest`.

## HTTPS with a reverse proxy

Run Platebook behind **exactly one** reverse proxy and don't expose port 3000 directly. Sign-in cookies are marked `Secure`, and rate limits use the client address the proxy adds to `X-Forwarded-For`.

`TRUST_PROXY_HOPS` tells the app how many proxies to trust in that header:

- `1` (the default): one reverse proxy, as in this guide.
- `2` or more: a chain, for example a CDN in front of Caddy. Use the number of proxies that each append to `X-Forwarded-For`.
- `0`: no proxy, clients connect to the app directly. The header is then ignored and rate limits use the connection's address.

If the number is too high, clients can choose the address their rate limit counts against. If it is too low, everyone behind the outer proxy shares one limit.

[Caddy](https://caddyserver.com) gets and renews certificates by itself. Its `/etc/caddy/Caddyfile`:

```
platebook.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

With nginx, proxy to `http://127.0.0.1:3000`, set `proxy_set_header X-Forwarded-For $remote_addr;` and `client_max_body_size 10m;`, and get a certificate with certbot.

## Email

Without `EMAIL_HOST`, emails are written to the app log instead of sent. Without email, sign-in links, password resets and the optional session reminders can't reach anyone. Set `EMAIL_HOST`, `EMAIL_PORT` (587 for STARTTLS), `EMAIL_USER`, `EMAIL_PASS` and `EMAIL_FROM` from your mail provider.

Client emails use the chef's business email (Settings) as `Reply-To`. To keep mail out of spam, send from a domain whose SPF, DKIM and DMARC records include your mail provider.

## Payments and AI (optional)

- **Deposits:** `PAYMENTS_MODE=manual` (the default) records deposits the chef collects by hand. `stripe` (beta) takes them online and needs `STRIPE_SECRET_KEY` and `STRIPE_WEBHOOK_SECRET`. Add a Stripe webhook to `https://<your domain>/api/payments/webhook` for `checkout.session.completed`, `payment_intent.payment_failed` and `charge.refunded`. Clients pay from their booking page and Stripe returns them there, so `CLIENT_URL` must be your public address. Try it with test-mode keys first. It uses one Stripe account for the whole server, so it suits a server for one chef: with several chefs, every client payment would go to that account. `disabled` hides deposits.
- **AI:** each chef chooses a provider (Anthropic, OpenAI, Google Gemini, OpenRouter, Mistral, Groq or any OpenAI-compatible service) in **Settings → AI Assistant** and enters their own API key, billed to them. Keys are encrypted with a key derived from `JWT_SECRET`: if you change `JWT_SECRET`, chefs have to enter their keys again. Optionally, `ANTHROPIC_API_KEY` gives accounts without their own settings a shared Claude key, billed to you. Chefs' custom base URLs must be public HTTPS addresses; set `AI_ALLOW_PRIVATE_URLS=true` only if you trust every account on the server and want them to reach a model server on your network. AI requests are rate-limited per chef account.

## Backups and restore

Postgres data lives in the `db-data` Docker volume. With `AUTO_BACKUP=true` the app writes a daily `pg_dump` at `AUTO_BACKUP_HOUR` (server local time) into the `backups` volume and keeps 30. Copy those off the machine regularly.

```bash
# take a backup now
docker compose -f docker-compose.selfhost.yml exec -T db pg_dump -U platebook -Fc platebook > platebook-$(date +%F).dump

# restore one (stop the app first)
docker compose -f docker-compose.selfhost.yml stop app
docker compose -f docker-compose.selfhost.yml exec -T db pg_restore -U platebook -d platebook --clean --if-exists < platebook-2026-10-21.dump
docker compose -f docker-compose.selfhost.yml start app
```

## Upgrading

```bash
git pull
docker compose -f docker-compose.selfhost.yml up -d --build
```

Database migrations run automatically when the app starts. Take a backup first. Read [CHANGELOG.md](../CHANGELOG.md) for anything that needs action.

## Who can sign up

Each account is a separate chef business. Its clients, recipes and money are private to it, enforced by Postgres row-level security. `SIGNUP_MODE` decides who may create one:

- `first`: a single chef on their own server. Sign-up closes after the first account.
- `closed`: no new accounts. Existing ones keep working.
- `open`: anyone who can reach the server can sign up. Use it only if you mean to host other chefs, and remember the license: you may not offer Platebook as a paid or competing service (see [LICENSE](../LICENSE)).

## A public demo

To show Platebook to others (a portfolio link, a demo for a chef you're talking to), set `DEMO_MODE=true`. The welcome and sign-in pages then offer **Try the demo**, which opens a shared account, "Demo Kitchen", without a password. It holds the starter recipes, five sample clients, two months of sessions, invoices (paid, sent, overdue and a draft) and three booking inquiries, all dated around today.

- Visitors can change anything. On start and every `DEMO_RESET_HOURS` (default 24) the server wipes the demo's data and seeds it again. A banner in the app says when.
- The demo never sends email: emailing an invoice or meal plan is refused, and booking-page confirmations and reminders for it are dropped. It can't save AI keys either.
- It has no password, so it can't be signed in to any other way, and it doesn't count as the first account for `SIGNUP_MODE=first`. Your own account and data are separate from it, as with any two accounts.
- Use `SIGNUP_MODE=first` or `closed` on a demo server unless you want visitors to create accounts of their own.

## Your responsibilities

A self-hosted server holds your clients' names, addresses, allergies and payment records. Keep the server patched, keep backups off the machine, use strong secrets, and follow the privacy rules that apply where you work. Allergen detection in Platebook is keyword-based and advisory only. Always confirm allergies with the client.
