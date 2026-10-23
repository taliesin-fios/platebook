// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, type MouseEvent } from 'react';
import { useNavigate } from 'react-router';
import { fmtSessionDate, fmtUtcTimestamp } from '../utils/dateUtils';
import PageHeader from '../components/PageHeader';
import { SkeletonList } from '../components/Skeleton';
import { useToast } from '../components/Toast';
import { STATUS_COLORS, marginColor, repeatLabel } from '../constants';
import { errorMessage } from '../hooks/useApi';
import type { EmailSentResponse, SessionListItem } from '../types';

const InvoiceIcon = () => (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/>
    <polyline points="14 2 14 8 20 8"/>
    <line x1="16" y1="13" x2="8" y2="13"/>
    <line x1="16" y1="17" x2="8" y2="17"/>
  </svg>
);

const SendIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2">
    <line x1="22" y1="2" x2="11" y2="13"/>
    <polygon points="22 2 15 22 11 13 2 9 22 2"/>
  </svg>
);

const STATUSES = ['All', 'Pending', 'Confirmed', 'Completed', 'Cancelled'];

export default function Sessions() {
  const navigate = useNavigate();
  const { show } = useToast();
  const [sessions, setSessions] = useState<SessionListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [sendingId, setSendingId] = useState<number | null>(null);
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState('All');

  useEffect(() => {
    fetch('/api/sessions')
      .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
      .then(d => setSessions(Array.isArray(d) ? d : []))
      .catch(() => setSessions([]))
      .finally(() => setLoading(false));
  }, []);

  const formatDate = (d: string | null) => fmtSessionDate(d, 'EEEE, MMMM d');

  const formatSentDate = (ts: string | null) => fmtUtcTimestamp(ts, 'MMM d');

  // Re-send meal plan PDF from the list without navigating into detail
  const handleResend = async (s: SessionListItem, e: MouseEvent) => {
    e.stopPropagation();
    setSendingId(s.id);
    try {
      // The server renders and attaches the meal plan itself.
      const res = await fetch(`/api/sessions/${s.id}/email-pdf`, { method: 'POST' });
      const data: EmailSentResponse & { error?: string; code?: string } = await res.json();
      if (!res.ok) {
        // The server records a failed send (not a missing address)
        if (data.code !== 'NO_EMAIL') {
          setSessions(prev => prev.map(x =>
            x.id === s.id ? { ...x, pdf_email_failed_at: new Date().toISOString() } : x
          ));
        }
        throw new Error(data.error || 'Failed to send');
      }

      // Update badge in local list state immediately
      setSessions(prev => prev.map(x =>
        x.id === s.id ? { ...x, pdf_emailed_at: new Date().toISOString(), pdf_email_failed_at: null } : x
      ));
      show(`Meal plan sent to ${data.sent_to}`, 'success', 4000);
    } catch (err) {
      if (errorMessage(err)?.includes('NO_EMAIL') || errorMessage(err)?.includes('no email')) {
        show('No email on file for this client — add one in the client profile', 'error', 4000);
      } else {
        show(errorMessage(err) || 'Could not send meal plan', 'error', 4000);
      }
    } finally {
      setSendingId(null);
    }
  };

  const today = new Date().toISOString().split('T')[0];

  const filtered = sessions.filter(s => {
    const matchSearch = !search.trim() ||
      s.client_name?.toLowerCase().includes(search.trim().toLowerCase());
    const matchStatus = statusFilter === 'All' || s.status === statusFilter;
    return matchSearch && matchStatus;
  });

  const upcoming = filtered.filter(s =>
    s.session_date! >= today &&
    s.status !== 'Cancelled' && s.status !== 'Completed'
  ).sort((a, b) => a.session_date!.localeCompare(b.session_date!));

  const past = filtered.filter(s =>
    s.session_date! < today ||
    s.status === 'Completed' || s.status === 'Cancelled'
  ).sort((a, b) => b.session_date!.localeCompare(a.session_date!));

  const SessionCard = ({ s }: { s: SessionListItem }) => {
    const isSending = sendingId === s.id;
    const canResend  = s.status === 'Completed' && s.client_email;

    return (
      <div
        className="card mb-3 cursor-pointer hover:shadow-card-hover transition-shadow"
        onClick={() => navigate(`/sessions/${s.id}`)}
      >
        <div className="flex items-start justify-between mb-2 gap-2">
          <div className="flex-1 min-w-0">
            <div className="font-display font-semibold text-navy truncate">{s.client_name || 'Unknown Client'}</div>
            <div className="text-xs text-gray-500 mt-0.5">
              {formatDate(s.session_date)}
              {s.session_time ? ` · ${s.session_time}` : ''}
            </div>
          </div>
          <div className="flex items-center gap-1.5 shrink-0">
            <span className={`text-xs px-2.5 py-1 rounded-full font-medium ${STATUS_COLORS[s.status] || 'bg-gray-100'}`}>
              {s.status}
            </span>
            {/* Re-send PDF — only on completed sessions with a client email */}
            {canResend && (
              <button
                onClick={e => handleResend(s, e)}
                disabled={isSending}
                className="p-1.5 rounded-lg text-gray-400 hover:text-green-dark hover:bg-green-light transition-colors disabled:opacity-40"
                title={s.pdf_emailed_at ? `Re-send meal plan (last sent ${formatSentDate(s.pdf_emailed_at)})` : 'Send meal plan PDF to client'}
                style={{ minHeight: 'auto' }}
              >
                {isSending
                  ? <span className="block w-3.5 h-3.5 border-2 border-green-dark border-t-transparent rounded-full animate-spin" />
                  : <SendIcon />
                }
              </button>
            )}
            {/* Invoice quick-action */}
            <button
              onClick={e => { e.stopPropagation(); navigate(`/invoices/new?session_id=${s.id}`); }}
              className="p-1.5 rounded-lg text-gray-400 hover:text-navy hover:bg-blue-light transition-colors"
              title="Generate invoice for this session"
              style={{ minHeight: 'auto' }}
            >
              <InvoiceIcon />
            </button>
          </div>
        </div>

        <div className="flex items-center gap-3 text-xs text-gray-400">
          <span>{s.dish_count || 0} dishes</span>
          {s.repeat_every_weeks && <span title={repeatLabel(s.repeat_every_weeks)}>🔁 {s.repeat_every_weeks === 1 ? 'Weekly' : `${s.repeat_every_weeks}w`}</span>}
          {s.service_fee > 0 && <span>${(s.service_fee || 0).toFixed(0)}</span>}
          {s.margin_pct > 0 && (
            <span className={marginColor(s.margin_pct)}>
              {(s.margin_pct || 0).toFixed(0)}% margin
            </span>
          )}
          {/* Email failed / sent badge */}
          {s.pdf_email_failed_at ? (
            <span className="text-red-600 font-medium ml-auto" title="The last meal-plan email failed. Open the session to try again.">
              ⚠ Email failed {formatSentDate(s.pdf_email_failed_at)}
            </span>
          ) : s.pdf_emailed_at && (
            <span className="flex items-center gap-1 text-green-dark font-medium ml-auto">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/>
                <polyline points="22,6 12,13 2,6"/>
              </svg>
              Sent {formatSentDate(s.pdf_emailed_at)}
            </span>
          )}
        </div>
        {s.chef_notes && (
          <div className="mt-2 text-xs text-gray-500 truncate border-t border-gray-100 pt-2">
            📝 {s.chef_notes}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="page">
      <PageHeader
        title="Sessions"
        action={
          <button
            onClick={() => navigate('/calendar')}
            className="text-white opacity-80 hover:opacity-100 text-sm font-medium flex items-center gap-1"
            style={{ minHeight: 'auto' }}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <rect x="3" y="4" width="18" height="18" rx="2"/><line x1="16" y1="2" x2="16" y2="6"/>
              <line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/>
            </svg>
            Calendar
          </button>
        }
      />

      {/* Search + Status Filter */}
      <div className="px-4 pt-4 pb-2 space-y-2">
        {/* Search */}
        <div className="relative">
          <svg className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
          </svg>
          <input
            type="search"
            placeholder="Search clients…"
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="pl-9 pr-3 py-2.5 w-full rounded-xl border border-gray-200 bg-white text-sm focus:outline-hidden focus:ring-2 focus:ring-green-mid"
            style={{ minHeight: '44px' }}
          />
          {search && (
            <button
              onClick={() => setSearch('')}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
              style={{ minHeight: 'auto' }}
            >
              ✕
            </button>
          )}
        </div>

        {/* Status pills */}
        <div className="flex gap-1.5 overflow-x-auto pb-1 -mb-1 scrollbar-hide">
          {STATUSES.map(st => (
            <button
              key={st}
              onClick={() => setStatusFilter(st)}
              className={`shrink-0 px-3 py-1.5 rounded-full text-xs font-semibold transition-colors ${
                statusFilter === st
                  ? 'bg-navy text-white'
                  : 'bg-gray-100 text-gray-500 hover:bg-gray-200'
              }`}
              style={{ minHeight: 'auto' }}
            >
              {st}
            </button>
          ))}
        </div>
      </div>

      <div className="px-4 pt-2">
        {loading ? <SkeletonList count={4} /> : (
          <>
            {upcoming.length > 0 && (
              <div className="mb-4">
                <div className="section-header">Upcoming ({upcoming.length})</div>
                {upcoming.map(s => <SessionCard key={s.id} s={s} />)}
              </div>
            )}

            {past.length > 0 && (
              <div>
                <div className="section-header">
                  {upcoming.length > 0 ? `Past (${past.length})` : `All Sessions (${past.length})`}
                </div>
                {past.map(s => <SessionCard key={s.id} s={s} />)}
              </div>
            )}

            {/* Truly empty — no sessions at all */}
            {sessions.length === 0 && (
              <div className="card text-center py-10">
                <div className="text-4xl mb-3">📅</div>
                <div className="font-semibold text-gray-700 mb-1">No sessions yet</div>
                <div className="text-sm text-gray-400 mb-4">Schedule your first meal prep session</div>
                <button
                  onClick={() => navigate('/sessions/new')}
                  className="px-5 py-2.5 bg-navy text-white rounded-xl text-sm font-semibold"
                >
                  New Session
                </button>
              </div>
            )}

            {/* Sessions exist but filter has no matches */}
            {sessions.length > 0 && filtered.length === 0 && (
              <div className="card text-center py-8 text-gray-400">
                <div className="text-3xl mb-2">🔍</div>
                <div className="text-sm font-medium text-gray-500 mb-1">No sessions match</div>
                <div className="text-xs mb-3">
                  {search && statusFilter !== 'All'
                    ? `No "${statusFilter}" sessions for "${search}"`
                    : search
                    ? `No sessions matching "${search}"`
                    : `No "${statusFilter}" sessions`}
                </div>
                <button
                  onClick={() => { setSearch(''); setStatusFilter('All'); }}
                  className="px-4 py-1.5 border border-gray-200 rounded-xl text-xs font-medium text-gray-600 hover:bg-gray-50"
                  style={{ minHeight: 'auto' }}
                >
                  Clear filters
                </button>
              </div>
            )}
          </>
        )}
      </div>

      <button className="fab" onClick={() => navigate('/sessions/new')}>+</button>
    </div>
  );
}
