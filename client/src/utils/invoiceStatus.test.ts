// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, expect, it } from 'vitest';
import { needsFinalConfirm } from './invoiceStatus';

describe('needsFinalConfirm', () => {
  it('asks before a draft or sent invoice becomes paid or void', () => {
    expect(needsFinalConfirm('paid', 'draft', false)).toBe(true);
    expect(needsFinalConfirm('paid', 'sent', false)).toBe(true);
    expect(needsFinalConfirm('void', 'sent', false)).toBe(true);
  });

  it('does not ask for ordinary saves', () => {
    expect(needsFinalConfirm('draft', 'draft', false)).toBe(false);
    expect(needsFinalConfirm('sent', 'draft', false)).toBe(false);
  });

  it('does not ask again once the invoice is paid or void', () => {
    expect(needsFinalConfirm('paid', 'paid', true)).toBe(false);
    expect(needsFinalConfirm('void', 'void', true)).toBe(false);
  });
});
