// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Multi-tenant isolation — one chef must never see or touch another chef's data.
 */
const { test, describe, before } = require('node:test');
const assert = require('node:assert/strict');
const { request, app, makeAccount, as } = require('./helpers');

let A, B, a, b;
const ids = {};

before(async () => {
  A = await makeAccount('Alpha Kitchen');
  B = await makeAccount('Beta Kitchen');
  a = as(A);
  b = as(B);

  // Account A builds out a little of everything
  const client = await a.post('/api/clients').send({ name: 'Alpha Client', email: 'ac@example.com' });
  ids.client = client.body.id;

  const recipe = await a.post('/api/recipes').send({
    name: 'Alpha Stew', ingredients: [{ name: 'onion', qty_4_serving: '1' }],
  });
  ids.recipe = recipe.body.id;

  const session = await a.post('/api/sessions').send({
    client_id: ids.client, session_date: '2026-11-01', service_fee: 200, recipe_ids: [ids.recipe],
  });
  ids.session = session.body.id;

  const grocery = await a.post(`/api/sessions/${ids.session}/grocery/generate`);
  ids.groceryItem = grocery.body.items[0].id;

  const template = await a.post('/api/templates').send({ name: 'Weeknight', recipe_ids: [ids.recipe] });
  ids.template = template.body.id;

  const invoice = await a.post(`/api/invoices/from-session/${ids.session}`);
  ids.invoice = invoice.body.id;

  const intake = await request(app).post('/api/intake').send({ slug: A.slug, name: 'Lead', terms_accepted: true });
  ids.intake = intake.body.id;
  ids.intakeToken = intake.body.edit_token;
});

describe('setup sanity', () => {
  test('account A created every record', () => {
    for (const [k, v] of Object.entries(ids)) assert.ok(v, `${k} missing`);
  });
});

describe('account B cannot read account A records by id', () => {
  const cases = [
    ['client',   () => `/api/clients/${ids.client}`],
    ['recipe',   () => `/api/recipes/${ids.recipe}`],
    ['session',  () => `/api/sessions/${ids.session}`],
    ['grocery',  () => `/api/sessions/${ids.session}/grocery`],
    ['invoice',  () => `/api/invoices/${ids.invoice}`],
    ['payments', () => `/api/payments/status/${ids.intake}`],
  ];
  for (const [name, url] of cases) {
    test(name, async () => {
      const res = await b.get(url());
      assert.equal(res.status, 404, `${name}: expected 404, got ${res.status}`);
    });
  }

  test('intake (signed in as another chef, no token) is refused', async () => {
    const res = await b.get(`/api/intake/${ids.intake}`);
    assert.equal(res.status, 401);
  });
});

describe('account B cannot modify account A records', () => {
  test('update client', async () => {
    assert.equal((await b.put(`/api/clients/${ids.client}`).send({ name: 'Hacked' })).status, 404);
  });
  test('delete client', async () => {
    assert.equal((await b.delete(`/api/clients/${ids.client}`)).status, 404);
  });
  test('update recipe', async () => {
    assert.equal((await b.put(`/api/recipes/${ids.recipe}`).send({ name: 'Hacked' })).status, 404);
  });
  test('delete session', async () => {
    assert.equal((await b.delete(`/api/sessions/${ids.session}`)).status, 404);
  });
  test('toggle grocery item', async () => {
    assert.equal((await b.put(`/api/grocery/${ids.groceryItem}`).send({ is_checked: true })).status, 404);
  });
  test('delete template', async () => {
    assert.equal((await b.delete(`/api/templates/${ids.template}`)).status, 404);
  });
  test('update invoice', async () => {
    assert.equal((await b.put(`/api/invoices/${ids.invoice}`).send({ status: 'void' })).status, 404);
  });
  test('convert intake', async () => {
    assert.equal((await b.post(`/api/intake/${ids.intake}/convert`)).status, 404);
  });
  test('resend intake confirmation', async () => {
    assert.equal((await b.post(`/api/intake/${ids.intake}/resend-confirmation`)).status, 404);
  });
  test('restore intake', async () => {
    assert.equal((await b.post(`/api/intake/${ids.intake}/restore`)).status, 404);
  });
  test('intake client link', async () => {
    assert.equal((await b.get(`/api/intake/${ids.intake}/client-link`)).status, 404);
  });
  test('intake checkout without the client\'s token', async () => {
    // Another chef is neither the owner nor holding the token.
    assert.equal((await b.post(`/api/intake/${ids.intake}/checkout`)).status, 401);
  });
  test('intake payment timeline', async () => {
    assert.equal((await b.get(`/api/intake/${ids.intake}/payments`)).status, 404);
  });
  test('delete intake', async () => {
    assert.equal((await b.delete(`/api/intake/${ids.intake}`)).status, 404);
  });
  test('record manual payment on intake', async () => {
    const res = await b.post('/api/payments/manual/action').send({ intake_id: ids.intake, action: 'mark_paid_offline' });
    assert.equal(res.status, 404);
  });
});

