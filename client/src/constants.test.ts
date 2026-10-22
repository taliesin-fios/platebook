// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { detectAllergens, calcFeasibility, marginColor, bookingUrl, dollarsToCents, centsToDollars, repeatLabel } from './constants';

const ing = (...names: string[]) => names.map(name => ({ name }));

describe('detectAllergens', () => {
  it('detects a direct allergen match', () => {
    expect(detectAllergens(ing('whole milk'))).toContain('Dairy');
    expect(detectAllergens(ing('egg yolk'))).toContain('Eggs');
  });

  it('does not flag "eggplant" as an Eggs allergen (A2 regression)', () => {
    expect(detectAllergens(ing('eggplant'))).not.toContain('Eggs');
  });

  it('does not flag "peanut butter" as a Dairy allergen (A3 regression)', () => {
    const detected = detectAllergens(ing('peanut butter'));
    expect(detected).not.toContain('Dairy');
    expect(detected).toContain('Peanuts');
  });

  it('does not flag "cocoa butter" as Dairy', () => {
    expect(detectAllergens(ing('cocoa butter'))).not.toContain('Dairy');
  });

  it('does not flag "eggnog" as Eggs via the exclusion list', () => {
    // eggnog does contain dairy/egg in reality, but the exclusion list
    // intentionally treats it as a named dish, not raw egg — documents
    // current (imperfect) behavior so a future change is a conscious choice.
    expect(detectAllergens(ing('eggnog'))).not.toContain('Eggs');
  });

  it('still flags a real dairy ingredient named "butter" alone', () => {
    expect(detectAllergens(ing('butter'))).toContain('Dairy');
  });

  it('returns an empty array for no ingredients', () => {
    expect(detectAllergens([])).toEqual([]);
    expect(detectAllergens()).toEqual([]);
  });

  it('is case-insensitive', () => {
    expect(detectAllergens(ing('WHOLE MILK'))).toContain('Dairy');
  });

  it('detects multiple distinct allergens across ingredients', () => {
    const detected = detectAllergens(ing('shrimp', 'peanut', 'wheat flour'));
    expect(detected).toContain('Shellfish');
    expect(detected).toContain('Peanuts');
    expect(detected).toContain('Gluten');
  });
});

describe('calcFeasibility', () => {
  it('returns NONE for an empty dish list', () => {
    expect(calcFeasibility([]).level).toBe('NONE');
    expect(calcFeasibility(null).level).toBe('NONE');
  });

  it('rates a short, all-session-friendly menu as GREEN', () => {
    const dishes = [
      { prep_time_min: 15, cook_time_min: 30, session_fit: 'Session-Friendly' },
      { prep_time_min: 15, cook_time_min: 30, session_fit: 'Session-Friendly' },
    ];
    expect(calcFeasibility(dishes).level).toBe('GREEN');
  });

  it('rates a menu exceeding 5 dishes as RED regardless of time', () => {
    const dishes = Array(6).fill({ prep_time_min: 5, cook_time_min: 5, session_fit: 'Session-Friendly' });
    expect(calcFeasibility(dishes).level).toBe('RED');
  });

  it('rates a long total time as RED', () => {
    const dishes = [{ prep_time_min: 120, cook_time_min: 180, session_fit: 'Session-Friendly' }];
    expect(calcFeasibility(dishes).level).toBe('RED');
  });
});

describe('marginColor', () => {
  it('returns green for healthy margin', () => {
    expect(marginColor(65)).toBe('text-green-dark');
  });
  it('returns amber for acceptable margin', () => {
    expect(marginColor(45)).toBe('text-amber-600');
  });
  it('returns danger red for low margin', () => {
    expect(marginColor(20)).toBe('text-danger');
  });
});

describe('bookingUrl', () => {
  it('builds a per-chef link from the slug', () => {
    expect(bookingUrl('sunday-suppers', 'https://app.test')).toBe('https://app.test/book/sunday-suppers');
  });
  it('falls back to the legacy intake link without a slug', () => {
    expect(bookingUrl(null, 'https://app.test')).toBe('https://app.test/intake/new');
  });
});

describe('deposit cents helpers', () => {
  it('converts dollars to integer cents', () => {
    expect(dollarsToCents('50')).toBe(5000);
    expect(dollarsToCents('12.345')).toBe(1235);
    expect(dollarsToCents('')).toBe(0);
    expect(dollarsToCents('-5')).toBe(0);
  });
  it('formats cents as a dollar string', () => {
    expect(centsToDollars(5000)).toBe('50');
    expect(centsToDollars(1250)).toBe('12.50');
    expect(centsToDollars(0)).toBe('0');
  });
});

describe('repeatLabel', () => {
  it('names each repeat interval', () => {
    expect(repeatLabel(null)).toBe('Does not repeat');
    expect(repeatLabel(undefined)).toBe('Does not repeat');
    expect(repeatLabel(1)).toBe('Every week');
    expect(repeatLabel(2)).toBe('Every 2 weeks');
    expect(repeatLabel(6)).toBe('Every 6 weeks');
  });
});
