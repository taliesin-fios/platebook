// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, expect, it } from 'vitest';
import { TOUR_STEPS, hasSeenTour, markTourSeen, tourSeenKey } from './tour';

function fakeStorage() {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => { data.set(k, v); },
  };
}

const blocked = {
  getItem: () => { throw new Error('storage is blocked'); },
  setItem: () => { throw new Error('storage is blocked'); },
};

describe('tour seen', () => {
  it('is unseen until marked, then stays seen', () => {
    const s = fakeStorage();
    expect(hasSeenTour(7, s)).toBe(false);
    markTourSeen(7, s);
    expect(hasSeenTour(7, s)).toBe(true);
    expect([...s.data.keys()]).toEqual([tourSeenKey(7)]);
  });

  it('is kept per account, so a second chef on the same device still gets the tour', () => {
    const s = fakeStorage();
    markTourSeen(7, s);
    expect(hasSeenTour(8, s)).toBe(false);
  });

  it('shows the tour again, without throwing, when the browser refuses storage', () => {
    expect(hasSeenTour(7, blocked)).toBe(false);
    expect(() => markTourSeen(7, blocked)).not.toThrow();
    expect(hasSeenTour(7, null)).toBe(false);
    expect(() => markTourSeen(7, null)).not.toThrow();
  });

  it('falls back to "not seen" where there is no window (server render, tests)', () => {
    expect(hasSeenTour(7)).toBe(false);
    expect(() => markTourSeen(7)).not.toThrow();
  });
});

describe('tour steps', () => {
  it('covers the five main screens, each with a title and a short explanation', () => {
    expect(TOUR_STEPS.map(s => s.title.split(':')[0])).toEqual(['Dashboard', 'Clients', 'Sessions', 'Recipes', 'Invoices']);
    for (const s of TOUR_STEPS) {
      expect(s.body.length).toBeGreaterThan(40);
      expect(s.body.length).toBeLessThan(260);
    }
  });

  it('mentions the grocery list with sessions', () => {
    expect(TOUR_STEPS[2].body).toMatch(/grocery list/);
  });
});
