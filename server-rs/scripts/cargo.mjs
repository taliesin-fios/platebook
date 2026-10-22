#!/usr/bin/env node
// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Runs cargo in server-rs/, finding it even when it isn't on the PATH
 * (rustup's default ~/.cargo/bin) and, on Windows with the GNU toolchain,
 * MinGW from scoop. Used by the root npm scripts:
 *
 *   node server-rs/scripts/cargo.mjs build --release
 *   node server-rs/scripts/cargo.mjs test
 *
 * SQL is checked against the committed .sqlx/ cache (SQLX_OFFLINE=true, as in
 * CI) unless SQLX_OFFLINE is set. `cargo test` needs a Postgres user that may
 * create databases; when DATABASE_URL isn't set it defaults to
 * postgres://postgres@localhost:5432/postgres.
 */
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const crateDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const extraPaths = [
  path.join(os.homedir(), '.cargo', 'bin'),
  path.join(os.homedir(), 'scoop', 'apps', 'mingw', 'current', 'bin'),
].filter((p) => fs.existsSync(p));
const systemPath = process.env.PATH ?? process.env.Path ?? '';
/** PATH with cargo's and MinGW's directories in front. */
export const PATH = [...extraPaths, systemPath].join(path.delimiter);

const exeName = process.platform === 'win32' ? 'cargo.exe' : 'cargo';
/** The cargo executable to run. */
export const cargo = [...extraPaths, ...systemPath.split(path.delimiter)]
  .map((dir) => path.join(dir, exeName))
  .find((p) => fs.existsSync(p)) ?? exeName;

/** Spawns `cargo <args>` in server-rs/ and exits with its status. */
export function runCargo(args, env = {}) {
  const child = spawn(cargo, args, {
    cwd: crateDir,
    stdio: 'inherit',
    env: { SQLX_OFFLINE: 'true', ...process.env, ...env, PATH, Path: PATH },
  });
  child.on('exit', (code) => process.exit(code ?? 1));
  for (const sig of ['SIGINT', 'SIGTERM']) process.on(sig, () => child.kill(sig));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const env = args[0] === 'test' && !process.env.DATABASE_URL
    ? { DATABASE_URL: 'postgres://postgres@localhost:5432/postgres' }
    : {};
  runCargo(args, env);
}
