// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Scripted owner demo: drives a real Chromium through the seeded fictional
 * week (seed.mjs), records a video, takes screenshots, and checks outcomes
 * (saved state, PDF text, mail-sink receipt), failing loudly if one is wrong.
 *
 *   node docs/rehearsal/demo/record.mjs [--a http://localhost:4320] [--b http://localhost:4321] \
 *        [--sink http://127.0.0.1:2590] [--out docs/rehearsal/evidence/demo-run]
 *
 * A = instance with the mail sink, B = same database with no email settings.
 * Needs `playwright` (preinstalled in the rehearsal container) and pdftotext.
 */

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const arg = (n, d) => { const i = process.argv.indexOf(`--${n}`); return i > 0 ? process.argv[i + 1] : d; };
const A = arg('a', 'http://localhost:4320');
const B = arg('b', 'http://localhost:4321');
const SINK = arg('sink', 'http://127.0.0.1:2590');
const OUT = path.resolve(arg('out', 'docs/rehearsal/evidence/demo-run'));
const CHEF = { email: 'robin.hale@example.com', password: 'rehearsal-only-pass-2026' };
fs.mkdirSync(path.join(OUT, 'screenshots'), { recursive: true });

const results = [];
const captions = [];
function check(name, ok, detail = '') {
  results.push({ name, ok: !!ok, detail });
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? ` — ${detail}` : ''}`);
}
const sink = async (p, opts) => (await fetch(SINK + p, opts)).json();
const setSink = (mode) => sink(`/mode?set=${mode}`, { method: 'POST' });
const scrub = (s) => s.replace(/token=[^\s"'&<>]+/g, 'token=<REDACTED>');

const browser = await chromium.launch();
const ctx = await browser.newContext({
  viewport: { width: 1280, height: 800 },
  recordVideo: { dir: path.join(OUT, 'video-tmp'), size: { width: 1280, height: 800 } },
  acceptDownloads: true,
});
const page = await ctx.newPage();
let n = 0;
async function shot(p, name, caption) {
  n += 1;
  const file = `${String(n).padStart(2, '0')}-${name}.png`;
  await p.waitForTimeout(400);
  await p.screenshot({ path: path.join(OUT, 'screenshots', file) });
  captions.push({ file, caption });
}
async function api(base, url, init = {}) {
  const cookies = await ctx.cookies(base);
  const cookie = cookies.map((c) => `${c.name}=${c.value}`).join('; ');
  const res = await fetch(base + url, { ...init, headers: { 'content-type': 'application/json', cookie, ...(init.headers || {}) } });
  return res.json();
}
const toast = (p, re) => p.getByText(re).first().waitFor({ timeout: 20000 }).then(() => true, () => false);
const skipTour = async (p) => { const s = p.getByRole('button', { name: /skip tour/i }); if (await s.count()) await s.first().click(); };

try {
  await setSink('accept');
  await sink('/clear', { method: 'POST' });

  // 1. Sign in
  await page.goto(`${A}/login`);
  await page.getByLabel(/email/i).fill(CHEF.email);
  await page.getByLabel(/password/i).fill(CHEF.password);
  await page.getByRole('button', { name: /sign in/i }).click();
  await page.waitForURL((u) => !u.pathname.startsWith('/login'));
  await page.waitForTimeout(1000);
  await skipTour(page);
  const dash = await page.locator('body').innerText();
  check('dashboard lists the week', /Okafor Household/.test(dash) && /Lindqvist Household/.test(dash) && /Castellano Household/.test(dash));
  check('dashboard flags Okafor allergens', /Peanuts/.test(dash) && /Tree Nuts/.test(dash));
  await shot(page, 'dashboard-week', 'Dashboard after seeding the fictional week: four upcoming sessions, allergens flagged on the Okafor booking, and the booking link.');

  // 2. Grocery list: aggregated quantities, tick-off survives reload
  const sessions = await api(A, '/api/sessions');
  const okafor = sessions.find((s) => s.client_name === 'Okafor Household' && s.session_date === '2026-12-07');
  await page.goto(`${A}/sessions/${okafor.id}`);
  await page.getByRole('button', { name: 'Grocery', exact: true }).click();
  await page.getByText(/garlic — 7 cloves/).waitFor();
  check('garlic aggregated across two dishes (4 + 3 cloves)', true, 'garlic — 7 cloves');
  await page.getByText(/garlic — 7 cloves/).click();
  await page.waitForTimeout(800);
  await page.reload();
  await page.getByRole('button', { name: 'Grocery', exact: true }).click();
  await page.waitForTimeout(800);
  const grocery = await api(A, `/api/sessions/${okafor.id}/grocery`);
  const garlic = grocery.items.find((i) => i.ingredient_name === 'garlic');
  check('ticked grocery item is still ticked after reload (saved)', garlic?.is_checked === 1, `is_checked=${garlic?.is_checked}`);
  await shot(page, 'grocery-aggregated', 'Okafor grocery list: garlic combined from two dishes (4 + 3 = 7 cloves) and ticked off; the tick survived a page reload.');

  // 3. Meal plan email with the mail server down → truthful failure + PDF alternative
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  await setSink('down');
  await page.getByRole('button', { name: /✉️ Email/ }).first().click();
  const downMsg = await toast(page, /mail server didn.t accept the meal plan/i);
  check('mail server down: error says the meal plan was not sent', downMsg);
  await page.getByText(/meal plan email wasn.t sent/i).waitFor({ timeout: 15000 });
  const s1 = await api(A, `/api/sessions/${okafor.id}`);
  check('mail server down: no pdf_emailed_at recorded', s1.pdf_emailed_at === null && !!s1.pdf_email_failed_at);
  await shot(page, 'mealplan-email-failed', 'Mail server down: the meal-plan send fails, the banner says it wasn’t sent and offers Download PDF or Try again. Nothing is marked sent.');

  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByRole('alert').getByRole('button', { name: 'Download PDF' }).click(),
  ]);
  const pdfPath = path.join(OUT, 'meal-plan-okafor.pdf');
  await download.saveAs(pdfPath);
  const pdfText = execFileSync('pdftotext', ['-layout', pdfPath, '-'], { encoding: 'utf8' });
  fs.writeFileSync(path.join(OUT, 'meal-plan-okafor.txt'), pdfText);
  check('meal-plan PDF names the household, both dishes and the allergens',
    /Okafor/.test(pdfText) && /Garlic Lemon Chicken/.test(pdfText) && /Chickpea/.test(pdfText) && /Peanut/i.test(pdfText),
    'see meal-plan-okafor.txt');

  // 4. Mail server back → retry succeeds, labelled as accepted (not delivered)
  await setSink('accept');
  await page.getByRole('alert').getByRole('button', { name: 'Try again' }).click();
  const okMsg = await toast(page, /Meal plan sent to ada\.okafor@example\.com \(accepted by the mail server\)/);
  check('retry succeeds and says "accepted by the mail server"', okMsg);
  await shot(page, 'mealplan-retry-ok', 'Mail server restored: “Try again” sends the meal plan; the toast says the mail server accepted it (Platebook can’t confirm inbox delivery).');
  const mail1 = await sink('/api/messages');
  const plan = mail1.find((m) => /meal plan/i.test(m.subject));
  check('mail sink received the meal plan with a PDF attached', plan && plan.to.includes('ada.okafor@example.com') && plan.attachments.some((a) => a.filename.endsWith('.pdf')),
    plan ? `${plan.subject} · ${plan.attachments.map((a) => a.filename).join(', ')} · Reply-To ${plan.reply_to}` : 'none');
  check('client mail has the chef business email as Reply-To', plan?.reply_to?.includes('hello@juniper-table.example.com'));

  // 5. Invoice: line items, rejected send keeps Draft, retry → Sent, then manual payment
  const invoices = await api(A, '/api/invoices');
  const inv = invoices.find((i) => i.client_name === 'Okafor Household');
  await page.goto(`${A}/invoices/${inv.id}`);
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '+ Add Line Item' }).click();
  await page.getByRole('button', { name: 'Meal Prep Session' }).click();
  await page.getByRole('button', { name: '+ Add Line Item' }).click();
  await page.getByRole('button', { name: 'Grocery Shopping' }).click();
  const spins = page.getByRole('spinbutton');
  await spins.nth(1).fill('240');
  await spins.nth(3).fill('86.40');
  await page.getByRole('button', { name: '💾 Save' }).click();
  await page.waitForTimeout(1200);
  const saved = await api(A, `/api/invoices/${inv.id}`);
  check('invoice total = 240.00 + 86.40 = 326.40', Number(saved.total) === 326.4, `total=${saved.total}`);

  await setSink('reject');
  await page.getByRole('button', { name: '📧 Email' }).click();
  check('rejected by mail server: error says it refused the invoice (check the address)', await toast(page, /mail server refused the invoice/i));
  await page.getByText(/invoice email wasn.t sent/i).waitFor({ timeout: 15000 });
  const afterReject = await api(A, `/api/invoices/${inv.id}`);
  check('rejected send leaves the invoice Draft with no emailed_at', afterReject.status === 'draft' && afterReject.emailed_at === null,
    `status=${afterReject.status} emailed_at=${afterReject.emailed_at}`);
  await page.getByRole('alert').first().scrollIntoViewIfNeeded();
  await shot(page, 'invoice-email-rejected', 'Mail server rejects the recipient: the invoice stays Draft, nothing is marked sent, and the banner offers Download PDF or Try again.');

  await setSink('accept');
  await page.getByRole('alert').getByRole('button', { name: 'Try again' }).click();
  check('invoice retry says accepted by the mail server', await toast(page, /Invoice sent to ada\.okafor@example\.com \(accepted by the mail server\)/));
  const afterOk = await api(A, `/api/invoices/${inv.id}`);
  check('successful send moves the invoice to Sent with emailed_at', afterOk.status === 'sent' && !!afterOk.emailed_at && !afterOk.email_failed_at,
    `status=${afterOk.status}`);
  await page.getByText(/^Invoice INV-/).first().scrollIntoViewIfNeeded().catch(() => {});
  await shot(page, 'invoice-sent', 'After the mail server accepts it, the invoice moves to Sent ($326.40 total).');
  const mail2 = await sink('/api/messages');
  const invMail = mail2.find((m) => /invoice/i.test(m.subject));
  check('mail sink received the invoice PDF', invMail?.attachments?.some((a) => /\.pdf$/.test(a.filename)), invMail?.subject);

  // 6. Same work on a server with NO email configured
  await page.goto(`${B}/sessions/${okafor.id}`);
  await page.waitForTimeout(1000);
  await skipTour(page);
  await page.getByRole('button', { name: /✉️ Email/ }).first().click();
  check('no email configured: says email isn’t set up and nothing was sent', await toast(page, /Email isn.t set up on this Platebook server/));
  await shot(page, 'no-email-server', 'Same account on a server with no SMTP settings: the send is refused with a plain explanation and the PDF alternative; nothing is recorded as sent.');

  // 7. Sign-in links: honest copy with and without email (no account enumeration)
  const anon = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const lp = await anon.newPage();
  for (const [base, label] of [[B, 'no-email'], [A, 'with-email']]) {
    await lp.goto(`${base}/login`);
    await lp.getByRole('button', { name: /sign-in link|magic link|email me/i }).first().click();
    await lp.getByLabel(/email/i).fill('robin.hale@example.com');
    await lp.getByRole('button', { name: /send|email me/i }).last().click();
    await lp.waitForTimeout(1200);
    const t = await lp.locator('body').innerText();
    if (label === 'no-email') check('login link on a no-email server says no link was sent', /isn.t set up to send email/.test(t));
    else check('login link with email says the mail server was asked (not "delivered")', /asked the mail server/.test(t) && !/delivered/i.test(t));
    await shot(lp, `signin-link-${label}`, label === 'no-email'
      ? 'Sign-in link requested on a server without email: the page says no link was sent and suggests the password instead (same answer for every address).'
      : 'Sign-in link requested on a server with email: “we’ve asked the mail server to send it” — no claim of delivery, same answer for unknown addresses.');
  }
  // Link mail is sent after the response (so timing can't reveal accounts).
  let mail3 = [];
  for (let i = 0; i < 20 && !mail3.some((m) => /login link/i.test(m.subject)); i += 1) {
    await new Promise((r) => setTimeout(r, 250));
    mail3 = await sink('/api/messages');
  }
  check('a login-link email reached the sink for the real account', mail3.some((m) => /login link/i.test(m.subject) && m.to.includes('robin.hale@example.com')));
  await anon.close();

  // 8. Phone width (emulated, not a physical phone)
  const phone = await browser.newContext({ viewport: { width: 375, height: 812 }, isMobile: true, hasTouch: true, storageState: await ctx.storageState() });
  const pp = await phone.newPage();
  await pp.goto(`${A}/sessions/${okafor.id}`);
  await pp.waitForTimeout(1200);
  await skipTour(pp);
  const overflow = await pp.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  check('session page has no horizontal scroll at 375 px', overflow <= 1, `overflow=${overflow}px`);
  await shot(pp, 'phone-session', 'Session page at 375 px wide (browser emulation, not a physical phone).');
  await phone.close();
} catch (err) {
  check('demo script ran to the end', false, scrub(String(err.stack || err)));
} finally {
  await setSink('accept').catch(() => {});
  const video = await page.video()?.path();
  await ctx.close();
  await browser.close();
  if (video && fs.existsSync(video)) fs.renameSync(video, path.join(OUT, 'walkthrough.webm'));
  fs.rmSync(path.join(OUT, 'video-tmp'), { recursive: true, force: true });
  fs.writeFileSync(path.join(OUT, 'results.json'), JSON.stringify({ ran_at: new Date().toISOString(), A, B, results, captions }, null, 2));
  const failed = results.filter((r) => !r.ok).length;
  console.log(`\n${results.length - failed}/${results.length} checks passed. Output: ${OUT}`);
  process.exitCode = failed ? 1 : 0;
}
