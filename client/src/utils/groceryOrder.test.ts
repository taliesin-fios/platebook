// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { sinkTicked } from './groceryOrder';

const names = (items: { name: string }[]) => items.map(i => i.name);

describe('sinkTicked', () => {
  it('puts ticked items after the ones still to buy, keeping each group in order', () => {
    const items = [
      { name: 'lemons', is_checked: 1 },
      { name: 'garlic', is_checked: 0 },
      { name: 'carrots', is_checked: 1 },
      { name: 'celery', is_checked: 0 },
    ];
    expect(names(sinkTicked(items))).toEqual(['garlic', 'celery', 'lemons', 'carrots']);
  });

  it('leaves a list with nothing ticked, or everything ticked, in its order', () => {
    const none = [{ name: 'a', is_checked: 0 }, { name: 'b' }];
    const all = [{ name: 'a', is_checked: true }, { name: 'b', is_checked: 1 }];
    expect(names(sinkTicked(none))).toEqual(['a', 'b']);
    expect(names(sinkTicked(all))).toEqual(['a', 'b']);
  });

  it('does not change the list it was given', () => {
    const items = [{ name: 'a', is_checked: 1 }, { name: 'b', is_checked: 0 }];
    sinkTicked(items);
    expect(names(items)).toEqual(['a', 'b']);
  });
});
