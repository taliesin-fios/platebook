// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * One command for the repeatable demo: a fresh isolated environment
 * (database platebook_demo, ports 4320/4321, sink 2535/2590), the fictional
 * week seeded, the scripted browser walkthrough recorded, the rapid-click
 * retest run, then everything
 * stopped. Exit code 0 only if every outcome check passed.
 *
 *   npm --prefix client run build && (cd server-rs && SQLX_OFFLINE=true cargo build)
 *   ADMIN_DATABASE_URL=postgres://postgres:…@localhost:5432/postgres \
 *     node docs/rehearsal/demo/run-all.mjs [--out docs/rehearsal/evidence/demo-run] [--keep]
 *
 * --keep leaves the environment running afterwards so you can click around
 * (http://localhost:4320, password in fictional-week.json); Ctrl-C stops it.
 */

import { spawn, spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const outIdx = process.argv.indexOf('--out');
const out = outIdx > 0 ? process.argv[outIdx + 1] : 'docs/rehearsal/evidence/demo-run';
const env = {
  ...process.env, REHEARSAL_DB: 'platebook_demo', PORT_A: '4320', PORT_B: '4321', SINK_SMTP: '2535', SINK_HTTP: '2590',
};

const envProc = spawn(process.execPath, [path.join(here, 'env.mjs'), '--fresh'], { env, stdio: 'inherit' });
const stop = () => envProc.kill('SIGTERM');
process.on('SIGINT', () => { stop(); process.exit(130); });

async function healthy(url) {
  for (let i = 0; i < 60; i += 1) {
    try { if ((await fetch(url)).ok) return true; } catch { /* not up yet */ }
    await new Promise((r) => setTimeout(r, 1000));
  }
  return false;
}

let code = 1;
if (await healthy('http://localhost:4320/health') && await healthy('http://localhost:4321/health')) {
  const seed = spawnSync(process.execPath, [path.join(here, 'seed.mjs'), 'http://localhost:4320'], { stdio: 'inherit' });
  if (seed.status === 0) {
    const rec = spawnSync(process.execPath, [path.join(here, 'record.mjs'), '--out', out], { stdio: 'inherit' });
    const retest = spawnSync(process.execPath, [path.join(here, 'retest-rapid-clicks.mjs')], { stdio: 'inherit' });
    code = (rec.status ?? 1) || (retest.status ?? 1);
  }
} else {
  console.error('[demo] the environment did not come up; see the logs in the temp directory env.mjs printed');
}

if (process.argv.includes('--keep')) {
  console.log('[demo] environment left running: http://localhost:4320 (Ctrl-C to stop)');
} else {
  stop();
  process.exit(code);
}
