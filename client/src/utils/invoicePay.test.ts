// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { howToPay } from './invoicePay';

const due = { status: 'sent', total: 120, can_pay_online: false, payment_methods: 'Venmo or check' } as const;

describe('howToPay', () => {
  it('gives the chef\'s payment methods for an invoice that is due', () => {
    expect(howToPay(due)).toBe('Venmo or check');
    expect(howToPay({ ...due, status: 'draft',payment_methods: '  Zelle  ' })).toBe('Zelle');
  });

  it('says the chef will be in touch when no methods are set', () => {
    expect(howToPay({ ...due, payment_methods: null })).toBe('Your chef will be in touch to arrange payment.');
    expect(howToPay({ ...due, payment_methods: '   ' })).toBe('Your chef will be in touch to arrange payment.');
  });

  it('says nothing when there is nothing to pay', () => {
    expect(howToPay({ ...due, status: 'paid' })).toBeNull();
    expect(howToPay({ ...due, status: 'void' })).toBeNull();
    expect(howToPay({ ...due, total: 0 })).toBeNull();
  });

  it('leaves it to the Pay button when online payment is on', () => {
    expect(howToPay({ ...due, can_pay_online: true })).toBeNull();
  });
});
