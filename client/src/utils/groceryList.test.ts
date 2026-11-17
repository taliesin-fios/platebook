// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { regenerateWarning } from './groceryList';

describe('regenerateWarning', () => {
  it('needs no warning when nothing is ticked', () => {
    expect(regenerateWarning([])).toBeNull();
    expect(regenerateWarning([{ is_checked: 0 }, { is_checked: false }, {}])).toBeNull();
  });

  it('counts the ticked items that would be lost', () => {
    expect(regenerateWarning([{ is_checked: 1 }, { is_checked: 0 }])).toBe('The 1 item you have ticked off will be unticked.');
    expect(regenerateWarning([{ is_checked: 1 }, { is_checked: true }, { is_checked: 1 }, { is_checked: 0 }]))
      .toBe('The 3 items you have ticked off will be unticked.');
  });
});
