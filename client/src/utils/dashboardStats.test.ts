// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { invoiceSummary, marginTile } from './dashboardStats';

describe('marginTile', () => {
  it('shows a dash, not a red 0%, with no sessions this month', () => {
    expect(marginTile(0, 0, 0)).toEqual({ value: '—', sub: 'No sessions this month', valueClass: 'text-gray-400' });
  });

  it('shows a dash when this month\'s sessions have no fees yet', () => {
    expect(marginTile(2, 0, 0)).toEqual({ value: '—', sub: 'No fees set this month', valueClass: 'text-gray-400' });
  });

  it('rates a real margin', () => {
    expect(marginTile(3, 900, 65)).toEqual({ value: '65%', sub: '✓ Healthy', valueClass: 'text-green-dark' });
    expect(marginTile(1, 300, 12.5)).toEqual({ value: '12.5%', sub: '↓ Review costs', valueClass: 'text-danger' });
  });

  it('still rates a real loss (fees below cost)', () => {
    expect(marginTile(1, 100, -20).value).toBe('-20%');
  });
});

describe('invoiceSummary', () => {
  const fmt = (v: number) => `$${v}`;

  it('says what is still owed', () => {
    expect(invoiceSummary(500, 120, fmt)).toBe('$120 outstanding');
  });

  it('says all paid only when something was invoiced', () => {
    expect(invoiceSummary(500, 0, fmt)).toBe('All invoices paid');
  });

  it('says there are no invoices on a new account', () => {
    expect(invoiceSummary(0, 0, fmt)).toBe('No invoices yet');
  });
});
