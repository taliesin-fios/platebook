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

/**
 * "in about 5 hours" / "in under an hour" until a `YYYY-MM-DD HH:MM:SS` UTC
 * timestamp; `fallback` when it's missing, unreadable or already past.
 */
export function fmtTimeUntilUtc(ts: string | null | undefined, fallback: string, now: Date = new Date()): string {
  if (!ts) return fallback;
  const iso = String(ts).replace(' ', 'T') + (String(ts).endsWith('Z') ? '' : 'Z');
  const ms = new Date(iso).getTime() - now.getTime();
  if (!Number.isFinite(ms) || ms <= 0) return fallback;
  const hours = Math.round(ms / 3_600_000);
  if (hours < 1) return 'in under an hour';
  if (hours < 48) return `in about ${hours} hour${hours === 1 ? '' : 's'}`;
  return `in about ${Math.round(hours / 24)} days`;
}

/**
 * A session time for display: "14:30" → "2:30 PM". Session times are free
 * text on older rows, so anything that isn't HH:MM is returned as typed.
 */
export function fmtSessionTime(time: string | null | undefined, fallback = '—'): string {
  const raw = (time ?? '').trim();
  if (!raw) return fallback;
  const m = /^(\d{1,2}):(\d{2})(?::\d{2})?$/.exec(raw);
  if (!m) return raw;
  const hours = Number(m[1]);
  const minutes = Number(m[2]);
  if (hours > 23 || minutes > 59) return raw;
  return `${hours % 12 === 0 ? 12 : hours % 12}:${m[2]} ${hours < 12 ? 'AM' : 'PM'}`;
}

/**
 * The line under a client's name in the client list. `last_session_date` is
 * the client's latest session, which may still be ahead (and need not be the
 * next one): "Booked through Nov 28, 2026" from today on, "Last: Nov 13, 2026"
 * once it has passed.
 */
export function clientSessionLabel(date: string | null | undefined, today: string = localTodayStr()): string {
  if (!date) return '';
  const day = String(date).slice(0, 10);
  return `${day >= today ? 'Booked through' : 'Last:'} ${fmtSessionDate(day, 'MMM d, yyyy')}`;
}
