// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import type { ClientInvoiceView } from '../types';

type PayFields = Pick<ClientInvoiceView, 'status' | 'total' | 'can_pay_online' | 'payment_methods'>;

/**
 * What the client's invoice page says about paying when there's no Pay
 * button: the chef's own payment methods text, or that the chef will be in
 * touch when they haven't set any. `null` when there's nothing to pay (paid,
 * cancelled or a zero total) or the page offers online payment.
 */
export function howToPay(invoice: PayFields): string | null {
  if (invoice.status === 'paid' || invoice.status === 'void') return null;
  if ((Number(invoice.total) || 0) <= 0) return null;
  if (invoice.can_pay_online) return null;
  const methods = (invoice.payment_methods || '').trim();
  return methods || 'Your chef will be in touch to arrange payment.';
}
