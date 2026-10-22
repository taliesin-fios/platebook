// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { addDays, format } from 'date-fns';

/**
 * C18: parseISO('YYYY-MM-DD') creates UTC midnight, which shifts to the previous day
 * in US/Western time zones. Appending T12:00:00 anchors the parse to local noon,
 * so the date displays correctly in any time zone.
 */
export function parseLocalDate(str: string | null | undefined): Date {
  if (!str) return new Date('invalid');
  const dateOnly = String(str).slice(0, 10);
  return new Date(dateOnly + 'T12:00:00');
}

/** Today's date as YYYY-MM-DD in the viewer's local time zone (not UTC). */
export function localTodayStr(): string {
  return format(new Date(), 'yyyy-MM-dd');
}

/** The local date `days` from today as YYYY-MM-DD (negative for the past). */
export function localDateStrOffset(days: number, from: Date = new Date()): string {
  return format(addDays(from, days), 'yyyy-MM-dd');
}

/** Format a YYYY-MM-DD date string in local time, returning fallback on error. */
export function fmtSessionDate(
  str: string | null | undefined,
  fmt: string,
  fallback: string | null | undefined = str,
): string {
  try {
    return format(parseLocalDate(str), fmt);
  } catch {
    return fallback || str || '';
  }
}

/**
 * Format a SQLite UTC timestamp ('YYYY-MM-DD HH:MM:SS', from datetime('now'))
 * converted to the viewer's local time. Without the 'Z' suffix the string
 * would be parsed as local time and displayed hours off.
 */
export function fmtUtcTimestamp(ts: string | null | undefined, fmt: string, fallback = ''): string {
  if (!ts) return fallback;
  try {
    const iso = String(ts).replace(' ', 'T') + (String(ts).endsWith('Z') ? '' : 'Z');
    return format(new Date(iso), fmt);
  } catch {
    return fallback;
  }
}
