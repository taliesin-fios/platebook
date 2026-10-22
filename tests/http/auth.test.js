// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Accounts: signup, password login, reset, onboarding flags.
 */
const { test, describe } = require('node:test');
const assert = require('node:assert/strict');
const jwt = require('jsonwebtoken');
const { request, app, makeAccount, as, signOneTime, JWT_SECRET } = require('./helpers');

const cookieFrom = (res) => (res.headers['set-cookie'] || []).find(c => c.startsWith('authToken='));

describe('signup', () => {
  test('reports whether sign-up is open, without a session', async () => {
    const res = await request(app).get('/api/auth/signup');
    assert.equal(res.status, 200);
    assert.deepEqual(res.body, { open: true }); // SIGNUP_MODE defaults to open
  });

  test('creates an account, settings row and session cookie', async () => {
    const res = await request(app).post('/api/auth/signup').send({
      email: 'New.Chef@Example.com', password: 'correct-horse-battery', business_name: 'Sunday Suppers',
    });
    assert.equal(res.status, 201);
    assert.equal(res.body.email, 'new.chef@example.com');
    assert.equal(res.body.slug, 'sunday-suppers');
    assert.equal(res.body.onboarding_completed, false);
    assert.ok(cookieFrom(res));

    // Checked through the API so the same test runs against either server.
    const settings = await request(app).get('/api/settings').set('Cookie', cookieFrom(res).split(';')[0]);
    assert.equal(settings.status, 200);
    assert.equal(settings.body.account_id, res.body.id);
    assert.equal(settings.body.business_name, 'Sunday Suppers');
    assert.equal(settings.body.invoice_prefix, 'INV');
  });

  test('rejects a duplicate email (case-insensitive)', async () => {
    const res = await request(app).post('/api/auth/signup').send({
      email: 'NEW.CHEF@example.com', password: 'another-long-password',
    });
    assert.equal(res.status, 409);
    assert.equal(res.body.code, 'EMAIL_TAKEN');
  });

  test('rejects a short password', async () => {
    const res = await request(app).post('/api/auth/signup').send({ email: 'short@example.com', password: 'abc' });
    assert.equal(res.status, 400);
  });

  test('rejects an invalid email', async () => {
    const res = await request(app).post('/api/auth/signup').send({ email: 'nope', password: 'long-enough-password' });
    assert.equal(res.status, 400);
  });
});

describe('login', () => {
  test('succeeds with the right password', async () => {
    const res = await request(app).post('/api/auth/login').send({
      email: 'new.chef@example.com', password: 'correct-horse-battery',
    });
    assert.equal(res.status, 200);
    assert.ok(cookieFrom(res));
  });

  test('fails with the wrong password', async () => {
    const res = await request(app).post('/api/auth/login').send({
      email: 'new.chef@example.com', password: 'wrong-password-here',
    });
    assert.equal(res.status, 401);
    assert.equal(res.body.code, 'INVALID_CREDENTIALS');
  });

  test('fails for an unknown email with the same message', async () => {
    const res = await request(app).post('/api/auth/login').send({
      email: 'ghost@example.com', password: 'whatever-password',
    });
    assert.equal(res.status, 401);
    assert.equal(res.body.error, 'Incorrect email or password.');
  });
});

describe('sessions', () => {
  test('a pre-accounts cookie (no accountId) is rejected', async () => {
    const legacy = jwt.sign({ email: 'chef@example.com' }, JWT_SECRET, { expiresIn: '7d' });
    const res = await request(app).get('/api/clients').set('Cookie', `authToken=${legacy}`);
    assert.equal(res.status, 401);
  });

  test('/me reports onboarding state, and /complete flips it', async () => {
    const acct = await makeAccount('Onboarding Co');
    const me1 = await as(acct).get('/api/auth/me');
    assert.equal(me1.body.onboarding_completed, false);
    assert.equal(me1.body.slug, 'onboarding-co');

    assert.equal((await as(acct).post('/api/onboarding/complete')).status, 200);
    const me2 = await as(acct).get('/api/auth/me');
    assert.equal(me2.body.onboarding_completed, true);
  });
});

