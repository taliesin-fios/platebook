// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Shared setup for the black-box HTTP suites: the server under test is the
 * Rust API (server-rs) at API_BASE_URL. `node tests/http/run.mjs` (root:
 * `npm run test:http`) boots one on a throwaway Postgres database with the
 * secrets below and runs these suites against it.
 *
 * Helpers to create accounts and act as them over HTTP.
 */
const crypto = require('crypto');
const jwt = require('jsonwebtoken');
const request = require('supertest');

// Node 20's `node --test` talks to each test file over the child's stdout;
// stray logging there can corrupt that stream. Set TEST_LOGS=1 to see it.
if (!process.env.TEST_LOGS) {
  console.log = () => {};
  console.info = () => {};
}

const BASE_URL = process.env.API_BASE_URL;
if (!BASE_URL) {
  throw new Error('API_BASE_URL is not set: run these suites with `npm run test:http` (tests/http/run.mjs)');
}
const app = BASE_URL.replace(/\/$/, '');

// The server under test must use the same secrets (run.mjs passes them).
const JWT_SECRET = 'test-jwt-secret-for-smoke-tests';
process.env.JWT_SECRET = JWT_SECRET;

/** Session cookie token, as the server signs it: `{ accountId, email }`, 7 days. */
const signSession = (account) =>
  jwt.sign({ accountId: account.id, email: account.email }, JWT_SECRET, { expiresIn: '7d' });

/** Single-use token for magic links and password resets (`purpose` + `jti`). */
const signOneTime = (account, purpose, expiresIn) =>
  jwt.sign(
    { accountId: account.id, email: account.email, purpose, jti: crypto.randomUUID() },
    JWT_SECRET,
    { expiresIn }
  );

let counter = 0;

/**
 * Creates an account over HTTP and resolves to `{ id, email, slug }`. Each
 * signup comes from its own client IP so the per-IP signup limit never trips.
 */
async function makeAccount(businessName = 'Test Kitchen') {
  counter += 1;
  const email = `chef${counter}-${process.pid}@example.com`;
  const res = await request(app)
    .post('/api/auth/signup')
    .set('X-Forwarded-For', `10.${(process.pid >> 8) & 255}.${process.pid & 255}.${counter}`)
    .send({ email, password: 'test-password-1234', business_name: businessName });
  if (res.status !== 201) {
    throw new Error(`makeAccount: signup returned ${res.status} ${JSON.stringify(res.body)}`);
  }
  return { id: res.body.id, email: res.body.email, slug: res.body.slug };
}

/** Cookie header value that authenticates as `account`. */
const cookieFor = (account) => `authToken=${signSession(account)}`;

/** supertest wrapper that sends every request as `account`. */
function as(account) {
  const cookie = cookieFor(account);
  const wrap = (method) => (url) => request(app)[method](url).set('Cookie', cookie);
  return { get: wrap('get'), post: wrap('post'), put: wrap('put'), delete: wrap('delete') };
}

module.exports = { app, request, makeAccount, cookieFor, as, signOneTime, JWT_SECRET };
