#!/usr/bin/env node
// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * One-time data migration: Platebook SQLite database → the Rust server's
 * Postgres schema (server-rs/migrations).
 *
 *   node tools/migrate-sqlite/migrate-to-postgres.js --sqlite <file> [--database-url <url>] [--force] [--dry-run]
 *
 *   --sqlite        the SQLite file to copy (opened read-only; use a backup
 *                   copy, never the live file of a running server)
 *   --database-url  target Postgres (default: $DATABASE_URL). It must already be
 *                   migrated by server-rs (start the Rust server once) and be
 *                   empty. The user must be a superuser or have BYPASSRLS.
 *   --force         allow a non-empty target: its app tables are TRUNCATEd
 *                   (in the same transaction) before copying
 *   --dry-run       do everything, print the report, then roll back
 *
 * Everything happens in ONE transaction: either every row is copied and the
 * per-account row counts match, or nothing is written. Ids are kept (so
 * links, cookies and intake edit tokens keep working) and every identity
 * sequence is moved past the highest id ever used (SQLite AUTOINCREMENT's
 * high-water mark included, so ids of deleted rows are never reused).
 *
 * Conversions follow the Postgres column types: 0/1 → BOOLEAN, JSON text →
 * JSONB, `YYYY-MM-DD HH:MM:SS` (SQLite datetime('now'), UTC) or ISO-8601 →
 * TIMESTAMPTZ, `YYYY-MM-DD` → DATE, REAL dollars → NUMERIC. Values that can't
 * be converted, NULLs in NOT NULL columns (e.g. account_id) and rows Postgres
 * rejects (foreign keys, uniqueness) are all REPORTED and the run fails —
 * nothing is silently dropped. Lossless clean-ups (blank strings in non-text
 * columns → NULL or '[]', date-only timestamps) are applied and listed.
 *
 * Exit codes: 0 copied (or dry run passed), 1 problems found or verification
 * failed (nothing written), 2 refused (bad arguments, target not ready).
 */
/* eslint-disable no-await-in-loop -- one transaction on one connection: statements must run in order */
const fs = require('fs');
const path = require('path');
const Database = require('better-sqlite3');
const { Client } = require('pg');

// Copy order respects foreign keys. `owner` says how a row maps to an
// account for the per-account verification.
const TABLES = [
  { name: 'accounts', owner: { self: true } },
  { name: 'settings', key: 'account_id', owner: { column: 'account_id' } },
  { name: 'clients', owner: { column: 'account_id' } },
  { name: 'recipes', owner: { column: 'account_id' } },
  { name: 'ingredients', owner: { parent: 'recipes', via: 'recipe_id' } },
  { name: 'session_templates', owner: { column: 'account_id' } },
  { name: 'sessions', owner: { column: 'account_id' } },
  { name: 'session_dishes', owner: { parent: 'sessions', via: 'session_id' } },
  { name: 'grocery_items', owner: { parent: 'sessions', via: 'session_id' } },
  { name: 'recipe_import_log', owner: { column: 'account_id' } },
  { name: 'client_intakes', owner: { column: 'account_id' } },
  { name: 'invoices', owner: { column: 'account_id' } },
  { name: 'invoice_line_items', owner: { parent: 'invoices', via: 'invoice_id' } },
  { name: 'payment_events', owner: { column: 'account_id' } },
];
// Tables that exist on one side only, on purpose.
const SQLITE_ONLY = new Set(['db_meta', 'sqlite_sequence']);
const PG_ONLY = new Set(['_sqlx_migrations', 'used_jtis', 'rate_limit_hits']);

// ── Arguments ───────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const opts = { force: false, dryRun: false, sqlite: null, databaseUrl: process.env.DATABASE_URL || null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--force') opts.force = true;
    else if (a === '--dry-run') opts.dryRun = true;
    else if (a === '--sqlite') opts.sqlite = argv[++i];
    else if (a === '--database-url') opts.databaseUrl = argv[++i];
    else if (a === '--help' || a === '-h') opts.help = true;
    else throw new Refusal(`unknown argument: ${a}`);
  }
  return opts;
}