describe('password reset', () => {
  test('forgot-password always responds uniformly', async () => {
    const res = await request(app).post('/api/auth/forgot-password').send({ email: 'ghost@example.com' });
    assert.equal(res.status, 200);
    assert.equal(res.body.sent, true);
  });

  test('a valid reset token sets a new password exactly once', async () => {
    const acct = await makeAccount('Reset Kitchen');
    const token = signOneTime(acct, 'password-reset', '30m');

    const res = await request(app).post('/api/auth/reset-password').send({ token, password: 'brand-new-password' });
    assert.equal(res.status, 200);

    const login = await request(app).post('/api/auth/login').send({ email: acct.email, password: 'brand-new-password' });
    assert.equal(login.status, 200);

    const reuse = await request(app).post('/api/auth/reset-password').send({ token, password: 'another-new-password' });
    assert.equal(reuse.status, 400);
    assert.equal(reuse.body.code, 'TOKEN_USED');
  });

  test('a magic-link token cannot be used as a reset token', async () => {
    const acct = await makeAccount('Wrong Purpose');
    const token = signOneTime(acct, 'magic-login', '15m');
    const res = await request(app).post('/api/auth/reset-password').send({ token, password: 'brand-new-password' });
    assert.equal(res.status, 400);
  });
});

describe('onboarding seed', () => {
  test('loads the starter library and a sample client once', async () => {
    const acct = await makeAccount('Seed Kitchen');
    const first = await as(acct).post('/api/onboarding/seed').send({ starter_library: true, sample_client: true });
    assert.equal(first.status, 200);
    assert.ok(first.body.library.recipes > 10);
    assert.ok(first.body.library.templates >= 1);
    assert.equal(first.body.sample_client, true);

    const again = await as(acct).post('/api/onboarding/seed').send({ starter_library: true, sample_client: true });
    assert.equal(again.body.library.recipes, 0);
    assert.equal(again.body.sample_client, false);

    const recipes = await as(acct).get('/api/recipes');
    assert.ok(recipes.body.length > 10);
  });

  test('clear-sample removes the demo client even when it has an invoice', async () => {
    const acct = await makeAccount('Clear Kitchen');
    await as(acct).post('/api/onboarding/seed').send({ sample_client: true });
    const [sample] = (await as(acct).get('/api/clients')).body;
    const session = await as(acct).post('/api/sessions').send({ client_id: sample.id, service_fee: 100 });
    await as(acct).post(`/api/invoices/from-session/${session.body.id}`);

    const res = await as(acct).delete('/api/settings/clear-sample');
    assert.equal(res.status, 200);
    assert.equal(res.body.removed, 1);
    assert.equal((await as(acct).get('/api/clients')).body.length, 0);
  });
});

describe('settings', () => {
  test('booking policy fields round-trip and appear on the public profile', async () => {
    const acct = await makeAccount('Policy Kitchen');
    const put = await as(acct).put('/api/settings').send({
      deposit_cents: 7500, cancellation_hours: 72, payment_methods: 'Venmo', booking_terms: 'Be nice.',
      service_types: ['Weekly meal prep'],
    });
    assert.equal(put.status, 200);
    assert.deepEqual(put.body.service_types, ['Weekly meal prep']);

    const pub = await request(app).get(`/api/public/${acct.slug}/settings`);
    assert.equal(pub.body.deposit_cents, 7500);
    assert.equal(pub.body.cancellation_hours, 72);
    assert.equal(pub.body.booking_terms, 'Be nice.');
  });

  test('rejects out-of-range values', async () => {
    const acct = await makeAccount('Range Kitchen');
    const res = await as(acct).put('/api/settings').send({ deposit_cents: -5 });
    assert.equal(res.status, 400);
  });
});
