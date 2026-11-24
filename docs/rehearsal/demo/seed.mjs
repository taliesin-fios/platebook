// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Seeds the fictional week (fictional-week.json) into a running, isolated
 * Platebook through its public API, so the owner can start the demo from a
 * known state. It does not replace the browser walkthrough: it only creates
 * the starting data a chef would otherwise type in.
 *
 *   node docs/rehearsal/demo/seed.mjs [http://localhost:4310]
 *
 * Creates: the chef account (settings + onboarding done), five recipes, the
 * three households, the week's sessions (Okafor recurring weekly, Lindqvist
 * moved Wed → Thu, Castellano plus a repeat booking), the Okafor grocery
 * list and a draft invoice for the Okafor session. Refuses to run against
 * anything but localhost / 127.0.0.1.
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const base = (process.argv[2] || 'http://localhost:4310').replace(/\/$/, '');
const host = new URL(base).hostname;
if (!['localhost', '127.0.0.1'].includes(host)) {
  console.error(`Refusing to seed ${base}: the demo only runs against a local, disposable instance.`);
  process.exit(2);
}
const week = JSON.parse(fs.readFileSync(path.join(path.dirname(fileURLToPath(import.meta.url)), 'fictional-week.json'), 'utf8'));

let cookie = '';
async function api(method, url, body) {
  const res = await fetch(base + url, {
    method,
    headers: { 'content-type': 'application/json', ...(cookie ? { cookie } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  });
  const set = res.headers.get('set-cookie');
  if (set && set.startsWith('authToken=')) cookie = set.split(';')[0];
  const text = await res.text();
  let json = null;
  try { json = text ? JSON.parse(text) : null; } catch { /* not JSON */ }
  if (!res.ok) throw new Error(`${method} ${url} → ${res.status} ${text.slice(0, 300)}`);
  return json;
}

/** "2 cup chickpeas" → { qty: 2, unit: "cup", name: "chickpeas" } */
function parseIngredient(s) {
  const m = s.match(/^([\d.]+)\s+(cup|tbsp|tsp|lb|oz|can|clove)?\s*(.+)$/);
  if (!m) return { qty: null, unit: '', name: s };
  return { qty: Number(m[1]), unit: m[2] || '', name: m[3] };
}
const fmtQty = (q) => String(Math.round(q * 100) / 100);
function ingredientRows(recipe) {
  return recipe.ingredients.map((s) => {
    const { qty, unit, name } = parseIngredient(s);
    const per = qty == null ? null : qty / recipe.servings;
    const at = (n) => (per == null ? '' : `${fmtQty(per * n)}${unit ? ` ${unit}` : ''}`);
    return { name, qty_4_serving: at(4), qty_6_serving: at(6) };
  });
}

const chef = week.chef;
const exists = await fetch(`${base}/api/auth/login`, {
  method: 'POST', headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ email: chef.login_email, password: chef.password }),
});
if (exists.ok) {
  console.error(`${chef.login_email} already exists on ${base}; start from a fresh database (env.mjs --fresh).`);
  process.exit(1);
}

await api('POST', '/api/auth/signup', { email: chef.login_email, password: chef.password, business_name: chef.business_name });
await api('PUT', '/api/settings', {
  business_name: chef.business_name, chef_name: chef.chef_name, email: chef.business_email,
  location: chef.location, timezone: chef.timezone, deposit_cents: 5000, cancellation_hours: 48,
});
await api('POST', '/api/onboarding/complete');

const recipes = {};
for (const r of week.recipes) {
  const saved = await api('POST', '/api/recipes', { name: r.name, servings_base: r.servings, ingredients: ingredientRows(r) });
  recipes[r.name] = saved.id;
}

const clients = {};
for (const h of week.households) {
  const [adults, children] = h.serving_size === 4 ? [2, 2] : h.serving_size === 2 ? [2, 0] : [4, 2];
  const saved = await api('POST', '/api/clients', {
    name: h.name, email: h.email, adults, children, serving_size: h.serving_size,
    allergens: h.allergens, dietary_styles: h.dietary ? ['Vegetarian'] : [],
    disliked_foods: h.preferences, spice_tolerance: /spicy/i.test(h.preferences) ? 'Hot' : 'Mild',
  });
  clients[h.name] = saved.id;
}

const session = (client, date, time, dishes, extra = {}) => api('POST', '/api/sessions', {
  client_id: clients[client], session_date: date, session_time: time,
  recipe_ids: dishes.map((d) => recipes[d]), ...extra,
});
const okafor = await session('Okafor Household', '2026-12-07', '10:00',
  ['Garlic Lemon Chicken Rice Bowls', 'Chickpea & Spinach Curry'], { repeat_every_weeks: 1 });
const lindqvist = await session('Lindqvist Household', '2026-12-09', '13:00', ['Roasted Vegetable Tray Bake', 'Chickpea & Spinach Curry']);
// Schedule change Wed → Thu. Through the API only: the app's screens can't
// edit a saved session (rehearsal finding; see DEFECTS.md).
await api('PUT', `/api/sessions/${lindqvist.id}`, { session_date: '2026-12-10' });
await session('Castellano Household', '2026-12-11', '15:00', ['Spicy Black Bean Chili']);
await session('Castellano Household', '2026-12-18', '15:00', ['Spicy Black Bean Chili']); // repeat booking

await api('POST', `/api/sessions/${okafor.id}/grocery/generate`);
const invoice = await api('POST', `/api/invoices/from-session/${okafor.id}`);

console.log(`Seeded ${base}:`);
console.log(`  chef ${chef.login_email} (password in fictional-week.json)`);
console.log(`  ${Object.keys(recipes).length} recipes, ${Object.keys(clients).length} households`);
console.log(`  Okafor session ${base}/sessions/${okafor.id} · draft invoice ${base}/invoices/${invoice.id ?? invoice.invoice?.id}`);
