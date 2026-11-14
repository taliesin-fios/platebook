// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { format, addDays } from 'date-fns';
import { parseLocalDate, localTodayStr } from './dateUtils';

const WEEKDAYS = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday'];

/**
 * The next date that falls on a client's preferred day ("Tuesday"), as
 * YYYY-MM-DD, strictly after `today`: a session is not suggested for the
 * day it is being planned on. '' when the day isn't a weekday name.
 */
export function nextDateForWeekday(day: string | null | undefined, today: string = localTodayStr()): string {
  const target = WEEKDAYS.indexOf((day ?? '').trim().toLowerCase());
  if (target < 0) return '';
  const from = parseLocalDate(today);
  if (Number.isNaN(from.getTime())) return '';
  const ahead = ((target - from.getDay() + 7) % 7) || 7;
  return format(addDays(from, ahead), 'yyyy-MM-dd');
}

/**
 * A client's preferred time as a value for a time input ("14:30"), or ''
 * when it is free text such as "Morning".
 */
export function defaultTimeFor(preferredTime: string | null | undefined): string {
  const m = /^(\d{2}):(\d{2})(?::\d{2})?$/.exec((preferredTime ?? '').trim());
  if (!m || Number(m[1]) > 23 || Number(m[2]) > 59) return '';
  return `${m[1]}:${m[2]}`;
}
