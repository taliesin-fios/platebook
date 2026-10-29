// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useEffect, useState, useCallback, useContext } from 'react';
import { useNavigate } from 'react-router';
import { fmtSessionDate, localDateStrOffset, localTodayStr } from '../utils/dateUtils';
import PageHeader from '../components/PageHeader';
import { SkeletonStatGrid } from '../components/Skeleton';
import { STATUS_COLORS, FEASIBILITY_COLORS, calcFeasibility, APP_NAME, bookingUrl } from '../constants';
import { SettingsContext } from '../App';
import { parseNum } from '../utils/numbers';
import { mtdTrend } from '../utils/trend';
import { clientsTile, invoiceSummary, marginTile } from '../utils/dashboardStats';
import type { ClientListItem, CostSummary, SessionListItem } from '../types';

function IntakeLinkCard() {
  const [copied, setCopied] = useState(false);
  const { slug } = useContext(SettingsContext);
  const intakeUrl = bookingUrl(slug);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(intakeUrl).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }).catch(() => {
      // Fallback for browsers without clipboard API
      const el = document.createElement('textarea');
      el.value = intakeUrl;
      document.body.appendChild(el);
      el.select();
      document.execCommand('copy');
      document.body.removeChild(el);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [intakeUrl]);

  return (
    <div className="px-4 mb-4">
      <div className="card border border-green-light">
        <div className="flex items-start justify-between mb-2">
          <div>
            <div className="font-semibold text-green-dark text-sm">📋 Your Booking Link</div>
            <div className="text-xs text-gray-500 mt-0.5">Share this link with prospective clients</div>
          </div>
          <span className="text-lg">🔗</span>
        </div>
        <div className="flex gap-2 mt-3">
          <div className="flex-1 min-w-0 bg-gray-50 border border-gray-200 rounded-lg px-3 py-2">
            <div className="text-xs text-gray-500 truncate">{intakeUrl}</div>
          </div>
          <button
            onClick={handleCopy}
            className={`px-4 py-2 rounded-lg text-sm font-semibold transition-colors shrink-0 ${
              copied ? 'bg-green-mid text-white' : 'bg-navy text-white'
            }`}
            style={{ minHeight: '36px' }}
          >
            {copied ? '✓ Copied!' : 'Copy'}
          </button>
        </div>
      </div>
    </div>
  );
}

