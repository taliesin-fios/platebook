// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from 'recharts';
import { format } from 'date-fns';
import { localTodayStr, parseLocalDate } from '../utils/dateUtils';
import PageHeader from '../components/PageHeader';
import { SkeletonStatGrid } from '../components/Skeleton';
import { marginColor } from '../constants';
import { parseNum, type NumInput } from '../utils/numbers';
import type { CostByClient, CostSession, CostSummary, Id } from '../types';

type ChartPoint = { date: string | null; revenue: number; cost: number };
type CostEdit = { service_fee?: NumInput; ingredient_cost?: NumInput };


export default function Costs() {
  const navigate = useNavigate();
  // An empty object until (or if the request fails) — every field reads as undefined
  const [summary, setSummary] = useState<CostSummary | null>(null);
  const [sessions, setSessions] = useState<CostSession[]>([]);
  const [byClient, setByClient] = useState<CostByClient[]>([]);
  const [loading, setLoading] = useState(true);
  const [editingRow, setEditingRow] = useState<Id | null>(null);
  const [editValues, setEditValues] = useState<CostEdit>({});

  useEffect(() => {
    const safeJson = (r: Response) => r.ok ? r.json() : Promise.resolve(null);
    Promise.all([
      fetch('/api/costs/summary').then(safeJson),
      fetch('/api/costs/sessions?days=30').then(safeJson),
      fetch('/api/costs/by-client').then(safeJson),
    ]).then(([s, ss, bc]) => {
      setSummary((s || {}) as CostSummary);
      setSessions(Array.isArray(ss) ? ss : []);
      setByClient(Array.isArray(bc) ? bc : []);
    }).catch(() => {}).finally(() => setLoading(false));
  }, []);

  const chartData = sessions.reduce<ChartPoint[]>((acc, s) => {
    const date = s.session_date;
    const existing = acc.find(d => d.date === date);
    if (existing) {
      existing.revenue += s.service_fee || 0;
      existing.cost += s.ingredient_cost || 0;
    } else {
      acc.push({ date, revenue: s.service_fee || 0, cost: s.ingredient_cost || 0 });
    }
    return acc;
  }, []).sort((a, b) => a.date!.localeCompare(b.date!)).map(d => ({
    ...d,
    label: (() => { try { return format(parseLocalDate(d.date), 'M/d'); } catch { return d.date; } })(),
  }));

  const handleEditSave = async (sessionId: Id) => {
    try {
      await fetch(`/api/sessions/${sessionId}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          service_fee: parseNum(editValues.service_fee) || 0,
          ingredient_cost: parseNum(editValues.ingredient_cost) || 0,
        }),
      });
      setSessions(prev => prev.map(s => s.id === sessionId ? {
        ...s,
        service_fee: parseNum(editValues.service_fee) || 0,
        ingredient_cost: parseNum(editValues.ingredient_cost) || 0,
        gross_margin: (parseNum(editValues.service_fee) || 0) - (parseNum(editValues.ingredient_cost) || 0),
        // Kept numeric (as the server sends it): the row renders margin_pct.toFixed(),
        // which threw on the string toFixed() returns.
        margin_pct: Number(editValues.service_fee) > 0
          ? parseFloat((((parseNum(editValues.service_fee) - parseNum(editValues.ingredient_cost)) / parseNum(editValues.service_fee)) * 100).toFixed(1))
          : 0,
      } : s));
      setEditingRow(null);
    } catch { /* network error: leave the row in edit mode */ }
  };

  const marginClass = (pct: number) => `${marginColor(pct)} font-semibold`;

  const exportCSV = () => {
    window.open('/api/costs/export-csv', '_blank');
  };

  return (
    <div className="page">
      <PageHeader title="Cost Tracker" />

      <div className="px-4 pt-4">
        {/* Summary cards */}
        {loading ? <SkeletonStatGrid /> : summary && (
          <>
            {/* Session cost metrics */}
            <div className="grid grid-cols-2 gap-3 mb-3">
              <div className="card">
                <div className="text-xs text-gray-500 mb-1">Billed (MTD)</div>
                <div className="text-2xl font-bold font-display text-navy">${(summary.total_revenue || 0).toLocaleString()}</div>
                <div className="text-xs text-gray-400">{summary.session_count} sessions</div>
              </div>
              <div className="card">
                <div className="text-xs text-gray-500 mb-1">Ingredient Cost</div>
                <div className="text-2xl font-bold font-display text-navy">${(summary.total_cost || 0).toLocaleString()}</div>
              </div>
              <div className="card">
                <div className="text-xs text-gray-500 mb-1">Gross Margin</div>
                <div className="text-2xl font-bold font-display text-navy">${(summary.gross_margin || 0).toLocaleString()}</div>
              </div>
              <div className="card">
                <div className="text-xs text-gray-500 mb-1">Margin %</div>
                <div className={`text-2xl font-bold font-display ${marginClass(summary.margin_pct || 0)}`}>{(summary.margin_pct || 0).toFixed(1)}%</div>
              </div>
            </div>

            {/* YTD summary */}
            {summary.ytd_sessions > 0 && (
              <div className="card mb-3 border border-green-light">
                <div className="flex items-center justify-between mb-2">
                  <div className="section-header mb-0">Year-to-Date</div>
                  <span className="text-xs text-gray-400">{new Date().getFullYear()}</span>
                </div>
                <div className="grid grid-cols-4 gap-2 text-center">
                  <div>
                    <div className="text-base font-bold font-display text-navy">{summary.ytd_sessions}</div>
                    <div className="text-[11px] text-gray-500">Sessions</div>
                  </div>
                  <div>
                    <div className="text-base font-bold font-display text-green-dark">${(summary.ytd_revenue || 0).toLocaleString()}</div>
                    <div className="text-[11px] text-gray-500">Revenue</div>
                  </div>
                  <div>
                    <div className="text-base font-bold font-display text-navy">${(summary.ytd_margin || 0).toLocaleString()}</div>
                    <div className="text-[11px] text-gray-500">Margin $</div>
                  </div>
                  <div>
                    <div className={`text-base font-bold font-display ${marginClass(summary.ytd_margin_pct || 0)}`}>
                      {(summary.ytd_margin_pct || 0).toFixed(0)}%
                    </div>
                    <div className="text-[11px] text-gray-500">Margin %</div>
                  </div>
                </div>
              </div>
            )}

            {/* AR snapshot */}
            <div className="card mb-4 border border-blue-100">
              <div className="section-header mb-3">Accounts Receivable</div>
              <div className="grid grid-cols-3 gap-3">
                <div className="text-center">
                  <div className="text-xs text-gray-500 mb-1">Collected (MTD)</div>
                  <div className="text-lg font-bold text-green-dark">${(summary.collected_mtd || 0).toLocaleString()}</div>
                </div>
                <div className="text-center">
                  <div className="text-xs text-gray-500 mb-1">Outstanding</div>
                  <div className="text-lg font-bold text-amber-600">${(summary.outstanding_total || 0).toLocaleString()}</div>
                </div>
                <div className="text-center">
                  <div className="text-xs text-gray-500 mb-1">Overdue</div>
                  <div className={`text-lg font-bold ${summary.overdue_count > 0 ? 'text-danger' : 'text-gray-400'}`}>
                    {summary.overdue_count || 0}
                  </div>
                  {summary.overdue_count > 0 && (
                    <button
                      onClick={() => navigate('/invoices?status=sent')}
                      className="text-xs text-danger underline mt-0.5"
                      style={{ minHeight: 'auto' }}
                    >
                      View
                    </button>
                  )}
                </div>
              </div>
            </div>
          </>
        )}

        {/* Chart */}
        {chartData.length > 0 && (
          <div className="card mb-4">
            <div className="section-header">Rolling 30 Days</div>
            <ResponsiveContainer width="100%" height={200}>
              <BarChart data={chartData} margin={{ top: 0, right: 0, left: -20, bottom: 0 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="#f0f0f0" />
                <XAxis dataKey="label" tick={{ fontSize: 10 }} />
                <YAxis tick={{ fontSize: 10 }} />
                <Tooltip
                  formatter={(val, name) => [`$${(val as number).toFixed(0)}`, name === 'revenue' ? 'Revenue' : 'Cost']}
                  contentStyle={{ fontSize: 12 }}
                />
                <Bar dataKey="revenue" fill="#0D2B4B" radius={[4, 4, 0, 0]} />
                <Bar dataKey="cost" fill="#27AE60" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
            <div className="flex justify-center gap-4 mt-2">
              <div className="flex items-center gap-1 text-xs text-gray-500"><div className="w-3 h-3 rounded bg-navy" /> Revenue</div>
              <div className="flex items-center gap-1 text-xs text-gray-500"><div className="w-3 h-3 rounded bg-green-mid" /> Cost</div>
            </div>
          </div>
        )}

        {/* Session table */}
        <div className="card mb-4">
          <div className="flex items-center justify-between mb-3">
            <div className="section-header mb-0">Sessions</div>
            <button onClick={exportCSV} className="text-xs text-navy-mid font-medium border border-navy-mid px-2 py-1 rounded-lg" style={{ minHeight: 'auto' }}>Export CSV</button>
          </div>
          {sessions.length === 0 ? (
            <div className="text-sm text-gray-400 text-center py-4">No session data yet</div>
          ) : (
            <div className="overflow-x-auto -mx-2 px-2">
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b border-gray-200">
                    <th className="text-left py-2 text-gray-500 font-medium">Date</th>
                    <th className="text-left py-2 text-gray-500 font-medium">Client</th>
                    <th className="text-right py-2 text-gray-500 font-medium">Fee</th>
                    <th className="text-right py-2 text-gray-500 font-medium">Cost</th>
                    <th className="text-right py-2 text-gray-500 font-medium">Margin</th>
                    <th className="text-center py-2 text-gray-500 font-medium">Inv.</th>
                  </tr>
                </thead>
                <tbody>
                  {sessions.map(s => {
                    const INV_PILL: Record<string, { label: string; cls: string }> = {
                      draft: { label: 'Draft', cls: 'bg-gray-100 text-gray-500' },
                      sent:  { label: 'Sent',  cls: 'bg-blue-100 text-blue-700' },
                      paid:  { label: 'Paid',  cls: 'bg-green-100 text-green-700' },
                    };
                    const today = localTodayStr();
                    const overdue = s.invoice_status && s.invoice_status !== 'paid' && s.invoice_due_date && s.invoice_due_date < today;
                    const pill = s.invoice_status ? (INV_PILL[s.invoice_status] || INV_PILL.draft) : null;

                    return (
                    <tr key={s.id} className="border-b border-gray-100 last:border-0 cursor-pointer hover:bg-warm-white"
                      onClick={() => navigate(`/sessions/${s.id}`)}>
                      <td className="py-2.5 pr-2">
                        {(() => { try { return format(parseLocalDate(s.session_date), 'M/d/yy'); } catch { return s.session_date; } })()}
                      </td>
                      <td className="py-2.5 pr-2 max-w-[80px] truncate">{s.client_name}</td>
                      <td className="py-2.5 text-right">
                        {editingRow === s.id ? (
                          <input
                            type="number"
                            value={editValues.service_fee}
                            onChange={e => setEditValues(prev => ({ ...prev, service_fee: e.target.value }))}
                            onClick={e => e.stopPropagation()}
                            style={{ width: '60px', fontSize: '11px', padding: '2px 4px', minHeight: 'auto' }}
                          />
                        ) : `$${(parseNum(s.service_fee) || 0).toFixed(0)}`}
                      </td>
                      <td className="py-2.5 text-right">
                        {editingRow === s.id ? (
                          <input
                            type="number"
                            value={editValues.ingredient_cost}
                            onChange={e => setEditValues(prev => ({ ...prev, ingredient_cost: e.target.value }))}
                            onClick={e => e.stopPropagation()}
                            style={{ width: '60px', fontSize: '11px', padding: '2px 4px', minHeight: 'auto' }}
                          />
                        ) : `$${(parseNum(s.ingredient_cost) || 0).toFixed(0)}`}
                      </td>
                      <td className={`py-2.5 text-right ${marginClass(s.margin_pct)}`}>
                        {editingRow === s.id ? (
                          <button
                            onClick={e => { e.stopPropagation(); handleEditSave(s.id); }}
                            className="text-green-dark font-bold"
                            style={{ minHeight: 'auto' }}
                          >Save</button>
                        ) : (
                          <span
                            onClick={e => { e.stopPropagation(); setEditingRow(s.id); setEditValues({ service_fee: s.service_fee, ingredient_cost: s.ingredient_cost }); }}
                          >
                            {(s.margin_pct || 0).toFixed(0)}%
                          </span>
                        )}
                      </td>
                      <td className="py-2.5 text-center">
                        {pill ? (
                          <button
                            onClick={e => { e.stopPropagation(); navigate(`/invoices/${s.invoice_id}`); }}
                            className={`text-[10px] px-1.5 py-0.5 rounded-full font-semibold ${pill.cls} ${overdue ? 'ring-1 ring-danger' : ''}`}
                            style={{ minHeight: 'auto' }}
                            title={overdue ? 'Overdue' : `Invoice ${s.invoice_status}`}
                          >
                            {overdue ? '!' : pill.label}
                          </button>
                        ) : (
                          <button
                            onClick={e => { e.stopPropagation(); navigate(`/invoices/new?session_id=${s.id}`); }}
                            className="text-[10px] text-gray-300 hover:text-navy transition-colors font-medium"
                            style={{ minHeight: 'auto' }}
                            title="Create invoice"
                          >
                            +
                          </button>
                        )}
                      </td>
                    </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {/* Per-client profitability */}
        {byClient.length > 0 && (
          <div className="card mb-4">
            <div className="section-header">Client Profitability</div>
            <div className="space-y-3">
              {byClient.map(c => (
                <div key={c.client_id} className="flex items-center justify-between py-2 border-b border-gray-100 last:border-0">
                  <div>
                    <div className="text-sm font-medium text-navy">{c.client_name}</div>
                    <div className="text-xs text-gray-400">{c.session_count} sessions · avg ${(c.avg_fee || 0).toFixed(0)}/session</div>
                  </div>
                  <div className="text-right">
                    <div className={`text-sm font-bold ${marginClass(c.avg_margin_pct)}`}>{(c.avg_margin_pct || 0).toFixed(0)}%</div>
                    <div className="text-xs text-gray-400">avg margin</div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