class Refusal extends Error {}

/** postgres://user@host:port/db without the password, for logs. */
function describeUrl(url) {
  try {
    const u = new URL(url);
    return `${u.username || '?'}@${u.hostname}${u.port ? `:${u.port}` : ''}${u.pathname}`;
  } catch {
    return '(unparseable URL)';
  }
}

// ── Value conversion ────────────────────────────────────────────────────────

const pad = (n, w = 2) => String(n).padStart(w, '0');

function validYmd(y, m, d) {
  const dt = new Date(Date.UTC(y, m - 1, d));
  return dt.getUTCFullYear() === y && dt.getUTCMonth() === m - 1 && dt.getUTCDate() === d;
}

const DATE_RE = /^(\d{4})-(\d{2})-(\d{2})$/;
const TS_RE = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::(\d{2})(\.\d+)?)?)?\s*(Z|[+-]\d{2}(?::?\d{2})?)?$/i;

/**
 * Converts one SQLite value for a Postgres column. Returns
 * `{ value }` (value may be null), optionally with `fixup` (a note about a
 * lossless clean-up), or `{ error }`.
 */
function convert(raw, col) {
  if (raw === null || raw === undefined) return { value: null };
  if (Buffer.isBuffer(raw)) return { error: 'binary (BLOB) value' };

  const base = col.base;
  const isText = base === 'text' || base === 'varchar' || base === 'bpchar';
  if (isText) {
    const s = typeof raw === 'string' ? raw : String(raw);
    if (s.includes('\u0000')) return { error: 'text contains a NUL byte' };
    return { value: s };
  }

  // Blank strings in typed columns: SQLite stored whatever the form sent.
  if (typeof raw === 'string' && raw.trim() === '') {
    if ((base === 'jsonb' || base === 'json') && /^'\[\]'/.test(col.defaultExpr || '')) {
      return { value: '[]', fixup: "blank → '[]' (the column default)" };
    }
    return { value: null, fixup: 'blank → NULL' };
  }

  switch (base) {
    case 'bool': {
      if (raw === 1 || raw === 0) return { value: raw === 1 };
      if (raw === true || raw === false) return { value: raw };
      const s = String(raw).trim().toLowerCase();
      if (s === '1' || s === 'true') return { value: true };
      if (s === '0' || s === 'false') return { value: false };
      return { error: 'not a 0/1 flag' };
    }
    case 'int2':
    case 'int4':
    case 'int8': {
      const n = typeof raw === 'number' ? raw : (/^\s*-?\d+\s*$/.test(String(raw)) ? Number(raw) : NaN);
      if (!Number.isInteger(n)) return { error: 'not an integer' };
      const limit = base === 'int2' ? 2 ** 15 : base === 'int4' ? 2 ** 31 : 2 ** 53;
      if (n >= limit || n < -limit) return { error: `out of range for ${col.type}` };
      return { value: n };
    }
    case 'numeric': {
      const n = typeof raw === 'number' ? raw : (/^\s*-?(\d+\.?\d*|\.\d+)\s*$/.test(String(raw)) ? Number(raw) : NaN);
      if (!Number.isFinite(n)) return { error: 'not a number' };
      const scale = col.scale;
      if (scale !== null) {
        const f = 10 ** scale;
        if (Math.abs(n * f - Math.round(n * f)) > 1e-6) {
          return { value: n, fixup: `rounded to ${scale} decimal places` };
        }
      }
      return { value: n };
    }
    case 'jsonb':
    case 'json': {
      if (typeof raw !== 'string') return { error: 'not JSON text' };
      try {
        return { value: JSON.stringify(JSON.parse(raw)) };
      } catch {
        return { error: 'invalid JSON' };
      }
    }
    case 'date': {
      const m = typeof raw === 'string' ? DATE_RE.exec(raw.trim()) : null;
      if (!m || !validYmd(+m[1], +m[2], +m[3])) return { error: 'not a valid YYYY-MM-DD date' };
      return { value: raw.trim() };
    }
    case 'timestamptz':
    case 'timestamp': {
      if (typeof raw === 'number') {
        const ms = raw > 1e11 ? raw : raw * 1000;
        const d = new Date(ms);
        if (Number.isNaN(d.getTime())) return { error: 'not a valid timestamp' };
        return { value: d.toISOString(), fixup: 'epoch number → timestamp' };
      }
      const m = TS_RE.exec(String(raw).trim());
      if (!m) return { error: 'not a valid timestamp' };
      const [, y, mo, d, h = '00', mi = '00', s = '00', frac = '', zone] = m;
      if (!validYmd(+y, +mo, +d) || +h > 23 || +mi > 59 || +s > 59) return { error: 'not a valid timestamp' };
      // No zone: SQLite's datetime('now') is UTC, and so is everything Node wrote.
      const z = zone ? (zone.toUpperCase() === 'Z' ? 'Z' : zone) : 'Z';
      const iso = `${y}-${mo}-${d}T${h}:${mi}:${s}${frac}${z}`;
      return m[4] === undefined ? { value: iso, fixup: 'date only → midnight UTC' } : { value: iso };
    }
    default:
      return { value: typeof raw === 'string' ? raw : String(raw), fixup: `copied as text into ${col.type}` };
  }
}

