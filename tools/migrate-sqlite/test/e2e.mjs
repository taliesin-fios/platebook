#!/usr/bin/env node
// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * End-to-end check of migrate-to-postgres.js against server-rs.
 *
 *   npm run test:e2e        (in tools/migrate-sqlite; `npm run test:migrate` at the root)
 *
 * 1. builds a legacy SQLite database from fixtures/legacy.sql: two accounts
 *    recorded from the last Express server (starter library, clients,
 *    sessions, grocery, invoices, an intake with a payment, an import log
 *    row, a Stripe event, a deleted client);
 * 2. starts server-rs on a fresh Postgres database (which migrates it) and
 *    runs the migration tool into it;
 * 3. signs in as both accounts (the bcrypt hashes came across) and requires
 *    every GET in fixtures/expected.json, the Express server's answers, to be
 *    identical from server-rs: ids, money, dates and flags;
 * 4. checks sequences continue past the highest id ever used, that a
 *    non-empty target is refused without --force, that --dry-run writes
 *    nothing, and that bad rows (NULL account_id, impossible dates, invalid
 *    JSON, orphans) are each reported and nothing is written.
 *
 * Never touches a real database file: everything is in the OS temp dir.
 * Needs psql on the PATH and a Postgres user that may CREATE DATABASE
 * (PARITY_ADMIN_DATABASE_URL, default postgres://postgres@localhost:5432/postgres).
 */
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import Database from 'better-sqlite3';
import pg from 'pg';
import { cargo, crateDir, PATH } from '../../../server-rs/scripts/cargo.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const tool = path.resolve(here, '..', 'migrate-to-postgres.js');
const fixture = JSON.parse(fs.readFileSync(path.join(here, 'fixtures', 'expected.json'), 'utf8'));
const { accounts: { a: xa, b: xb } } = fixture;

const adminUrl = process.env.PARITY_ADMIN_DATABASE_URL || 'postgres://postgres@localhost:5432/postgres';
const dbName = `chef_migrate_${process.pid}`;
const pgUrl = (() => { const u = new URL(adminUrl); u.pathname = `/${dbName}`; return u.toString(); })();
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'chef-migrate-'));
const sqlitePath = path.join(tmp, 'source.db');
const badPath = path.join(tmp, 'bad.db');

// Same secrets the fixture was recorded with.
const ENV = {
  JWT_SECRET: 'migrate-check-jwt-secret',
  INTAKE_SECRET: 'migrate-check-intake-secret',
  NODE_ENV: 'test',
  CLIENT_URL: 'http://localhost:5174',
  EMAIL_HOST: '',
  PAYMENTS_MODE: 'manual',
  ANTHROPIC_API_KEY: '',
  AUTO_BACKUP: 'false',
};

const freePort = () => new Promise((resolve, reject) => {
  const srv = net.createServer();
  srv.listen(0, '127.0.0.1', () => { const { port } = srv.address(); srv.close(() => resolve(port)); });
  srv.on('error', reject);
});

async function waitForHealth(base, child) {
  for (let i = 0; i < 300; i++) {
    if (child.exitCode !== null) throw new Error(`server exited early (${child.exitCode})`);
    try { if ((await fetch(`${base}/health`)).ok) return; } catch { /* not yet */ }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`${base} did not become healthy`);
}

const psql = (sql) => {
  const r = spawnSync('psql', [adminUrl, '-v', 'ON_ERROR_STOP=1', '-q', '-c', sql], { stdio: 'inherit' });
  if (r.status !== 0) throw new Error(`psql failed: ${sql}`);
};

/** Runs the migration tool; returns { code, out }. */
function migrate(sqlite, ...flags) {
  const r = spawnSync(process.execPath, [tool, '--sqlite', sqlite, '--database-url', pgUrl, ...flags], { encoding: 'utf8' });
  return { code: r.status, out: `${r.stdout}${r.stderr}` };
}

