// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { nextDateForWeekday, defaultTimeFor, applySuggestion } from './sessionDefaults';

describe('nextDateForWeekday', () => {
  // 2026-11-13 is a Friday.
  it('finds the next such day after today', () => {
    expect(nextDateForWeekday('Tuesday', '2026-11-13')).toBe('2026-11-17');
    expect(nextDateForWeekday('saturday', '2026-11-13')).toBe('2026-11-14');
    expect(nextDateForWeekday(' Thursday ', '2026-11-13')).toBe('2026-11-19');
  });

  it('skips today: the same weekday means a week from now', () => {
    expect(nextDateForWeekday('Friday', '2026-11-13')).toBe('2026-11-20');
  });

  it('crosses a month and a year end', () => {
    expect(nextDateForWeekday('Monday', '2026-11-28')).toBe('2026-11-30');
    expect(nextDateForWeekday('Friday', '2026-12-30')).toBe('2027-01-01');
  });

  it('suggests nothing without a weekday name', () => {
    expect(nextDateForWeekday(null, '2026-11-13')).toBe('');
    expect(nextDateForWeekday('', '2026-11-13')).toBe('');
    expect(nextDateForWeekday('No preference', '2026-11-13')).toBe('');
    expect(nextDateForWeekday('Tuesday', 'not a date')).toBe('');
  });
});

describe('defaultTimeFor', () => {
  it('keeps a clock time', () => {
    expect(defaultTimeFor('10:00')).toBe('10:00');
    expect(defaultTimeFor('14:30:00')).toBe('14:30');
  });

  it('suggests nothing for free text or impossible times', () => {
    expect(defaultTimeFor('Morning')).toBe('');
    expect(defaultTimeFor(null)).toBe('');
    expect(defaultTimeFor('25:00')).toBe('');
    expect(defaultTimeFor('9:00')).toBe('');
  });
});

describe('applySuggestion', () => {
  const none = { date: '', time: '' };
  const alice = { date: '2026-11-23', time: '10:00' };
  const bob = { date: '2026-11-27', time: '14:00' };

  it('fills blank fields with the suggestion for the client', () => {
    expect(applySuggestion(none, none, alice)).toEqual({ ...alice, suggested: alice });
  });

  it('replaces the suggestion made for the last client when another is picked', () => {
    expect(applySuggestion(alice, alice, bob)).toEqual({ ...bob, suggested: bob });
  });

  it('withdraws the last suggestion when the new client has none', () => {
    expect(applySuggestion(alice, alice, none)).toEqual({ ...none, suggested: none });
  });

  it('keeps a date and time the chef set', () => {
    const own = { date: '2026-12-01', time: '09:15' };
    expect(applySuggestion(own, none, bob)).toEqual({ ...own, suggested: none });
    expect(applySuggestion(own, alice, bob)).toEqual({ ...own, suggested: none });
  });

  it('treats each field on its own', () => {
    const linkDate = { date: '2026-12-01', time: '' };
    expect(applySuggestion(linkDate, none, bob))
      .toEqual({ date: '2026-12-01', time: '14:00', suggested: { date: '', time: '14:00' } });
  });
});
