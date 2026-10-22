#!/usr/bin/env node
// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * `cargo run` for the dev server (see cargo.mjs for how cargo is found).
 * Used by `npm run server` and the "Platebook — API (Rust)" entry in
 * .claude/launch.json. Extra arguments go to cargo (`--release`). Reads
 * server-rs/.env: run `npm run db:setup` (or copy .env.example) first.
 */
import fs from 'node:fs';
import path from 'node:path';
import { crateDir, runCargo } from './cargo.mjs';

if (!fs.existsSync(path.join(crateDir, '.env'))) {
  console.error('server-rs/.env is missing: run `npm run db:setup` (or copy .env.example and point DATABASE_URL at your Postgres).');
  process.exit(1);
}

const args = process.argv.slice(2);
// `npm start` (--release) also serves the React build, like the Docker image.
const env = args.includes('--release') && !process.env.CLIENT_DIST ? { CLIENT_DIST: '../client/dist' } : {};
runCargo(['run', ...args], env);