/** A tiny client bound to the server, holding its session cookie. */
function client(base, ip) {
  let cookie = '';
  const call = async (method, url, body, extra = {}) => {
    const headers = { 'x-forwarded-for': ip };
    if (cookie) headers.cookie = cookie;
    if (body !== undefined) headers['content-type'] = 'application/json';
    const res = await fetch(base + url, { method, headers, body: body === undefined ? undefined : JSON.stringify(body), redirect: 'manual' });
    const set = res.headers.get('set-cookie');
    if (set && set.startsWith('authToken=')) cookie = set.split(';')[0];
    const text = await res.text();
    let json;
    try { json = JSON.parse(text); } catch { json = text; }
    if (res.status >= 400 && !extra.allowError) throw new Error(`${method} ${url} → ${res.status} ${text.slice(0, 200)}`);
    return json;
  };
  return { call };
}

// Values that legitimately differ between two reads (time of the request).
const VOLATILE = /^(exported_at)$/;
// Timestamps Node wrote as ISO strings (`new Date().toISOString()`) come back
// from server-rs in its one format, `YYYY-MM-DD HH:MM:SS` UTC (the client's
// fmtUtcTimestamp reads both): compare `*_at` values as instants, to the second.
const instant = (v) => (typeof v === 'string'
  ? Math.floor(Date.parse(v.replace(' ', 'T') + (/Z$|[+-]\d\d:?\d\d$/.test(v) ? '' : 'Z')) / 1000)
  : v);
// Rows created in the same second had no defined order in Express (SQLite);
// server-rs breaks ties by id. Lists of rows are compared by id.
const byId = (v) => (Array.isArray(v) && v.length && v.every((r) => r && typeof r === 'object' && 'id' in r)
  ? [...v].sort((x, y) => x.id - y.id)
  : v);
function diffValues(a, b, p = '$', out = [], key = '') {
  if (JSON.stringify(a) === JSON.stringify(b)) return out;
  if (/_at$/.test(key) && a !== null && b !== null && instant(a) === instant(b)) return out;
  if (a && b && typeof a === 'object' && typeof b === 'object') {
    [a, b] = [byId(a), byId(b)];
    for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) {
      if (!VOLATILE.test(k)) diffValues(a[k], b[k], `${p}.${k}`, out, k);
    }
    return out;
  }
  out.push(`${p}: express ${JSON.stringify(a)?.slice(0, 80)} vs rust ${JSON.stringify(b)?.slice(0, 80)}`);
  return out;
}

const procs = [];
let failed = false;
const check = (ok, msg) => { console.log(`${ok ? 'ok  ' : 'FAIL'} ${msg}`); if (!ok) failed = true; };