// ── Postgres metadata ───────────────────────────────────────────────────────

async function loadPgColumns(pg) {
  const { rows } = await pg.query(`
    SELECT c.relname AS table_name, a.attname AS column_name,
           format_type(a.atttypid, a.atttypmod) AS type, t.typname AS base,
           a.attnotnull AS not_null, (a.atthasdef OR a.attidentity <> '') AS has_default,
           a.attidentity <> '' AS is_identity, pg_get_expr(d.adbin, d.adrelid) AS default_expr
      FROM pg_attribute a
      JOIN pg_class c ON c.oid = a.attrelid
      JOIN pg_namespace n ON n.oid = c.relnamespace
      JOIN pg_type t ON t.oid = a.atttypid
      LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
     WHERE n.nspname = 'public' AND c.relkind = 'r' AND a.attnum > 0 AND NOT a.attisdropped
     ORDER BY c.relname, a.attnum`);
  const tables = new Map();
  for (const r of rows) {
    if (!tables.has(r.table_name)) tables.set(r.table_name, []);
    const scale = /^numeric\(\d+,(\d+)\)$/.exec(r.type);
    tables.get(r.table_name).push({
      name: r.column_name,
      type: r.type,
      base: r.base,
      notNull: r.not_null,
      hasDefault: r.has_default,
      isIdentity: r.is_identity,
      defaultExpr: r.default_expr,
      scale: scale ? Number(scale[1]) : null,
    });
  }
  return tables;
}

const q = (ident) => `"${ident.replace(/"/g, '""')}"`;

// ── Per-account counts (same SQL runs on both databases) ────────────────────

function countSql(t) {
  const key = t.key || 'id';
  if (t.owner.self) return `SELECT id AS account_id, COUNT(*) AS n, SUM(id) AS idsum FROM accounts GROUP BY id`;
  if (t.owner.column) {
    return `SELECT ${t.owner.column} AS account_id, COUNT(*) AS n, SUM(${key}) AS idsum FROM ${t.name} GROUP BY ${t.owner.column}`;
  }
  return `SELECT p.account_id AS account_id, COUNT(*) AS n, SUM(c.id) AS idsum
            FROM ${t.name} c LEFT JOIN ${t.owner.parent} p ON p.id = c.${t.owner.via}
           GROUP BY p.account_id`;
}

const normCounts = (rows) => {
  const m = new Map();
  for (const r of rows) {
    m.set(r.account_id === null ? 'none' : String(r.account_id), { n: String(r.n), idsum: String(r.idsum ?? 0) });
  }
  return m;
};

