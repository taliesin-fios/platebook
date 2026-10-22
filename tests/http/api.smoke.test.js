// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Smoke tests — exercise the API over HTTP (server-rs on a throwaway
 * Postgres database, see run.mjs), without touching a dev database.
 */
const { test, describe, before } = require('node:test');
const assert = require('node:assert/strict');
const { app, request, makeAccount } = require('./helpers');

let chef;
before(async () => { chef = await makeAccount('Smoke Test Kitchen'); });

describe('GET /health', () => {
  test('is public and returns ok:true without auth', async () => {
    const res = await request(app).get('/health');
    assert.equal(res.status, 200);
    assert.equal(res.body.ok, true);
  });
});

describe('Content-Security-Policy', () => {
  test('allows the Google Fonts stylesheet and font files', async () => {
    const res = await request(app).get('/api/clients');
    const csp = res.headers['content-security-policy'];
    assert.match(csp, /style-src [^;]*https:\/\/fonts\.googleapis\.com/);
    assert.match(csp, /font-src [^;]*https:\/\/fonts\.gstatic\.com/);
  });
});

describe('Auth gate', () => {
  test('protected route rejects a request with no auth cookie', async () => {
    const res = await request(app).get('/api/clients');
    assert.equal(res.status, 401);
    assert.equal(res.body.code, 'UNAUTHORIZED');
  });

  test('public booking profile does not require auth', async () => {
    const res = await request(app).get(`/api/public/${chef.slug}/settings`);
    assert.equal(res.status, 200);
    assert.equal(res.body.business_name, 'Smoke Test Kitchen');
    assert.equal(res.body.email, undefined, 'must not expose contact details');
  });

  test('public booking profile 404s for an unknown slug', async () => {
    const res = await request(app).get('/api/public/no-such-chef/settings');
    assert.equal(res.status, 404);
  });

  test('full settings endpoint requires auth (SEC6)', async () => {
    const res = await request(app).get('/api/settings');
    assert.equal(res.status, 401);
  });
});

describe('POST /api/intake — public submission', () => {
  let intakeId;
  let editToken;

  test('rejects a submission with no name', async () => {
    const res = await request(app).post('/api/intake').send({ slug: chef.slug, terms_accepted: true });
    assert.equal(res.status, 400);
    assert.equal(res.body.field, 'name');
  });

  test('rejects a submission that has not accepted terms', async () => {
    const res = await request(app).post('/api/intake').send({ slug: chef.slug, name: 'Test Client' });
    assert.equal(res.status, 400);
    assert.equal(res.body.field, 'terms_accepted');
  });

  test('rejects an invalid email format', async () => {
    const res = await request(app).post('/api/intake').send({
      slug: chef.slug, name: 'Test Client', terms_accepted: true, email: 'not-an-email',
    });
    assert.equal(res.status, 400);
    assert.equal(res.body.field, 'email');
  });

  test('accepts a valid submission and returns an edit token', async () => {
    const res = await request(app).post('/api/intake').send({
      slug: chef.slug, name: 'Test Client', email: 'client@example.com', terms_accepted: true,
    });
    assert.equal(res.status, 201);
    assert.equal(res.body.chef_slug, chef.slug);
    assert.ok(res.body.id);
    assert.ok(res.body.edit_token);
    intakeId = res.body.id;
    editToken = res.body.edit_token;
  });

  test('GET with the correct token returns the intake', async () => {
    const res = await request(app).get(`/api/intake/${intakeId}?token=${editToken}`);
    assert.equal(res.status, 200);
    assert.equal(res.body.name, 'Test Client');
  });

  test('GET with no token and no auth is rejected', async () => {
    const res = await request(app).get(`/api/intake/${intakeId}`);
    assert.equal(res.status, 401);
  });

  test('GET with a wrong token is rejected', async () => {
    const res = await request(app).get(`/api/intake/${intakeId}?token=wrongtoken`);
    assert.equal(res.status, 403);
  });

  test('PUT with the correct token can update client-editable fields', async () => {
    const res = await request(app)
      .put(`/api/intake/${intakeId}?token=${editToken}`)
      .send({ phone: '555-0100' });
    assert.equal(res.status, 200);
    assert.equal(res.body.phone, '555-0100');
  });

  test('PUT with a valid token cannot set admin-only fields (status)', async () => {
    const res = await request(app)
      .put(`/api/intake/${intakeId}?token=${editToken}`)
      .send({ status: 'booked' });
    assert.equal(res.status, 200);
    // Status must remain unchanged — token holders aren't chef-authenticated
    assert.notEqual(res.body.status, 'booked');
  });

  test('DELETE requires chef auth — an unauthenticated caller cannot delete an intake', async () => {
    const res = await request(app).delete('/api/intake/999999');
    assert.equal(res.status, 401);
  });
});

describe('Rate limiting', () => {
  test('magic-link request endpoint is reachable and responds uniformly', async () => {
    // Always returns {sent:true} regardless of whether the email matched,
    // to avoid leaking which email is the configured chef account.
    const res = await request(app).post('/api/auth/request-link').send({ email: 'nobody@example.com' });
    assert.equal(res.status, 200);
    assert.equal(res.body.sent, true);
  });
});
