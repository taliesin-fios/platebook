// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

export interface Trend {
  /** e.g. "▲ 12% vs last month" */
  text: string;
  direction: 'up' | 'down' | 'flat';
}

/**
 * Month-to-date versus the same days of last month (the server sends both).
 * Null when last month had nothing to compare with: a percentage change from
 * $0 means nothing.
 */
export function mtdTrend(current: number, previous: number): Trend | null {
  if (!(previous > 0) || !Number.isFinite(current)) return null;
  const pct = Math.round(((current - previous) / previous) * 100);
  if (pct === 0) return { text: '→ same as last month', direction: 'flat' };
  return pct > 0
    ? { text: `▲ ${pct}% vs last month`, direction: 'up' }
    : { text: `▼ ${Math.abs(pct)}% vs last month`, direction: 'down' };
}