// ── Main ────────────────────────────────────────────────────────────────────

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (opts.help) {
    console.log(fs.readFileSync(__filename, 'utf8').split('*/')[0]);
    return 0;
  }
  if (!opts.sqlite) throw new Refusal('--sqlite <file> is required');
  if (!opts.databaseUrl) throw new Refusal('--database-url <url> (or DATABASE_URL) is required');
  if (!fs.existsSync(opts.sqlite)) throw new Refusal(`no such file: ${opts.sqlite}`);

  const lite = new Database(opts.sqlite, { readonly: true, fileMustExist: true });
  const check = lite.pragma('quick_check', { simple: true });
  if (check !== 'ok') throw new Refusal(`SQLite quick_check failed: ${check}`);

  const pg = new Client({ connectionString: opts.databaseUrl });
  await pg.connect();
  console.log(`source  ${path.resolve(opts.sqlite)}`);
  console.log(`target  ${describeUrl(opts.databaseUrl)}${opts.dryRun ? '  (dry run)' : ''}\n`);

  try {
    return await migrate(lite, pg, opts);
  } finally {
    await pg.end();
    lite.close();
  }
}

async function migrate(lite, pg, opts) {
  // ── Target checks ─────────────────────────────────────────────────────────
  const role = await pg.query('SELECT rolsuper OR rolbypassrls AS ok FROM pg_roles WHERE rolname = current_user');
  if (!role.rows[0]?.ok) {
    throw new Refusal('the Postgres user must be a superuser or have BYPASSRLS (row-level security would hide rows)');
  }
  const migrated = await pg.query("SELECT to_regclass('public._sqlx_migrations') AS t");
  if (!migrated.rows[0].t) {
    throw new Refusal('target is not migrated: start the Rust server against it once (it runs server-rs/migrations), then retry');
  }
  const failedMigrations = await pg.query('SELECT version FROM _sqlx_migrations WHERE NOT success');
  if (failedMigrations.rowCount) throw new Refusal('target has a failed sqlx migration; fix that first');

  const pgCols = await loadPgColumns(pg);
  for (const t of TABLES) {
    if (!pgCols.has(t.name)) throw new Refusal(`target has no table ${t.name}: is it the Platebook database?`);
  }
  const liteTables = lite
    .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
    .all()
    .map((r) => r.name);
  const known = new Set(TABLES.map((t) => t.name));
  const warnings = [];
  for (const name of liteTables) {
    if (!known.has(name) && !SQLITE_ONLY.has(name)) {
      const n = lite.prepare(`SELECT COUNT(*) AS n FROM ${q(name)}`).get().n;
      warnings.push(`SQLite table ${name} (${n} rows) has no Postgres counterpart and is not copied`);
    }
  }
  for (const name of pgCols.keys()) {
    if (!known.has(name) && !PG_ONLY.has(name)) {
      throw new Refusal(`target table ${name} is not handled by this script: update TABLES before migrating`);
    }
  }

  const problems = [];
  const fixups = new Map(); // "table.column: note" → { count, examples }
  const noteFixup = (table, column, note, rowDesc) => {
    const k = `${table}.${column}: ${note}`;
    const f = fixups.get(k) || { count: 0, examples: [] };
    f.count += 1;
    if (f.examples.length < 3) f.examples.push(rowDesc);
    fixups.set(k, f);
  };

  await pg.query('BEGIN');
  let committed = false;
  try {
    await pg.query("SET LOCAL TIME ZONE 'UTC'");
    // Keep the server (if running) from writing while we copy.
    await pg.query(`LOCK TABLE ${TABLES.map((t) => t.name).join(', ')} IN EXCLUSIVE MODE`);

    const existing = [];
    for (const t of TABLES) {
      const { rows } = await pg.query(`SELECT COUNT(*)::int AS n FROM ${t.name}`);
      if (rows[0].n > 0) existing.push(`${t.name}: ${rows[0].n}`);
    }
    if (existing.length) {
      if (!opts.force) {
        throw new Refusal(`target is not empty (${existing.join(', ')}). Use a fresh database, or --force to TRUNCATE it first.`);
      }
      console.log(`--force: emptying target (${existing.join(', ')})\n`);
      await pg.query(`TRUNCATE ${TABLES.map((t) => t.name).join(', ')} RESTART IDENTITY CASCADE`);
    }

    // ── Copy ──────────────────────────────────────────────────────────────
    const liteSet = new Set(liteTables);
    for (const t of TABLES) {
      if (!liteSet.has(t.name)) {
        warnings.push(`SQLite has no ${t.name} table (nothing to copy)`);
        continue;
      }
      const cols = pgCols.get(t.name);
      const liteCols = lite.prepare(`PRAGMA table_info(${q(t.name)})`).all().map((c) => c.name);
      const liteColSet = new Set(liteCols);
      const copyCols = cols.filter((c) => liteColSet.has(c.name));
      const pgColSet = new Set(cols.map((c) => c.name));
      for (const c of liteCols) {
        if (pgColSet.has(c)) continue;
        const n = lite.prepare(`SELECT COUNT(*) AS n FROM ${q(t.name)} WHERE ${q(c)} IS NOT NULL`).get().n;
        if (n > 0) warnings.push(`${t.name}.${c} exists only in SQLite (${n} non-NULL values not copied)`);
      }
      const rows = lite.prepare(`SELECT * FROM ${q(t.name)} ORDER BY rowid`).all();
      const missingRequired = cols.filter((c) => !liteColSet.has(c.name) && c.notNull && !c.hasDefault);
      for (const c of missingRequired) {
        if (rows.length) problems.push({ table: t.name, row: `${rows.length} rows`, column: c.name, reason: 'required column missing from SQLite' });
      }

      const keyCol = t.key || 'id';
      const converted = [];
      for (const row of rows) {
        const desc = `${keyCol}=${row[keyCol]}`;
        let ok = true;
        const values = copyCols.map((c) => {
          const r = convert(row[c.name], c);
          if (r.error) {
            problems.push({ table: t.name, row: desc, column: c.name, value: row[c.name], reason: r.error });
            ok = false;
            return null;
          }
          if (r.fixup) noteFixup(t.name, c.name, r.fixup, desc);
          if (r.value === null && c.notNull) {
            problems.push({ table: t.name, row: desc, column: c.name, value: row[c.name], reason: 'NULL in a NOT NULL column' });
            ok = false;
          }
          return r.value;
        });
        if (ok) converted.push({ desc, values });
      }

      await insertRows(pg, t.name, copyCols, converted, problems);
      console.log(`copied  ${t.name.padEnd(20)} ${String(converted.length).padStart(6)} / ${rows.length}`);
    }

    // ── Report problems (nothing is committed if there are any) ───────────
    for (const w of warnings) console.log(`\nwarning: ${w}`);
    if (fixups.size) {
      console.log('\nClean-ups applied (values kept, representation changed):');
      for (const [k, f] of fixups) console.log(`  ${k} — ${f.count} row(s), e.g. ${f.examples.join(', ')}`);
    }
    if (problems.length) {
      console.log(`\n${problems.length} problem(s) — nothing was written. Fix these rows in a COPY of the SQLite file and re-run:`);
      for (const p of problems.slice(0, 500)) {
        const v = p.value === undefined ? '' : ` value=${JSON.stringify(p.value)?.slice(0, 60)}`;
        console.log(`  ${p.table} ${p.row}${p.column ? ` ${p.column}` : ''}:${v} — ${p.reason}`);
      }
      if (problems.length > 500) console.log(`  … and ${problems.length - 500} more`);
      await pg.query('ROLLBACK');
      return 1;
    }

    // ── Sequences ─────────────────────────────────────────────────────────
    const liteSeq = new Map();
    if (liteSet.has('sqlite_sequence') || lite.prepare("SELECT 1 FROM sqlite_master WHERE name = 'sqlite_sequence'").get()) {
      for (const r of lite.prepare('SELECT name, seq FROM sqlite_sequence').all()) liteSeq.set(r.name, Number(r.seq));
    }
    console.log('\nNext ids:');
    for (const t of TABLES) {
      const idCol = pgCols.get(t.name).find((c) => c.name === 'id' && c.isIdentity);
      if (!idCol) continue;
      const { rows } = await pg.query(`SELECT COALESCE(MAX(id), 0)::bigint AS m FROM ${t.name}`);
      const high = Math.max(Number(rows[0].m), liteSeq.get(t.name) || 0);
      await pg.query(`SELECT setval(pg_get_serial_sequence($1, 'id'), $2, $3)`, [t.name, Math.max(high, 1), high > 0]);
      console.log(`  ${t.name.padEnd(20)} ${high + 1}`);
    }

    // ── Verify per table per account ──────────────────────────────────────
    console.log('\nRow counts (table, account, sqlite, postgres):');
    let mismatches = 0;
    for (const t of TABLES) {
      if (!liteSet.has(t.name)) continue;
      const sql = countSql(t);
      const a = normCounts(lite.prepare(sql).all());
      const b = normCounts((await pg.query(sql)).rows);
      const accounts = [...new Set([...a.keys(), ...b.keys()])].sort((x, y) => Number(x) - Number(y));
      if (!accounts.length) console.log(`  ${t.name.padEnd(20)} (empty)`);
      for (const acct of accounts) {
        const x = a.get(acct) || { n: '0', idsum: '0' };
        const y = b.get(acct) || { n: '0', idsum: '0' };
        const ok = x.n === y.n && x.idsum === y.idsum;
        if (!ok) mismatches += 1;
        console.log(`  ${t.name.padEnd(20)} ${`account ${acct}`.padEnd(14)} ${x.n.padStart(6)} ${y.n.padStart(6)}${ok ? '' : '   MISMATCH'}`);
      }
    }
    if (mismatches) {
      console.log(`\n${mismatches} count mismatch(es) — rolled back, nothing was written.`);
      await pg.query('ROLLBACK');
      return 1;
    }

    if (opts.dryRun) {
      await pg.query('ROLLBACK');
      console.log('\nDry run OK: every row converts and verifies. Rolled back; nothing was written.');
      return 0;
    }
    await pg.query('COMMIT');
    committed = true;
    await pg.query('ANALYZE');
    console.log('\nMigration committed. Row counts verified for every table and account.');
    return 0;
  } catch (err) {
    if (!committed) await pg.query('ROLLBACK').catch(() => {});
    throw err;
  }
}

