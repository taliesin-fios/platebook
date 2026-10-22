#!/usr/bin/env node
// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * One-command local database setup for server-rs (`npm run db:setup`).
 *
 * 1. Creates server-rs/.env from .env.example if it's missing (with random
 *    JWT_SECRET / INTAKE_SECRET values).
 * 2. Finds the Postgres server named by DATABASE_URL in that file:
 *    - reachable (a local install, e.g. Postgres 18 with psql on the PATH):
 *      creates the database if it doesn't exist;
 *    - not reachable, or `--docker`: starts the `db` service from the root
 *      docker-compose.yml (postgres:18 on 127.0.0.1:5432, database chef_dev)
 *      and waits until it's healthy.
 *
 * The schema itself is applied by the server on start (`npm run server`
 * runs any pending migrations), so this never touches existing data.
 */
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { crateDir } from './cargo.mjs';

const repoRoot = path.resolve(crateDir, '..');
const envFile = path.join(crateDir, '.env');
const useDocker = process.argv.includes('--docker');

const log = (msg) => console.log(`[db:setup] ${msg}`);
const fail = (msg) => {
  console.error(`[db:setup] ${msg}`);
  process.exit(1);
};

// 1. server-rs/.env
if (!fs.existsSync(envFile)) {
  const example = fs.readFileSync(path.join(crateDir, '.env.example'), 'utf8');
  const secret = () => crypto.randomBytes(32).toString('hex');
  fs.writeFileSync(envFile, example
    .replace(/^JWT_SECRET=.*$/m, `JWT_SECRET=${secret()}`)
    .replace(/^INTAKE_SECRET=.*$/m, `INTAKE_SECRET=${secret()}`));
  log('created server-rs/.env from .env.example');
}

const envUrl = fs.readFileSync(envFile, 'utf8').match(/^DATABASE_URL=(.+)$/m)?.[1]?.trim();
if (!envUrl) fail('DATABASE_URL is not set in server-rs/.env');
const target = new URL(envUrl);
const dbName = decodeURIComponent(target.pathname.replace(/^\//, ''));
if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(dbName)) fail(`unexpected database name "${dbName}" in DATABASE_URL`);
const admin = new URL(envUrl);
admin.pathname = '/postgres';
const where = `${target.hostname}:${target.port || 5432}`;

const psql = (sql) => spawnSync('psql', [admin.toString(), '-v', 'ON_ERROR_STOP=1', '-tAc', sql], { encoding: 'utf8' });
const reachable = () => {
  const r = psql('SELECT 1');
  return !r.error && r.status === 0;
};

function startDocker() {
  log('starting Postgres 18 in Docker (docker compose up -d --wait db)');
  const r = spawnSync('docker', ['compose', 'up', '-d', '--wait', 'db'], { cwd: repoRoot, stdio: 'inherit' });
  if (r.error || r.status !== 0) {
    fail('could not start the Docker database. Start Docker Desktop, or install Postgres 14+ locally and run this again.');
  }
  if (dbName !== 'chef_dev') log(`note: the Docker database is chef_dev; server-rs/.env names "${dbName}"`);
  log(`ready: ${where}/chef_dev. Next: npm run dev (the server applies the schema on start).`);
}

// 2. the database
if (useDocker) {
  startDocker();
} else if (reachable()) {
  const exists = psql(`SELECT 1 FROM pg_database WHERE datname = '${dbName}'`).stdout.trim() === '1';
  if (exists) {
    log(`database ${dbName} already exists on ${where}; nothing to do`);
  } else {
    const r = psql(`CREATE DATABASE ${dbName}`);
    if (r.status !== 0) fail(`CREATE DATABASE ${dbName} failed:\n${r.stderr}`);
    log(`created database ${dbName} on ${where}`);
  }
  log('next: npm run dev (the server applies the schema on start)');
} else {
  const noPsql = psql('SELECT 1').error?.code === 'ENOENT';
  log(noPsql
    ? 'psql is not on the PATH; using Docker instead'
    : `no Postgres answering at ${where} (as ${target.username || 'postgres'}); using Docker instead`);
  startDocker();
}