export default function Dashboard() {
  const navigate = useNavigate();
  const appSettings = useContext(SettingsContext);
  const [summary, setSummary] = useState<CostSummary | null>(null);
  const [upcoming, setUpcoming] = useState<SessionListItem[]>([]);
  const [recent, setRecent] = useState<SessionListItem[]>([]);
  const [clients, setClients] = useState<ClientListItem[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const safeJson = (r: Response) => r.ok ? r.text().then(t => t ? JSON.parse(t) : {}) : Promise.resolve(null);
    Promise.all([
      fetch('/api/costs/summary').then(safeJson),
      fetch(`/api/sessions?upcoming=true&today=${localTodayStr()}`).then(safeJson),
      fetch('/api/sessions').then(safeJson),
      fetch('/api/clients').then(safeJson),
    ]).then(([s, up, all, cl]) => {
      setSummary(s as CostSummary | null);
      setUpcoming(Array.isArray(up) ? up : []);
      setRecent(Array.isArray(all) ? all.slice(0, 5) : []);
      setClients(Array.isArray(cl) ? cl : []);
    }).catch(console.error).finally(() => setLoading(false));
  }, []);

  const nextSession = upcoming[0];
  // "This week" = today plus the next six days.
  const weekEnd = localDateStrOffset(6);
  const thisWeekCount = upcoming.filter(s => s.session_date && s.session_date.slice(0, 10) <= weekEnd).length;

  const formatDate = (d: string | null) => fmtSessionDate(d, 'EEEE, MMMM d');

  const fmt$ = (v: unknown) => `$${(parseNum(v) || 0).toLocaleString('en-US', { minimumFractionDigits: 0, maximumFractionDigits: 0 })}`;

  const collectedTrend = summary ? mtdTrend(parseNum(summary.collected_mtd) || 0, parseNum(summary.collected_prev_mtd) || 0) : null;
  const TREND_COLORS = { up: 'text-green-dark', down: 'text-danger', flat: 'text-gray-400' } as const;

  const statCards = summary ? [
    {
      label: 'Sessions this week',
      value: thisWeekCount,
      sub: nextSession ? `Next: ${formatDate(nextSession.session_date)}` : 'No upcoming sessions',
    },
    {
      label: 'Active Clients',
      ...clientsTile(clients),
    },
    {
      label: 'Collected (MTD)',
      value: fmt$(summary.collected_mtd ?? summary.total_revenue),
      trend: collectedTrend,
      trendTitle: `Compared with ${fmt$(summary.collected_prev_mtd)} collected over the same days last month`,
      sub: invoiceSummary(parseNum(summary.invoiced_total) || 0, parseNum(summary.outstanding_total) || 0, fmt$),
      valueClass: 'text-green-dark',
      subClass: summary.overdue_count > 0 ? 'text-danger font-medium' : '',
      subPrefix: summary.overdue_count > 0 ? `⚠ ${summary.overdue_count} overdue · ` : '',
    },
    {
      label: 'Avg Margin',
      ...marginTile(summary.session_count, parseNum(summary.total_revenue) || 0, parseNum(summary.margin_pct) || 0),
    },
  ] : null;

  const quickActions = [
    { label: 'New Session',  icon: '+',  path: '/sessions/new',    color: 'bg-navy text-white' },
    { label: 'New Client',   icon: '👤', path: '/clients/new',     color: 'bg-navy-mid text-white' },
    { label: 'Calendar',     icon: '📅', path: '/calendar',        color: 'bg-green-dark text-white' },
    { label: 'Menu Planner', icon: '📋', path: '/planner',         color: 'bg-olive text-white' },
    { label: 'Import Recipe',icon: '↓',  path: '/recipes/import',  color: 'bg-amber text-olive' },
    { label: 'Templates',    icon: '▤',  path: '/templates',       color: 'bg-gray-600 text-white' },
  ];

  return (
    <div className="page pb-24">
      <div className="page-header">
        <div className="flex-1">
          <div className="font-display text-xl font-semibold">{appSettings.business_name || APP_NAME}</div>
          {appSettings.location && <div className="text-xs text-blue-200 mt-0.5">{appSettings.location}</div>}
        </div>
        <button
          onClick={() => navigate('/settings')}
          className="text-white opacity-80"
          style={{ minHeight: 'auto' }}
        >
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z" />
          </svg>
        </button>
      </div>

      {/* First-run onboarding banner — shown only after settings load and business_name is still blank */}
      {!loading && 'business_name' in appSettings && !appSettings.business_name && (
        <div className="mx-4 mt-4 mb-1 p-4 bg-amber-50 border border-amber-200 rounded-2xl">
          <div className="flex items-start gap-3">
            <span className="text-2xl">👋</span>
            <div className="flex-1">
              <div className="font-semibold text-amber-900 text-sm mb-1">Welcome! Set up your business profile</div>
              <p className="text-xs text-amber-700 mb-3 leading-relaxed">
                Add your business name, location, and contact info so they appear on emails, invoices, and PDFs.
              </p>
              <button
                onClick={() => navigate('/settings')}
                className="px-4 py-2 bg-amber-600 text-white rounded-xl text-xs font-semibold"
                style={{ minHeight: '36px' }}
              >
                Go to Settings →
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Stat Cards */}
      {loading || !statCards ? <SkeletonStatGrid /> : (
        <div className="grid grid-cols-2 gap-3 px-4 pt-4 mb-4">
          {statCards.map((card, i) => (
            <div key={i} className="card">
              <div className="text-xs text-gray-500 font-medium mb-1">{card.label}</div>
              <div className={`text-2xl font-bold font-display ${card.valueClass || 'text-navy'}`}>{card.value}</div>
              {'trend' in card && card.trend && (
                <div className={`text-xs font-medium ${TREND_COLORS[card.trend.direction]}`} title={card.trendTitle}>
                  {card.trend.text}
                </div>
              )}
              <div className={`text-xs mt-1 ${card.subClass || 'text-gray-400'}`}>
                {card.subPrefix || ''}{card.sub}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Overdue invoice alert */}
      {!loading && summary && summary.overdue_count > 0 && (
        <div className="px-4 mb-4">
          <button
            onClick={() => navigate('/invoices')}
            className="w-full flex items-center gap-3 px-4 py-3 bg-red-50 border border-red-200 rounded-2xl text-left hover:bg-red-100 transition-colors"
            style={{ minHeight: 'auto' }}
          >
            <span className="text-xl">⚠️</span>
            <div className="flex-1 min-w-0">
              <div className="text-sm font-semibold text-danger">
                {summary.overdue_count} overdue invoice{summary.overdue_count !== 1 ? 's' : ''}
              </div>
              <div className="text-xs text-red-400">
                {fmt$(summary.outstanding_total)} outstanding — tap to view
              </div>
            </div>
            <span className="text-danger opacity-60">›</span>
          </button>
        </div>
      )}

      {/* Quick Actions — wraps to next row so nothing is ever clipped */}
      <div className="px-4 mb-4">
        <div className="flex flex-wrap gap-2">
          {quickActions.map(a => (
            <button
              key={a.path}
              onClick={() => navigate(a.path)}
              className={`px-4 py-2.5 rounded-xl text-sm font-semibold flex items-center gap-2 ${a.color}`}
              style={{ minHeight: '44px' }}
            >
              <span>{a.icon}</span>
              <span className="whitespace-nowrap">{a.label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Upcoming Sessions */}
      {upcoming.length > 0 && (
        <div className="px-4 mb-4">
          <div className="section-header">
            {upcoming.length === 1 ? 'Next Session' : `Upcoming (${upcoming.length})`}
          </div>

          {/* First (next) session — full detail */}
          <div
            className="card cursor-pointer hover:shadow-card-hover transition-shadow mb-2"
            onClick={() => navigate(`/sessions/${nextSession.id}`)}
          >
            <div className="flex items-start justify-between mb-2">
              <div>
                <div className="font-display text-lg font-semibold text-navy">{nextSession.client_name}</div>
                <div className="text-sm text-gray-600">{formatDate(nextSession.session_date)}{nextSession.session_time ? ` · ${nextSession.session_time}` : ''}</div>
              </div>
              <span className={`px-2 py-1 rounded-full text-xs font-medium ${STATUS_COLORS[nextSession.status] || 'bg-gray-100'}`}>
                {nextSession.status}
              </span>
            </div>
            <div className="flex items-center gap-3 text-sm text-gray-500">
              <span>{nextSession.dish_count || 0} dishes</span>
              {nextSession.service_fee > 0 && (
                <span>${nextSession.service_fee.toFixed(0)}</span>
              )}
            </div>
            {/* Allergen warning */}
            {(() => {
              const allergens = (() => {
                try { return JSON.parse(nextSession.client_allergens || '[]') as string[]; } catch { return []; }
              })();
              return allergens.length > 0 ? (
                <div className="mt-2 flex items-center gap-1.5 flex-wrap border-t border-gray-100 pt-2">
                  <span className="text-xs text-danger font-semibold">⚠ Allergens:</span>
                  {allergens.map(a => (
                    <span key={a} className="text-xs px-2 py-0.5 bg-red-50 text-danger rounded-full font-medium">{a}</span>
                  ))}
                </div>
              ) : null;
            })()}
            {/* Chef notes preview */}
            {nextSession.chef_notes && (
              <div className="mt-2 text-xs text-gray-500 truncate border-t border-gray-100 pt-2">
                📝 {nextSession.chef_notes}
              </div>
            )}
          </div>

          {/* Additional upcoming sessions — compact */}
          {upcoming.slice(1).map(s => (
            <div
              key={s.id}
              className="card mb-2 cursor-pointer hover:shadow-card-hover transition-shadow"
              onClick={() => navigate(`/sessions/${s.id}`)}
            >
              <div className="flex items-center justify-between">
                <div className="min-w-0 flex-1">
                  <div className="font-semibold text-navy text-sm truncate">{s.client_name}</div>
                  <div className="text-xs text-gray-500 mt-0.5">
                    {formatDate(s.session_date)}{s.session_time ? ` · ${s.session_time}` : ''}
                  </div>
                </div>
                <div className="flex items-center gap-2 ml-2 shrink-0">
                  <span className="text-xs text-gray-400">{s.dish_count || 0} dishes</span>
                  <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${STATUS_COLORS[s.status] || 'bg-gray-100'}`}>
                    {s.status}
                  </span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Intake Form Share Card */}
      <IntakeLinkCard />

      {/* Recent Activity */}
      <div className="px-4">
        <div className="section-header">Recent Sessions</div>
        {recent.length === 0 ? (
          <div className="card text-center py-8">
            <div className="text-3xl mb-2">🍳</div>
            <div className="text-gray-500 text-sm">No sessions yet</div>
            <button
              onClick={() => navigate('/sessions/new')}
              className="mt-3 px-4 py-2 bg-navy text-white rounded-xl text-sm font-medium"
            >
              Create your first session
            </button>
          </div>
        ) : recent.map(s => (
          <div
            key={s.id}
            className="card mb-3 cursor-pointer hover:shadow-card-hover transition-shadow"
            onClick={() => navigate(`/sessions/${s.id}`)}
          >
            <div className="flex items-center justify-between">
              <div>
                <div className="font-semibold text-navy">{s.client_name}</div>
                <div className="text-xs text-gray-500 mt-0.5">
                  {fmtSessionDate(s.session_date, 'MMM d, yyyy')}
                </div>
              </div>
              <div className="flex flex-col items-end gap-1">
                <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${STATUS_COLORS[s.status] || ''}`}>
                  {s.status}
                </span>
                {s.service_fee > 0 && (
                  <span className="text-xs text-gray-500">${s.service_fee.toFixed(0)}</span>
                )}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
