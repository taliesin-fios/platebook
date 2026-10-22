#!/usr/bin/env node
// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Checks that every source file starts with the Platebook copyright header
 * (after a shebang, if any). `npm run lint` and CI run it; `--fix` adds the
 * header to files that lack it.
 *
 *   node scripts/license-headers.mjs          # check
 *   node scripts/license-headers.mjs --fix    # add missing headers
 *
 * Skipped: generated files (ts-rs types, .sqlx), shipped migrations (sqlx
 * checksums them), test fixtures, JSON and HTML.
 */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SPDX = 'SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0';
const COPYRIGHT = 'Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook';

const LINE = /\.(ts|tsx|js|mjs|cjs|rs|typ)$/;
const BLOCK = /\.css$/;
const SKIP = [
  /^client\/src\/types\/generated\//,
  /^server-rs\/\.sqlx\//,
  /^server-rs\/migrations\//,
  /\/fixtures\//,
];

const header = (file) => (BLOCK.test(file)
  ? `/* ${SPDX}\n * ${COPYRIGHT} */\n`
  : `// ${SPDX}\n// ${COPYRIGHT}\n`);

const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard'], { cwd: root, encoding: 'utf8' })
  .split('\n')
  .filter((f) => (LINE.test(f) || BLOCK.test(f)) && !SKIP.some((re) => re.test(f)) && fs.existsSync(path.join(root, f)));

const fix = process.argv.includes('--fix');
const missing = [];
for (const file of files) {
  const full = path.join(root, file);
  const text = fs.readFileSync(full, 'utf8');
  const shebang = text.startsWith('#!') ? text.slice(0, text.indexOf('\n') + 1) : '';
  const head = text.slice(shebang.length, shebang.length + 400);
  if (head.includes(SPDX) && head.includes('taliesin-fios')) continue;
  if (fix) {
    fs.writeFileSync(full, shebang + header(file) + '\n' + text.slice(shebang.length));
  } else {
    missing.push(file);
  }
}

if (missing.length) {
  console.error(`License header missing in ${missing.length} file(s):\n  ${missing.join('\n  ')}\nRun: node scripts/license-headers.mjs --fix`);
  process.exit(1);
}
console.log(`License headers: ${files.length} files ${fix ? 'checked/fixed' : 'OK'}.`);
