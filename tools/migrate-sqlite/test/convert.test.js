// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Value conversions of migrate-to-postgres.js (no database needed).
 * The end-to-end run against Postgres and server-rs is test/e2e.mjs.
 */
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { convert } = require('../migrate-to-postgres');

const col = (base, extra = {}) => ({ base, type: base, notNull: false, scale: null, defaultExpr: null, ...extra });

test('0/1 flags become booleans; anything else is an error', () => {
  assert.deepEqual(convert(1, col('bool')), { value: true });
  assert.deepEqual(convert(0, col('bool')), { value: false });
  assert.deepEqual(convert('true', col('bool')), { value: true });
  assert.ok(convert(2, col('bool')).error);
  assert.ok(convert('yes', col('bool')).error);
});

test('JSON text is parsed and re-serialized; invalid JSON is an error', () => {
  assert.deepEqual(convert('["Peanuts", "Soy"]', col('jsonb')), { value: '["Peanuts","Soy"]' });
  assert.deepEqual(convert('{"a":1}', col('jsonb')), { value: '{"a":1}' });
  assert.equal(convert('not json', col('jsonb')).error, 'invalid JSON');
  assert.deepEqual(convert('', col('jsonb', { defaultExpr: "'[]'::jsonb" })), { value: '[]', fixup: "blank → '[]' (the column default)" });
});

test('SQLite datetime text is UTC; ISO strings keep their zone', () => {
  assert.deepEqual(convert('2026-03-04 05:06:07', col('timestamptz')), { value: '2026-03-04T05:06:07Z' });
  assert.deepEqual(convert('2026-03-04T05:06:07.123Z', col('timestamptz')), { value: '2026-03-04T05:06:07.123Z' });
  assert.deepEqual(convert('2026-03-04T05:06:07+02:00', col('timestamptz')), { value: '2026-03-04T05:06:07+02:00' });
  assert.equal(convert('2026-03-04', col('timestamptz')).fixup, 'date only → midnight UTC');
  assert.ok(convert('2026-02-30 10:00:00', col('timestamptz')).error);
  assert.ok(convert('yesterday', col('timestamptz')).error);
});

test('dates must be real calendar dates', () => {
  assert.deepEqual(convert('2026-11-03', col('date')), { value: '2026-11-03' });
  assert.ok(convert('2026-02-30', col('date')).error);
  assert.ok(convert('11/03/2026', col('date')).error);
  assert.deepEqual(convert('  ', col('date')), { value: null, fixup: 'blank → NULL' });
});

test('dollars become numerics; sub-cent values are flagged as rounded', () => {
  assert.deepEqual(convert(200.5, col('numeric', { scale: 2 })), { value: 200.5 });
  assert.deepEqual(convert('60.25', col('numeric', { scale: 2 })), { value: 60.25 });
  assert.deepEqual(convert(0.1 + 0.2, col('numeric', { scale: 2 })), { value: 0.1 + 0.2 });
  assert.equal(convert(1.005001, col('numeric', { scale: 2 })).fixup, 'rounded to 2 decimal places');
  assert.ok(convert('abc', col('numeric', { scale: 2 })).error);
});

test('integers are range-checked; text rejects NUL bytes and BLOBs', () => {
  assert.deepEqual(convert(42, col('int4')), { value: 42 });
  assert.ok(convert(4.5, col('int4')).error);
  assert.ok(convert(2 ** 31, col('int4')).error);
  assert.deepEqual(convert(5550100, col('text')), { value: '5550100' });
  assert.ok(convert('a\u0000b', col('text')).error);
  assert.ok(convert(Buffer.from('x'), col('text')).error);
  assert.deepEqual(convert(null, col('int8')), { value: null });
});
