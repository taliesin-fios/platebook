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

export interface DayAndTime { date: string; time: string }

/**
 * The wizard's date and time after a client is picked. A field takes the new
 * client's suggestion when it is blank or still holds the suggestion made
 * for the client picked before (`previous`); anything else is the chef's own
 * and is kept. `suggested` is what was filled in this time ('' = nothing),
 * to pass back as `previous` on the next pick.
 */
export function applySuggestion(
  current: DayAndTime, previous: DayAndTime, next: DayAndTime,
): DayAndTime & { suggested: DayAndTime } {
  const open = (now: string, was: string) => !now || (was !== '' && now === was);
  const date = open(current.date, previous.date) ? next.date : current.date;
  const time = open(current.time, previous.time) ? next.time : current.time;
  return {
    date,
    time,
    suggested: {
      date: open(current.date, previous.date) ? next.date : '',
      time: open(current.time, previous.time) ? next.time : '',
    },
  };
}
