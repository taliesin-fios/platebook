# Contributing to Platebook

Thanks for helping! Bug reports, fixes, docs and features are all welcome. For anything bigger than a small fix, please open an issue first, so we can agree on the approach before you spend time on it.

## Set up

You need Rust (stable, 1.92+), Node 20.19+, and PostgreSQL 14+ (or Docker). Then run:

```bash
npm run install:all   # Node dependencies
npm run db:setup      # server-rs/.env with random secrets + a local database
npm run dev           # API on :3002, app on :5174
```

The [README](README.md#development) has more detail, and [`server-rs/README.md`](server-rs/README.md) covers the server (tests, row-level security, compile-time-checked SQL).

## Before you open a pull request

Run everything CI runs:

```bash
npm run lint    # client ESLint + cargo fmt --check + cargo clippy -D warnings
npm test        # client (vitest), server (cargo test), HTTP suites
npm run build   # type-check and build the client, release-build the server
```

The server tests create a throwaway Postgres database per test, so they need a Postgres user that may create databases (default `postgres://postgres@localhost:5432/postgres`) and `psql` on the `PATH`.

Every source file starts with the Platebook copyright header. `npm run lint` checks it, and `node scripts/license-headers.mjs --fix` adds it to new files.

`cargo test` regenerates the TypeScript API types in `client/src/types/generated/`, so commit those changes along with your own. Don't add new lint warnings.

## Ground rules

- **Account isolation comes first.** Every chef's data is private to their account.
  - Data handlers use the `db::Tenant` extractor, which turns on Postgres row-level security for that account.
  - Ids from the request are checked with `ownership::owned`.
  - Every new data route needs a cross-account test in `server-rs/tests/tenancy.rs` and `tests/http/tenancy.test.js`.
  - A new public (no-login) route must be added to `auth::is_public_route` and to the allowlist in [ARCHITECTURE.md](ARCHITECTURE.md).
- **Migrations:** add a new numbered file in `server-rs/migrations/`. Never edit one that has been released. Migrations must be safe on a database that already has data.
- **Money** uses the integer-cents helpers in `server-rs/src/money.rs`. **Dates** in the client go through `client/src/utils/dateUtils.ts`.
- **No hardcoded business details.** The product name is `APP_NAME` in `client/src/constants.ts`. A chef's name, logo, prices and policies come from their settings.
- **Tests never send real email, take payments or call the Claude API.**
- Keep files LF-only, TypeScript strict, and no `any`.

[CLAUDE.md](CLAUDE.md) has the full list of conventions. It's written for AI coding assistants but reads fine for people too.

## Licensing of contributions

Platebook is source-available under the [PolyForm Shield License 1.0.0](LICENSE). By submitting a contribution, you agree that it's your own work, or that you have the right to submit it, and that it's licensed to the project and its users under the same license. Please sign off your commits (`git commit -s`) to confirm this, following the [Developer Certificate of Origin](https://developercertificate.org/).

## Code of conduct

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
