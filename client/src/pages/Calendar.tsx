// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router';
import { format, startOfMonth, endOfMonth, eachDayOfInterval,
         getDay, isSameMonth, isToday, isSameDay, addMonths, subMonths } from 'date-fns';
import PageHeader from '../components/PageHeader';
import { STATUS_COLORS } from '../constants';
import { findConflictDates } from '../utils/sessionConflicts';
import type { SessionListItem } from '../types';

type SessionsByDate = Record<string, SessionListItem[]>;

// Returns sessions grouped by YYYY-MM-DD string
function groupByDate(sessions: SessionListItem[]): SessionsByDate {
  return sessions.reduce<SessionsByDate>((acc, s) => {
    const key = s.session_date?.slice(0, 10);
    if (!key) return acc;
    if (!acc[key]) acc[key] = [];
    acc[key].push(s);
    return acc;
  }, {});
}

const STATUS_DOT: Record<string, string> = {
  Pending:   'bg-gray-400',
  Confirmed: 'bg-blue-500',
  Completed: 'bg-green-mid',
  Cancelled: 'bg-gray-300',
};

export default function Calendar() {
  const navigate = useNavigate();
  const [currentMonth, setCurrentMonth] = useState(new Date());
  const [sessions, setSessions]         = useState<SessionListItem[]>([]);
  const [loading, setLoading]           = useState(true);
  const [selectedDate, setSelectedDate] = useState<Date | null>(null);

  useEffect(() => {
    fetch('/api/sessions')
      .then(r => r.ok ? r.json() : Promise.reject(new Error(`HTTP ${r.status}`)))
      .then(d => setSessions(Array.isArray(d) ? d : []))
      .catch(() => setSessions([]))
      .finally(() => setLoading(false));
  }, []);

  const grouped   = groupByDate(sessions);
  const conflicts = findConflictDates(grouped);

  // Build calendar grid
  const monthStart  = startOfMonth(currentMonth);
  const monthEnd    = endOfMonth(currentMonth);
  const days        = eachDayOfInterval({ start: monthStart, end: monthEnd });

  // Leading empty cells so the grid starts on the right weekday
  const leadingBlanks = getDay(monthStart); // 0 = Sun

  // Sessions for the selected date
  const selectedSessions = selectedDate
    ? (grouped[format(selectedDate, 'yyyy-MM-dd')] || [])
    : [];

  const today = new Date();

  return (
    <div className="page pb-24">
      <PageHeader title="Calendar" />

      {/* Month navigator */}
      <div className="flex items-center justify-between px-4 py-3 bg-white border-b border-gray-100">
        <button
          onClick={() => setCurrentMonth(prev => subMonths(prev, 1))}
          className="p-2 rounded-xl hover:bg-green-light text-navy"
          style={{ minHeight: 'auto' }}
        >
          ‹
        </button>
        <span className="font-display font-semibold text-navy">
          {format(currentMonth, 'MMMM yyyy')}
        </span>
        <button
          onClick={() => setCurrentMonth(prev => addMonths(prev, 1))}
          className="p-2 rounded-xl hover:bg-green-light text-navy"
          style={{ minHeight: 'auto' }}
        >
          ›
        </button>
      </div>

      {/* Conflict banner */}
      {conflicts.size > 0 && (
        <div className="mx-4 mt-3 px-3 py-2 bg-amber-50 border border-amber-200 rounded-xl flex items-center gap-2">
          <span className="text-amber-600">⚠</span>
          <span className="text-xs text-amber-700 font-medium">
            {conflicts.size} day{conflicts.size !== 1 ? 's' : ''} with overlapping sessions — check the times
          </span>
        </div>
      )}

      <div className="px-4 mt-3">
        {/* Day-of-week headers */}
        <div className="grid grid-cols-7 mb-1">
          {['Su','Mo','Tu','We','Th','Fr','Sa'].map(d => (
            <div key={d} className="text-center text-xs font-medium text-gray-400 py-1">{d}</div>
          ))}
        </div>

        {/* Calendar grid */}
        {loading ? (
          <div className="space-y-1">
            {[0,1,2,3,4].map(i => (
              <div key={i} className="grid grid-cols-7 gap-1">
                {Array(7).fill(0).map((_, j) => (
                  <div key={j} className="skeleton h-12 rounded-xl" />
                ))}
              </div>
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-7 gap-1">
            {/* Leading blanks */}
            {Array(leadingBlanks).fill(null).map((_, i) => (
              <div key={`blank-${i}`} />
            ))}

            {/* Days */}
            {days.map(day => {
              const key        = format(day, 'yyyy-MM-dd');
              const daySessions= grouped[key] || [];
              const hasConflict= conflicts.has(key);
              const isSelected = selectedDate && isSameDay(day, selectedDate);
              const isTodayDay = isToday(day);

              return (
                <button
                  key={key}
                  onClick={() => setSelectedDate(isSelected ? null : day)}
                  style={{ minHeight: 'auto' }}
                  className={`
                    relative flex flex-col items-center rounded-xl py-1.5 px-0.5 transition-colors
                    ${isSelected          ? 'bg-navy text-white'        : ''}
                    ${!isSelected && isTodayDay ? 'bg-green-light border border-green-mid' : ''}
                    ${!isSelected && !isTodayDay && daySessions.length ? 'bg-white hover:bg-green-light' : ''}
                    ${!isSelected && !isTodayDay && !daySessions.length ? 'hover:bg-gray-50' : ''}
                  `}
                >
                  {/* Date number */}
                  <span className={`text-xs font-semibold ${
                    isSelected    ? 'text-white'
                    : isTodayDay  ? 'text-green-dark'
                    : daySessions.length ? 'text-navy'
                    : 'text-gray-400'
                  }`}>
                    {format(day, 'd')}
                  </span>

                  {/* Session dots */}
                  {daySessions.length > 0 && (
                    <div className="flex gap-0.5 mt-0.5 flex-wrap justify-center max-w-[28px]">
                      {daySessions.slice(0, 3).map((s, i) => (
                        <span
                          key={i}
                          className={`w-1.5 h-1.5 rounded-full ${
                            isSelected ? 'bg-white' : (STATUS_DOT[s.status] || 'bg-gray-400')
                          }`}
                        />
                      ))}
                    </div>
                  )}

                  {/* Conflict indicator */}
                  {hasConflict && !isSelected && (
                    <span className="absolute top-0.5 right-0.5 text-[8px] text-amber-600">⚠</span>
                  )}
                </button>
              );
            })}
          </div>
        )}

        {/* Legend */}
        <div className="flex flex-wrap gap-3 mt-3 px-1">
          {Object.entries(STATUS_DOT).map(([status, dot]) => (
            <div key={status} className="flex items-center gap-1.5">
              <span className={`w-2 h-2 rounded-full ${dot}`} />
              <span className="text-xs text-gray-500">{status}</span>
            </div>
          ))}
          <div className="flex items-center gap-1.5">
            <span className="text-[10px] text-amber-600">⚠</span>
            <span className="text-xs text-gray-500">Conflict</span>
          </div>
        </div>

        {/* Selected-date session list */}
        {selectedDate && (
          <div className="mt-4">
            <div className="section-header">
              {format(selectedDate, 'EEEE, MMMM d')}
              {conflicts.has(format(selectedDate, 'yyyy-MM-dd')) && (
                <span className="ml-2 text-xs font-sans text-amber-600 bg-amber-50 px-2 py-0.5 rounded-full">
                  ⚠ Multiple sessions
                </span>
              )}
            </div>

            {selectedSessions.length === 0 ? (
              <div className="card text-center py-6">
                <p className="text-gray-400 text-sm mb-3">No sessions on this day</p>
                <button
                  onClick={() => navigate(`/sessions/new?date=${format(selectedDate, 'yyyy-MM-dd')}`)}
                  className="px-5 py-2.5 bg-navy text-white rounded-xl text-sm font-semibold"
                >
                  + Schedule Session
                </button>
              </div>
            ) : (
              <div className="space-y-2">
                {selectedSessions.map(s => (
                  <div
                    key={s.id}
                    onClick={() => navigate(`/sessions/${s.id}`)}
                    className="card cursor-pointer hover:shadow-card-hover transition-shadow"
                  >
                    <div className="flex items-start justify-between">
                      <div>
                        <p className="font-semibold text-navy text-sm">{s.client_name || 'Unknown Client'}</p>
                        <p className="text-xs text-gray-500 mt-0.5">
                          {s.session_time || 'Time TBD'} · {s.dish_count || 0} dishes · {s.serving_size} servings
                        </p>
                      </div>
                      <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${STATUS_COLORS[s.status] || 'bg-gray-100'}`}>
                        {s.status}
                      </span>
                    </div>
                    {s.service_fee > 0 && (
                      <p className="text-xs text-gray-400 mt-1">Fee: ${s.service_fee.toFixed(0)}</p>
                    )}
                  </div>
                ))}
                <button
                  onClick={() => navigate(`/sessions/new?date=${format(selectedDate, 'yyyy-MM-dd')}`)}
                  className="w-full py-2.5 border-2 border-dashed border-gray-200 text-gray-400 rounded-xl text-sm hover:border-navy hover:text-navy transition-colors"
                  style={{ minHeight: '44px' }}
                >
                  + Add session on this day
                </button>
              </div>
            )}
          </div>
        )}

        {/* Month summary */}
        {!selectedDate && !loading && (
          <div className="mt-4 card">
            <p className="text-xs font-semibold text-gray-500 mb-2 uppercase tracking-wide">
              {format(currentMonth, 'MMMM')} Summary
            </p>
            {(() => {
              const monthKey = format(currentMonth, 'yyyy-MM');
              const monthSessions = sessions.filter(s => s.session_date?.startsWith(monthKey));
              const upcoming = monthSessions.filter(s => s.session_date! >= format(today, 'yyyy-MM-dd') && s.status !== 'Cancelled');
              const revenue  = monthSessions.filter(s => s.status === 'Completed').reduce((n, s) => n + (s.service_fee || 0), 0);
              return (
                <div className="grid grid-cols-3 gap-3 text-center">
                  <div>
                    <p className="text-lg font-bold font-display text-navy">{monthSessions.length}</p>
                    <p className="text-xs text-gray-500">Total sessions</p>
                  </div>
                  <div>
                    <p className="text-lg font-bold font-display text-navy">{upcoming.length}</p>
                    <p className="text-xs text-gray-500">Upcoming</p>
                  </div>
                  <div>
                    <p className="text-lg font-bold font-display text-navy">${revenue.toFixed(0)}</p>
                    <p className="text-xs text-gray-500">Earned</p>
                  </div>
                </div>
              );
            })()}
          </div>
        )}
      </div>
    </div>
  );
}
