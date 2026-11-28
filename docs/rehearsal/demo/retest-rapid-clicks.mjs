// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Retest for rehearsal finding D1 (rapid clicks sent duplicate emails).
 * Fires five clicks at an Email button, first in one JavaScript tick (harsher
 * than a person), then as real mouse clicks 60 ms apart, on the meal-plan and
 * invoice pages, and counts new messages in the mail sink each time.
 *
 *   node docs/rehearsal/demo/retest-rapid-clicks.mjs [--a http://localhost:4320] [--sink http://127.0.0.1:2590]
 *
 * Run after seed.mjs (it uses the Okafor session and invoice). Local only.
 * Two guards: the browser ignores clicks while a send runs, and the server
 * refuses a repeat of a successful send within 10 s (EMAIL_JUST_SENT).
 */

import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const arg = (n, d) => { const i = process.argv.indexOf(`--${n}`); return i > 0 ? process.argv[i + 1] : d; };
const A = arg('a', 'http://localhost:4320');
const SINK = arg('sink', 'http://127.0.0.1:2590');
for (const u of [A, SINK]) if (!['localhost', '127.0.0.1'].includes(new URL(u).hostname)) throw new Error(`local only: ${u}`);
const count = async () => (await (await fetch(`${SINK}/api/messages`)).json()).length;
await fetch(`${SINK}/mode?set=accept`, { method: 'POST' });

const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
const page = await ctx.newPage();
await page.goto(`${A}/login`);
await page.getByLabel(/email/i).fill('robin.hale@example.com');
await page.getByLabel(/password/i).fill('rehearsal-only-pass-2026');
await page.getByRole('button', { name: /sign in/i }).click();
await page.waitForURL((u) => !u.pathname.startsWith('/login'));
const cookie = (await ctx.cookies()).map((c) => `${c.name}=${c.value}`).join('; ');
const get = async (u) => (await fetch(A + u, { headers: { cookie } })).json();
const session = (await get('/api/sessions')).find((s) => s.client_name === 'Okafor Household');
const invoice = (await get('/api/invoices')).find((i) => i.client_name === 'Okafor Household' && ['draft', 'sent'].includes(i.status));

let failed = 0;
for (const [label, url, name] of [['meal plan', `/sessions/${session.id}`, /✉️ Email/], ['invoice', invoice && `/invoices/${invoice.id}`, /📧 Email/]]) {
  if (!url) { console.log(`SKIP  ${label}: no open invoice`); continue; }
  for (const how of ['one tick', 'mouse 60 ms apart']) {
    // Each round starts after the server's 10 s repeat-send window, so it
    // measures one fresh send (inside the window every click is refused).
    await page.waitForTimeout(11000);
    await page.goto(A + url);
    await page.waitForTimeout(1500);
    const skip = page.getByRole('button', { name: /skip tour/i });
    if (await skip.count()) await skip.click();
    const button = page.getByRole('button', { name }).first();
    const before = await count();
    if (how === 'one tick') {
      await page.evaluate((el) => { for (let i = 0; i < 5; i += 1) el.click(); }, await button.elementHandle());
    } else {
      const box = await button.boundingBox();
      for (let i = 0; i < 5; i += 1) { await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2); await page.waitForTimeout(60); }
    }
    await page.waitForTimeout(6000);
    const sent = (await count()) - before;
    if (sent !== 1) failed += 1;
    console.log(`${sent === 1 ? 'PASS' : 'FAIL'}  ${label}, 5 clicks (${how}) -> ${sent} message(s)`);
  }
}
await browser.close();
process.exitCode = failed ? 1 : 0;
