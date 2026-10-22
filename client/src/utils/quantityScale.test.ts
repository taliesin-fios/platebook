// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { parseQtyStr, formatQtyNum, scaleQty, deriveQty6 } from './quantityScale';

describe('parseQtyStr', () => {
  it('parses a whole number with unit', () => {
    expect(parseQtyStr('2 cups')).toEqual({ value: 2, unit: 'cups' });
  });

  it('parses a bare fraction', () => {
    expect(parseQtyStr('1/2 cup')).toEqual({ value: 0.5, unit: 'cup' });
  });

  it('parses a mixed number', () => {
    expect(parseQtyStr('1 1/2 cups')).toEqual({ value: 1.5, unit: 'cups' });
  });

  it('parses a decimal', () => {
    expect(parseQtyStr('2.5 cups')).toEqual({ value: 2.5, unit: 'cups' });
  });

  it('parses a range, using the lower bound and flagging isRange', () => {
    const r = parseQtyStr('2-3 cloves')!;
    expect(r.value).toBe(2);
    expect(r.rangeHi).toBe(3);
    expect(r.isRange).toBe(true);
  });

  it('treats "to taste" as unparseable with a null value', () => {
    expect(parseQtyStr('to taste')).toEqual({ value: null, unit: 'to taste' });
  });

  it('guards a zero denominator instead of dividing by zero', () => {
    expect(parseQtyStr('1/0 cup')).toBeNull();
  });

  it('returns null for an empty or placeholder string', () => {
    expect(parseQtyStr('')).toBeNull();
    expect(parseQtyStr('—')).toBeNull();
  });

  it('parses a bare number with no unit', () => {
    expect(parseQtyStr('3')).toEqual({ value: 3, unit: '' });
  });
});

describe('formatQtyNum', () => {
  it('formats a whole number', () => {
    expect(formatQtyNum(2, 'cups')).toBe('2 cups');
  });

  it('formats a clean fraction', () => {
    expect(formatQtyNum(0.5, 'cup')).toBe('1/2 cup');
  });

  it('formats a mixed number', () => {
    expect(formatQtyNum(1.5, 'cups')).toBe('1 1/2 cups');
  });

  it('rounds to the nearest 1/8 and carries into the whole number on overflow', () => {
    // 1.99 rounds to 2 — the fractional part (0.99) rounds up to 1/1,
    // which must carry into the whole number rather than showing "1 8/8"
    expect(formatQtyNum(1.99, 'cups')).toBe('2 cups');
  });

  it('omits the unit when there is none', () => {
    expect(formatQtyNum(3, '')).toBe('3');
  });

  it('returns the unit alone for a null/undefined value', () => {
    expect(formatQtyNum(null, 'to taste')).toBe('to taste');
  });
});

describe('scaleQty', () => {
  it('scales a simple quantity up (unit string is preserved as-is, not pluralized)', () => {
    expect(scaleQty('1 cup', 4, 8)).toBe('2 cup');
  });

  it('scales a mixed-fraction quantity', () => {
    expect(scaleQty('1 1/2 cups', 4, 6)).toBe('2 1/4 cups');
  });

  it('returns the original string unchanged when servings match', () => {
    expect(scaleQty('1 1/2 cups', 4, 4)).toBe('1 1/2 cups');
  });

  it('passes through unparseable quantities untouched', () => {
    expect(scaleQty('to taste', 4, 8)).toBe('to taste');
  });

  it('scales both bounds of a range and renders as lo–hi', () => {
    expect(scaleQty('2-4 cloves', 4, 8)).toBe('4–8 cloves');
  });

  it('handles empty input gracefully', () => {
    expect(scaleQty('', 4, 8)).toBe('');
    expect(scaleQty(null, 4, 8)).toBe('');
  });
});

describe('deriveQty6', () => {
  it('multiplies the 4-serving quantity by 1.5', () => {
    expect(deriveQty6('2 cups')).toBe('3 cups');
  });

  it('rounds to a clean fraction (unit string is preserved as-is, not pluralized)', () => {
    expect(deriveQty6('1 cup')).toBe('1 1/2 cup');
  });

  it('passes through unparseable strings unchanged', () => {
    expect(deriveQty6('a pinch')).toBe('a pinch');
  });
});
