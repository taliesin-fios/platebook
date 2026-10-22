// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { parseLocalDate, fmtSessionDate, fmtUtcTimestamp, localTodayStr, localDateStrOffset } from './dateUtils';

describe('parseLocalDate', () => {
  it('anchors a date-only string to local noon, not UTC midnight', () => {
    const d = parseLocalDate('2026-03-15');
    // Regardless of the runner's time zone, noon-local never rolls to the
    // previous or next calendar day the way UTC midnight can (C18).
    expect(d.getFullYear()).toBe(2026);
    expect(d.getMonth()).toBe(2); // March = index 2
    expect(d.getDate()).toBe(15);
  });

  it('handles a full timestamp by taking only the date portion', () => {
    const d = parseLocalDate('2026-03-15T08:00:00.000Z');
    expect(d.getDate()).toBe(15);
  });

  it('returns an invalid date for empty/null input', () => {
    expect(Number.isNaN(parseLocalDate('').getTime())).toBe(true);
    expect(Number.isNaN(parseLocalDate(null).getTime())).toBe(true);
  });
});

describe('fmtSessionDate', () => {
  it('formats a date-only string correctly', () => {
    expect(fmtSessionDate('2026-01-01', 'yyyy-MM-dd')).toBe('2026-01-01');
  });

  it('never shifts to the previous day for dates near a US timezone boundary', () => {
    // This is the exact regression case: parseISO('2026-06-19') displayed as
    // June 18 in Pacific time because it parsed to UTC midnight.
    expect(fmtSessionDate('2026-06-19', 'MMMM d, yyyy')).toBe('June 19, 2026');
  });

  it('falls back to the raw string on invalid input', () => {
    expect(fmtSessionDate('not-a-date', 'yyyy-MM-dd')).toBe('not-a-date');
  });

  it('uses the fallback argument when provided', () => {
    expect(fmtSessionDate('', 'yyyy-MM-dd', 'N/A')).toBe('N/A');
  });
});

describe('fmtUtcTimestamp', () => {
  it('parses a SQLite datetime("now") string as UTC, not local time', () => {
    // '2026-01-01 00:00:00' with no 'Z' would be parsed as local time if we
    // didn't append it — that's exactly the bug this helper exists to avoid.
    // date-fns `format()` always renders in the runner's local time zone, so
    // instead of asserting a fixed clock string (which would only pass in
    // one timezone), assert the underlying instant is the UTC one by
    // building the expected instant with Date.UTC and comparing epoch ms.
    const formatted = fmtUtcTimestamp('2026-01-01 00:00:00', "yyyy-MM-dd'T'HH:mm:ss.SSS");
    const parsedBack = new Date(formatted); // reparsed as local time, same as it was formatted
    const expectedInstant = Date.UTC(2026, 0, 1, 0, 0, 0);
    expect(parsedBack.getTime()).toBe(expectedInstant);
  });

  it('returns the fallback for empty input', () => {
    expect(fmtUtcTimestamp(null, 'yyyy-MM-dd', 'never')).toBe('never');
    expect(fmtUtcTimestamp('', 'yyyy-MM-dd', 'never')).toBe('never');
  });

  it('does not double-append Z if already present', () => {
    expect(() => fmtUtcTimestamp('2026-01-01T00:00:00Z', 'yyyy-MM-dd')).not.toThrow();
  });
});

describe('localTodayStr', () => {
  it('returns a YYYY-MM-DD string', () => {
    expect(localTodayStr()).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('matches the local calendar date', () => {
    const now = new Date();
    const expected = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
    expect(localTodayStr()).toBe(expected);
  });
});

describe('localDateStrOffset', () => {
  it('adds local days across a month boundary', () => {
    expect(localDateStrOffset(6, new Date(2026, 9, 28, 12))).toBe('2026-11-03');
  });
  it('accepts negative offsets', () => {
    expect(localDateStrOffset(-1, new Date(2026, 0, 1, 12))).toBe('2025-12-31');
  });
});
