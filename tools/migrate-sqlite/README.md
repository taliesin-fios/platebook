# migrate-sqlite — legacy SQLite → Postgres

The old Express server kept each install's data in one SQLite file
(`chef_app.db`). This tool copies such a file into a server-rs Postgres
database, once, at cutover. It is a standalone Node package (`better-sqlite3`
+ `pg`) so it can run anywhere the SQLite file can be read.

```bash
cd tools/migrate-sqlite
npm ci
node migrate-to-postgres.js --sqlite <copy-of-chef_app.db> --database-url <postgres-url> --dry-run
node migrate-to-postgres.js --sqlite <copy-of-chef_app.db> --database-url <postgres-url>
```

- The SQLite file is opened **read-only**. Use a backup copy, not the live
  file of a running server, and never put a copy of real data in the repo.
- The target must already be migrated by server-rs (start the server against
  it once; `npm run db:setup` + `npm run dev` does that locally) and must be
  empty. A non-empty target is refused; `--force` TRUNCATEs the app tables
  first (for rehearsals only).
- The Postgres user must be a superuser or have `BYPASSRLS`.
- Everything runs in one transaction. Ids are kept, sequences move past the
  highest id ever used, and row counts (and id sums) are verified per table
  and account. Any row that can't be converted is reported by table and id,
  and nothing is written.
- Exit codes: `0` done (or dry run passed), `1` problems found (nothing
  written), `2` refused (bad arguments, target not ready).

Full details (conversions, clean-ups) are in
[`server-rs/README.md`](../../server-rs/README.md#sqlite--postgres-data-migration).

## Tests

```bash
npm test            # value conversions (no database)
npm run test:e2e    # needs cargo, psql and a Postgres user that may CREATE DATABASE
```

`test:e2e` builds a SQLite database from `test/fixtures/legacy.sql` (two
synthetic accounts recorded from the last Express server, with every table
filled), migrates it into a fresh server-rs database, signs in with the
migrated password hashes and requires every GET in
`test/fixtures/expected.json` (what Express answered) to come back identical
from server-rs. It also checks sequences, the non-empty refusal, `--dry-run`
and that bad rows are reported. The root `npm run test:migrate` runs both,
and CI does too.
