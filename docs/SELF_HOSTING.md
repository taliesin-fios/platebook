# Self-hosting Platebook

This guide runs Platebook on your own server with Docker Compose: one container for the app and one for Postgres, behind a reverse proxy that handles HTTPS.

## What you need

- A Linux server or VPS (1 vCPU and 1 GB RAM is plenty for a few chefs) with Docker and the Compose plugin.
- A domain or subdomain pointing at it, e.g. `platebook.example.com`.
- Optional:
  - an SMTP mailbox, to send meal plans, invoices and sign-in links;
  - a Stripe account, for online deposits;
  - an Anthropic API key, for AI recipe import and menu suggestions.

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

- **Deposits:** `PAYMENTS_MODE=manual` (the default) records deposits the chef collects by hand. `stripe` takes them online and needs `STRIPE_SECRET_KEY` and `STRIPE_WEBHOOK_SECRET`. Add a Stripe webhook to `https://<your domain>/api/payments/webhook` for `checkout.session.completed`, `payment_intent.payment_failed` and `charge.refunded`. Clients pay from their booking page and Stripe returns them there, so `CLIENT_URL` must be your public address. Try it with test-mode keys first. `disabled` hides deposits.
- **AI:** with `ANTHROPIC_API_KEY` set, chefs can import recipes from pasted text and get menu suggestions. Usage is billed to your Anthropic account and rate-limited per chef account.

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

## Your responsibilities

A self-hosted server holds your clients' names, addresses, allergies and payment records. Keep the server patched, keep backups off the machine, use strong secrets, and follow the privacy rules that apply where you work. Allergen detection in Platebook is keyword-based and advisory only. Always confirm allergies with the client.
