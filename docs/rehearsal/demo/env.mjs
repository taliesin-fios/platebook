// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Isolated rehearsal environment (local only; nothing leaves the machine):
 *   - a disposable Postgres database (dropped and recreated with --fresh)
 *   - the local mail sink (tools/dev-mail-sink): SMTP :2525, web view :2580
 *   - API A on http://localhost:4310 — NODE_ENV=production, email → the sink
 *   - API B on http://localhost:4311 — NODE_ENV=production, NO email settings
 * Both APIs share the database and secrets, so a chef signed in on one is
 * signed in on the other (cookies ignore the port). Both serve client/dist.
 *
 *   npm --prefix client run build            # once
 *   (cd server-rs && SQLX_OFFLINE=true cargo build)
 *   ADMIN_DATABASE_URL=postgres://postgres:…@localhost:5432/postgres \
 *     node docs/rehearsal/demo/env.mjs [--fresh]
 *
 * Optional: REHEARSAL_DB (default platebook_rehearsal), PORT_A, PORT_B,
 * SINK_SMTP, SINK_HTTP to run a second copy next to the first.
 *
 * Payments: manual only. AI: no key. Ctrl-C stops everything.
 */

import { spawn, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const admin = process.env.ADMIN_DATABASE_URL || 'postgres://postgres@localhost:5432/postgres';
const dbName = process.env.REHEARSAL_DB || 'platebook_rehearsal';
const dbUrl = admin.replace(/\/[^/]*$/, `/${dbName}`);
const work = path.join(os.tmpdir(), dbName.replace(/_/g, '-'));
const PORT_A = Number(process.env.PORT_A || 4310);
const PORT_B = Number(process.env.PORT_B || 4311);
const SINK_SMTP = Number(process.env.SINK_SMTP || 2525);
const SINK_HTTP = Number(process.env.SINK_HTTP || 2580);
fs.mkdirSync(work, { recursive: true });

if (process.argv.includes('--fresh')) {
  spawnSync('psql', [admin, '-qc', `DROP DATABASE IF EXISTS ${dbName} WITH (FORCE)`], { stdio: 'inherit' });
  fs.rmSync(path.join(work, 'mail'), { recursive: true, force: true });
}
spawnSync('psql', [admin, '-qc', `CREATE DATABASE ${dbName}`], { stdio: 'ignore' });

// Secrets for this disposable environment only (kept so restarts keep sessions).
const secretsFile = path.join(work, 'secrets.json');
const secrets = fs.existsSync(secretsFile) ? JSON.parse(fs.readFileSync(secretsFile, 'utf8')) : {
  jwt: crypto.randomBytes(32).toString('hex'), intake: crypto.randomBytes(32).toString('hex'),
};
fs.writeFileSync(secretsFile, JSON.stringify(secrets), { mode: 0o600 });

const exe = path.join(root, 'server-rs/target/debug', process.platform === 'win32' ? 'chef-server.exe' : 'chef-server');
const children = [];
function run(name, cmd, args, env) {
  const out = fs.openSync(path.join(work, `${name}.log`), 'a');
  const child = spawn(cmd, args, { cwd: work, env: { ...process.env, ...env }, stdio: ['ignore', out, out] });
  children.push(child);
  child.on('exit', (code) => console.log(`[env] ${name} exited (${code})`));
  return child;
}

run('mail-sink', process.execPath, [path.join(root, 'tools/dev-mail-sink/sink.mjs'), '--smtp', String(SINK_SMTP), '--http', String(SINK_HTTP), '--dir', path.join(work, 'mail')], {});

const common = {
  DATABASE_URL: dbUrl, NODE_ENV: 'production', JWT_SECRET: secrets.jwt, INTAKE_SECRET: secrets.intake,
  CLIENT_DIST: path.join(root, 'client/dist'), PAYMENTS_MODE: 'manual', ANTHROPIC_API_KEY: '',
  SIGNUP_MODE: 'open', TRUST_PROXY_HOPS: '0', AUTO_BACKUP: 'false', DEMO_MODE: 'false',
  RUST_LOG: 'info,tower_http=warn', EMAIL_CAPTURE: 'false', ALLOW_DEV_LINK: 'false',
};
run('api-mail', exe, [], {
  ...common, PORT: String(PORT_A), CLIENT_URL: `http://localhost:${PORT_A}`,
  EMAIL_HOST: '127.0.0.1', EMAIL_PORT: String(SINK_SMTP), EMAIL_TLS: 'none', EMAIL_USER: '', EMAIL_PASS: '',
  EMAIL_FROM: 'Platebook Rehearsal <noreply@example.test>', EMAIL_TIMEOUT_SECS: '5',
});
// Give A a head start so only one instance runs the migrations.
await new Promise((r) => setTimeout(r, 4000));
run('api-nomail', exe, [], {
  ...common, PORT: String(PORT_B), CLIENT_URL: `http://localhost:${PORT_B}`,
  EMAIL_HOST: '', EMAIL_PORT: '', EMAIL_USER: '', EMAIL_PASS: '', EMAIL_FROM: '',
});

console.log(`[env] database ${dbName}; logs in ${work}`);
console.log(`[env] API with mail sink : http://localhost:${PORT_A}`);
console.log(`[env] API with NO email  : http://localhost:${PORT_B}`);
console.log(`[env] mail sink web view : http://127.0.0.1:${SINK_HTTP}  (POST /mode?set=accept|reject|tempfail|down)`);
const stop = () => { for (const c of children) c.kill('SIGTERM'); process.exit(0); };
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
await new Promise(() => {});