try {
  const build = spawnSync(cargo, ['build', '--quiet'], { cwd: crateDir, stdio: 'inherit', env: { ...process.env, PATH, Path: PATH, SQLX_OFFLINE: 'true' } });
  if (build.status !== 0) throw new Error('cargo build failed');

  // ── 1. the legacy SQLite database ──────────────────────────────────────
  const lite = new Database(sqlitePath);
  lite.exec(fs.readFileSync(path.join(here, 'fixtures', 'legacy.sql'), 'utf8'));
  lite.close();

  // ── 2. Postgres via server-rs, then migrate ────────────────────────────
  psql(`CREATE DATABASE ${dbName}`);
  const port = await freePort();
  const base = `http://127.0.0.1:${port}`;
  const exe = path.join(crateDir, 'target', 'debug', process.platform === 'win32' ? 'chef-server.exe' : 'chef-server');
  const rust = spawn(exe, [], {
    cwd: os.tmpdir(),
    stdio: ['ignore', 'ignore', 'inherit'],
    env: { ...process.env, ...ENV, PORT: String(port), DATABASE_URL: pgUrl, RUST_LOG: 'error' },
  });
  procs.push(rust);
  await waitForHealth(base, rust);

  const first = migrate(sqlitePath);
  console.log(first.out);
  check(first.code === 0, 'migration of the legacy database succeeds');
  check(/Row counts/.test(first.out) && !/MISMATCH/.test(first.out), 'per-account row counts are printed and match');

  // ── 3. Same JSON from server-rs as from Express ────────────────────────
  const a = client(base, '10.1.1.1');
  const b = client(base, '10.2.2.2');
  for (const [key, c, x] of [['a', a, xa], ['b', b, xb]]) {
    await c.call('POST', '/api/auth/login', { email: x.email, password: fixture.password });
    check(true, `account ${key.toUpperCase()} signs in with its migrated password hash`);
    for (const [url, expected] of Object.entries(fixture.responses[key])) {
      const after = await c.call('GET', url, undefined, { allowError: true });
      const d = diffValues(expected, after);
      check(d.length === 0, `account ${key.toUpperCase()} GET ${url} identical after migration`);
      for (const l of d.slice(0, 8)) console.log(`        ${l}`);
    }
  }

  // ── 4. Sequences, refusals, dry run, bad rows ──────────────────────────
  const fresh = await a.call('POST', '/api/clients', { name: 'After Cutover' });
  check(fresh.id > xa.deletedClient, `new ids continue past the deleted max (${fresh.id} > ${xa.deletedClient})`);
  const db = new pg.Client({ connectionString: pgUrl });
  await db.connect();
  const count = async () => Number((await db.query('SELECT COUNT(*) AS n FROM clients')).rows[0].n);
  const clientsAfter = await count();

  const again = migrate(sqlitePath);
  check(again.code === 2 && /not empty/.test(again.out), 'a non-empty target is refused without --force');
  const dry = migrate(sqlitePath, '--force', '--dry-run');
  check(dry.code === 0 && /Dry run OK/.test(dry.out), '--dry-run --force verifies and rolls back');
  check(await count() === clientsAfter, '--dry-run wrote nothing');

  fs.copyFileSync(sqlitePath, badPath);
  const w = new Database(badPath);
  w.pragma('foreign_keys = OFF');
  w.exec(`
    INSERT INTO clients (id, account_id, name) VALUES (9001, NULL, 'Orphan Client');
    INSERT INTO clients (id, account_id, name, allergens) VALUES (9002, ${xa.accountId}, 'Bad Json', 'peanuts, soy');
    INSERT INTO sessions (id, account_id, client_id, session_date) VALUES (9003, ${xa.accountId}, ${xa.client}, '2026-02-30');
    INSERT INTO session_dishes (id, session_id, recipe_id) VALUES (9004, ${xa.session}, 999999);
    INSERT INTO invoices (id, account_id, issue_date) VALUES (9005, ${xa.accountId}, '13/01/2026');
  `);
  w.close();
  const broken = migrate(badPath, '--force');
  check(broken.code === 1, 'bad rows fail the run');
  for (const [needle, what] of [
    [/clients id=9001 account_id:.*NULL in a NOT NULL column/, 'NULL account_id'],
    [/clients id=9002 allergens:.*invalid JSON/, 'invalid JSON'],
    [/sessions id=9003 session_date:.*not a valid YYYY-MM-DD date/, 'impossible date'],
    [/session_dishes id=9004: .*rejected by Postgres: .*foreign key/, 'orphan session dish'],
    [/invoices id=9005 issue_date:.*not a valid YYYY-MM-DD date/, 'non-ISO invoice date'],
  ]) check(needle.test(broken.out), `reported: ${what}`);
  check(/nothing was written/.test(broken.out) && await count() === clientsAfter, 'a failed run leaves the target untouched');
  if (failed) console.log(broken.out);
  await db.end();
} catch (err) {
  console.error(err);
  failed = true;
} finally {
  for (const p of procs) if (p.exitCode === null) p.kill();
  await new Promise((r) => setTimeout(r, 500));
  if (!process.env.PARITY_KEEP_DB) {
    try { psql(`DROP DATABASE IF EXISTS ${dbName} WITH (FORCE)`); } catch { /* best effort */ }
  }
  fs.rmSync(tmp, { recursive: true, force: true });
}
console.log(failed ? '\nmigrate e2e FAILED' : '\nmigrate e2e passed');
process.exit(failed ? 1 : 0);