describe('account B cannot reference account A records in its own writes', () => {
  test('session for A client', async () => {
    const res = await b.post('/api/sessions').send({ client_id: ids.client });
    assert.equal(res.status, 400);
  });
  test('session with A recipe', async () => {
    const own = await b.post('/api/clients').send({ name: 'Beta Client' });
    const res = await b.post('/api/sessions').send({ client_id: own.body.id, recipe_ids: [ids.recipe] });
    assert.equal(res.status, 400);
  });
  test('invoice for A client', async () => {
    assert.equal((await b.post(`/api/invoices/from-client/${ids.client}`)).status, 404);
  });
  test('template with A recipe silently drops it', async () => {
    const res = await b.post('/api/templates').send({ name: 'Sneaky', recipe_ids: [ids.recipe] });
    assert.equal(res.status, 201);
    assert.deepEqual(res.body.recipe_ids, []);
  });
  test('planner ignores A recipes', async () => {
    const res = await b.post('/api/planner/ingredients').send({ recipe_ids: [ids.recipe] });
    assert.equal(res.status, 200);
    assert.equal(res.body.total_items, 0);
  });
});

describe('lists only show the account\'s own rows', () => {
  const lists = ['/api/clients', '/api/recipes', '/api/sessions', '/api/invoices', '/api/intake'];
  for (const url of lists) {
    test(url, async () => {
      const resA = await a.get(url);
      const resB = await b.get(url);
      assert.equal(resA.status, 200);
      assert.ok(resA.body.length >= 1, `${url}: A should see its rows`);
      const aIds = new Set(resA.body.map(r => r.id));
      assert.ok(resB.body.every(r => !aIds.has(r.id)), `${url}: B sees A's rows`);
    });
  }

  test('cost summary is per account', async () => {
    const res = await b.get('/api/costs/by-client');
    assert.ok(res.body.every(r => r.client_name !== 'Alpha Client'));
  });

  test('data export only contains own rows', async () => {
    const res = await b.get('/api/settings/export');
    assert.equal(res.status, 200);
    assert.ok(res.body.clients.every(c => c.name !== 'Alpha Client'));
    assert.ok(res.body.invoices.every(i => i.id !== ids.invoice));
  });

  test('whole-database download no longer exists', async () => {
    const res = await a.get('/api/settings/export-db');
    assert.notEqual(res.status, 200);
  });
});

describe('per-account uniqueness and numbering', () => {
  test('template names may repeat across accounts', async () => {
    const res = await b.post('/api/templates').send({ name: 'Weeknight', recipe_ids: [] });
    assert.equal(res.status, 201);
  });

  test('duplicate template name within one account is rejected', async () => {
    const res = await a.post('/api/templates').send({ name: 'Weeknight', recipe_ids: [] });
    assert.equal(res.status, 409);
  });

  test('invoice numbers are sequenced per account with each account\'s prefix', async () => {
    await b.put('/api/settings').send({ invoice_prefix: 'bk' });
    const own = await b.post('/api/clients').send({ name: 'Numbered Client' });
    const inv = await b.post(`/api/invoices/from-client/${own.body.id}`);
    const year = new Date().getFullYear();
    assert.equal(inv.body.invoice_number, `BK-${year}-0001`);

    const invA = await a.get(`/api/invoices/${ids.invoice}`);
    assert.equal(invA.body.invoice_number, `INV-${year}-0001`);
  });
});

describe('intake ownership', () => {
  test('owning chef can update admin fields without a token (intake-admin fix)', async () => {
    const res = await a.put(`/api/intake/${ids.intake}`).send({ status: 'reviewed_by_chef', internal_notes: 'call Tue' });
    assert.equal(res.status, 200);
    assert.equal(res.body.status, 'reviewed_by_chef');
    assert.equal(res.body.internal_notes, 'call Tue');
  });

  test('the client\'s edit link never returns the chef\'s private fields', async () => {
    const url = `/api/intake/${ids.intake}?token=${ids.intakeToken}`;
    for (const res of [await request(app).get(url), await request(app).put(url).send({ phone: '555-0101' })]) {
      assert.equal(res.status, 200);
      assert.equal(res.body.name, 'Lead');
      for (const key of ['internal_notes', 'payment_notes', 'stripe_session_id', 'account_id']) {
        assert.ok(!(key in res.body), `${key} leaked to the client`);
      }
    }
  });

  test('intake submitted to B\'s slug lands only in B\'s inbox', async () => {
    const res = await request(app).post('/api/intake').send({ slug: B.slug, name: 'Beta Lead', terms_accepted: true });
    assert.equal(res.status, 201);
    const listA = await a.get('/api/intake');
    assert.ok(listA.body.every(i => i.name !== 'Beta Lead'));
    const listB = await b.get('/api/intake');
    assert.ok(listB.body.some(i => i.name === 'Beta Lead'));
  });

  test('unknown slug is rejected', async () => {
    const res = await request(app).post('/api/intake').send({ slug: 'nobody-here', name: 'X', terms_accepted: true });
    assert.equal(res.status, 404);
  });
});
