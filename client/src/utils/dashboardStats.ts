// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { marginColor, marginLabel } from '../constants';

export interface MarginTile {
  value: string;
  sub: string;
  valueClass: string;
}

/**
 * The dashboard's Avg Margin tile. A margin needs fees to be a share of:
 * with no sessions this month, or none with a fee, there is nothing to rate,
 * so it shows a dash instead of a red "0% · Review costs".
 */
export function marginTile(sessionCount: number, revenue: number, marginPct: number): MarginTile {
  if (!(sessionCount > 0)) return { value: '—', sub: 'No sessions this month', valueClass: 'text-gray-400' };
  if (!(revenue > 0)) return { value: '—', sub: 'No fees set this month', valueClass: 'text-gray-400' };
  const pct = marginPct || 0;
  return { value: `${pct}%`, sub: marginLabel(pct), valueClass: marginColor(pct) };
}

/**
 * The dashboard's Active Clients tile. Onboarding's sample client is starter
 * content, not a client, so it isn't counted (the line under says so).
 */
export function clientsTile(clients: { is_sample: 0 | 1 }[]): { value: number; sub: string } {
  const samples = clients.filter(c => c.is_sample).length;
  const value = clients.length - samples;
  if (samples > 0) return { value, sub: samples === 1 ? 'Sample client not counted' : `${samples} sample clients not counted` };
  return { value, sub: value === 0 ? 'No clients yet' : '' };
}

/**
 * The line under Collected: what's still owed, or that it's all paid, or
 * (when nothing has been invoiced) that there are no invoices yet, rather
 * than "All invoices paid" on a new account.
 */
export function invoiceSummary(invoicedTotal: number, outstandingTotal: number, fmt: (v: number) => string): string {
  if (outstandingTotal > 0) return `${fmt(outstandingTotal)} outstanding`;
  return invoicedTotal > 0 ? 'All invoices paid' : 'No invoices yet';
}
