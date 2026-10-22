// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import type { SessionListItem } from '../types';

type ConflictCandidate = Pick<SessionListItem, 'session_time' | 'session_window_hours' | 'status'>;

/** Window used when a session has no `session_window_hours` (the column's default). */
export const DEFAULT_WINDOW_HOURS = 3;

/** "HH:MM" (or "HH:MM:SS") → minutes after midnight; null when blank or malformed. */
export function timeToMinutes(time: string | null | undefined): number | null {
  const m = /^(\d{1,2}):(\d{2})/.exec(time?.trim() ?? '');
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  if (h > 23 || min > 59) return null;
  return h * 60 + min;
}

/**
 * True when two sessions on the same day can't both happen: their
 * [start, start + window) ranges overlap. A session without a time could be
 * at any hour, so it conflicts with any other session that day.
 */
export function sessionsOverlap(a: ConflictCandidate, b: ConflictCandidate): boolean {
  const aStart = timeToMinutes(a.session_time);
  const bStart = timeToMinutes(b.session_time);
  if (aStart === null || bStart === null) return true;
  const aEnd = aStart + (a.session_window_hours ?? DEFAULT_WINDOW_HOURS) * 60;
  const bEnd = bStart + (b.session_window_hours ?? DEFAULT_WINDOW_HOURS) * 60;
  return aStart < bEnd && bStart < aEnd;
}

/** Dates (keys of `grouped`) that hold at least two overlapping, non-cancelled sessions. */
export function findConflictDates(grouped: Record<string, ConflictCandidate[]>): Set<string> {
  const conflicts = new Set<string>();
  for (const [date, all] of Object.entries(grouped)) {
    const sessions = all.filter(s => s.status !== 'Cancelled');
    outer: for (let i = 0; i < sessions.length; i++) {
      for (let j = i + 1; j < sessions.length; j++) {
        if (sessionsOverlap(sessions[i], sessions[j])) {
          conflicts.add(date);
          break outer;
        }
      }
    }
  }
  return conflicts;
}
