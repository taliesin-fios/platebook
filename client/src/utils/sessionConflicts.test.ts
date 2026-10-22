// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { timeToMinutes, sessionsOverlap, findConflictDates } from './sessionConflicts';
import type { SessionStatus } from '../types';

const s = (session_time: string | null, session_window_hours: number | null = 3, status: SessionStatus = 'Confirmed') =>
  ({ session_time, session_window_hours, status });

describe('timeToMinutes', () => {
  it('parses HH:MM and HH:MM:SS', () => {
    expect(timeToMinutes('09:30')).toBe(570);
    expect(timeToMinutes('17:00:00')).toBe(1020);
  });
  it('returns null for blank or invalid times', () => {
    expect(timeToMinutes(null)).toBeNull();
    expect(timeToMinutes('')).toBeNull();
    expect(timeToMinutes('noon')).toBeNull();
    expect(timeToMinutes('25:00')).toBeNull();
  });
});

describe('sessionsOverlap', () => {
  it('9am and 5pm with 3h windows do not overlap', () => {
    expect(sessionsOverlap(s('09:00'), s('17:00'))).toBe(false);
  });
  it('back-to-back windows do not overlap', () => {
    expect(sessionsOverlap(s('09:00'), s('12:00'))).toBe(false);
  });
  it('overlapping windows conflict', () => {
    expect(sessionsOverlap(s('09:00'), s('11:00'))).toBe(true);
    expect(sessionsOverlap(s('09:00', 8), s('16:00'))).toBe(true);
  });
  it('a missing time conflicts with anything that day', () => {
    expect(sessionsOverlap(s(null), s('17:00'))).toBe(true);
  });
  it('a missing window uses the 3h default', () => {
    expect(sessionsOverlap(s('09:00', null), s('11:59'))).toBe(true);
    expect(sessionsOverlap(s('09:00', null), s('12:00'))).toBe(false);
  });
});

describe('findConflictDates', () => {
  it('flags only days with overlapping, non-cancelled sessions', () => {
    const result = findConflictDates({
      '2026-10-01': [s('09:00'), s('17:00')],
      '2026-10-02': [s('09:00'), s('10:00')],
      '2026-10-03': [s('09:00'), s('10:00', 3, 'Cancelled')],
      '2026-10-04': [s('09:00')],
    });
    expect([...result]).toEqual(['2026-10-02']);
  });
});