/**
 * Inserts rows in batches, each under a savepoint. When a batch fails, its
 * rows are retried one by one so every rejected row is reported (and the
 * others still go in, so later tables' foreign keys are checked too).
 */
async function insertRows(pg, table, cols, rows, problems) {
  if (!rows.length || !cols.length) return;
  const colList = cols.map((c) => q(c.name)).join(', ');
  const batchSize = Math.max(1, Math.min(500, Math.floor(30000 / cols.length)));
  const sqlFor = (n) => {
    const tuples = [];
    for (let r = 0; r < n; r++) {
      tuples.push(`(${cols.map((c, i) => `$${r * cols.length + i + 1}::${c.type}`).join(', ')})`);
    }
    return `INSERT INTO ${q(table)} (${colList}) VALUES ${tuples.join(', ')}`;
  };
  for (let start = 0; start < rows.length; start += batchSize) {
    const batch = rows.slice(start, start + batchSize);
    await pg.query('SAVEPOINT batch');
    try {
      await pg.query(sqlFor(batch.length), batch.flatMap((r) => r.values));
      await pg.query('RELEASE SAVEPOINT batch');
      continue;
    } catch {
      await pg.query('ROLLBACK TO SAVEPOINT batch');
    }
    for (const row of batch) {
      await pg.query('SAVEPOINT one');
      try {
        await pg.query(sqlFor(1), row.values);
        await pg.query('RELEASE SAVEPOINT one');
      } catch (err) {
        await pg.query('ROLLBACK TO SAVEPOINT one');
        problems.push({ table, row: row.desc, reason: `rejected by Postgres: ${err.message}${err.detail ? ` (${err.detail})` : ''}` });
      }
    }
  }
}

if (require.main === module) {
  main()
    .then((code) => process.exit(code))
    .catch((err) => {
      if (err instanceof Refusal) {
        console.error(`refused: ${err.message}`);
        process.exit(2);
      }
      console.error(`failed: ${err.message}`);
      process.exit(1);
    });
}

module.exports = { convert };
