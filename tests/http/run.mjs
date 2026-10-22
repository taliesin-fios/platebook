#!/usr/bin/env node
// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Black-box HTTP suites against the Rust API server.
 *
 *   node tests/http/run.mjs        (root: npm run test:http)
 *
 * 1. creates a throwaway Postgres database (psql must be on the PATH),
 * 2. builds and starts server-rs on a free port with the test secrets,
 * 3. runs `node --test` on the suites here with API_BASE_URL pointing at it,
 * 4. stops the server and drops the database.
 *
 * Env: PARITY_ADMIN_DATABASE_URL (a user that may CREATE DATABASE; default
 * postgres://postgres@localhost:5432/postgres), PARITY_KEEP_DB=1 to keep it.
 */
import { spawn, spawnSync } from 'node:child_process';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { cargo, crateDir, PATH } from '../../server-rs/scripts/cargo.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const adminUrl = process.env.PARITY_ADMIN_DATABASE_URL || 'postgres://postgres@localhost:5432/postgres';
const dbName = `chef_http_${process.pid}`;
const dbUrl = (() => {
  const u = new URL(adminUrl);
  u.pathname = `/${dbName}`;
  return u.toString();
})();

const SUITES = ['api.smoke.test.js', 'auth.test.js', 'tenancy.test.js'];

const run = (cmd, args, opts = {}) => {
  const r = spawnSync(cmd, args, { stdio: 'inherit', shell: false, ...opts });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')} exited with ${r.status}`);
};
const psql = (sql) => run('psql', [adminUrl, '-v', 'ON_ERROR_STOP=1', '-q', '-c', sql]);

const freePort = () => new Promise((resolve, reject) => {
  const srv = net.createServer();
  srv.listen(0, '127.0.0.1', () => {
    const { port } = srv.address();
    srv.close(() => resolve(port));
  });
  srv.on('error', reject);
});

async function waitForHealth(base, child) {
  for (let i = 0; i < 300; i++) {
    if (child.exitCode !== null) throw new Error(`server exited early (${child.exitCode})`);
    try {
      const res = await fetch(`${base}/health`);
      if (res.ok) return;
    } catch { /* not up yet */ }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error('server did not become healthy');
}

let server;
let code = 1;
try {
  console.log(`[http] building server-rs`);
  run(cargo, ['build', '--quiet'], { cwd: crateDir, env: { ...process.env, PATH, Path: PATH, SQLX_OFFLINE: 'true' } });

  console.log(`[http] creating database ${dbName}`);
  psql(`CREATE DATABASE ${dbName}`);

  const port = await freePort();
  const base = `http://127.0.0.1:${port}`;
  const exe = path.join(crateDir, 'target', 'debug', process.platform === 'win32' ? 'chef-server.exe' : 'chef-server');
  server = spawn(exe, [], {
    // Run outside server-rs/ so its .env (dev settings) is never picked up.
    cwd: os.tmpdir(),
    stdio: ['ignore', 'inherit', 'inherit'],
    env: {
      ...process.env,
      DATABASE_URL: dbUrl,
      PORT: String(port),
      NODE_ENV: 'test',
      JWT_SECRET: 'test-jwt-secret-for-smoke-tests',
      INTAKE_SECRET: 'test-intake-secret-for-smoke-tests',
      CLIENT_URL: 'http://localhost:5174',
      // Tests must never send real mail, take payments or call the AI API.
      EMAIL_HOST: '',
      PAYMENTS_MODE: 'manual',
      SIGNUP_MODE: 'open',
      ANTHROPIC_API_KEY: '',
      AUTO_BACKUP: 'false',
      RUST_LOG: process.env.RUST_LOG || 'warn',
    },
  });
  await waitForHealth(base, server);
  console.log(`[http] server-rs up at ${base}; running ${SUITES.length} suites`);

  const r = spawnSync(process.execPath, ['--test', ...SUITES], {
    cwd: here,
    stdio: 'inherit',
    env: { ...process.env, API_BASE_URL: base },
  });
  code = r.status ?? 1;
} catch (err) {
  console.error(`[http] ${err.message}`);
} finally {
  if (server && server.exitCode === null) server.kill();
  if (!process.env.PARITY_KEEP_DB) {
    await new Promise((r) => setTimeout(r, 300));
    try { psql(`DROP DATABASE IF EXISTS ${dbName} WITH (FORCE)`); } catch (e) { console.error(`[http] ${e.message}`); }
  }
}
process.exit(code);
