// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { format, isAfter } from 'date-fns';
import { parseLocalDate } from '../utils/dateUtils';
import PageHeader from '../components/PageHeader';
import type { InvoiceListItem } from '../types';

const STATUS_CONFIG: Record<string, { label: string; bg: string; text: string }> = {
  draft: { label: 'Draft', bg: 'bg-gray-100',  text: 'text-gray-600' },
  sent:  { label: 'Sent',  bg: 'bg-blue-100',  text: 'text-blue-700' },
  paid:  { label: 'Paid',  bg: 'bg-green-100', text: 'text-green-700' },
  void:  { label: 'Void',  bg: 'bg-red-100',   text: 'text-red-600' },
};

const fmt$ = (v: number | string | null | undefined) => `$${(parseFloat(String(v)) || 0).toFixed(2)}`;
const fmtDate = (str: string | null) => {
  try { return format(parseLocalDate(str), 'MMM d, yyyy'); } catch { return str; }
};
const isOverdue = (due: string | null, status: string) => {
  if (!due || status === 'paid' || status === 'void') return false;
  // parseLocalDate anchors to local noon — an invoice due today is not overdue
  // until tomorrow, instead of flipping at 4-5pm local (UTC midnight).
  try { return isAfter(new Date(), parseLocalDate(due)); } catch { return false; }
};

export default function Invoices() {
  const navigate      = useNavigate();
  const [searchParams] = useSearchParams();
  const [invoices, setInvoices]   = useState<InvoiceListItem[]>([]);
  const [loading,  setLoading]    = useState(true);
  // Initialise from ?status= URL param so deep-links work (e.g. from Costs page)
  const [filter,   setFilter]     = useState(() => {
    const p = searchParams.get('status');
    return p && ['draft','sent','paid','void'].includes(p) ? p : 'all';
  });

  useEffect(() => {
    setLoading(true);
    const q = filter !== 'all' ? `?status=${filter}` : '';
    fetch(`/api/invoices${q}`)
      .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
      .then(data => setInvoices(Array.isArray(data) ? data : []))
      .catch(() => setInvoices([]))
      .finally(() => setLoading(false));
  }, [filter]);

  const filters = ['all', 'draft', 'sent', 'paid', 'void'];

  return (
    <div className="page">
      <PageHeader
        title="Invoices"
        action={
          <button
            onClick={() => navigate('/invoices/new')}
            className="text-white text-sm font-medium"
            style={{ minHeight: 'auto' }}
          >
            + New
          </button>
        }
      />

      {/* Filter tabs */}
      <div className="flex gap-1 px-4 py-2 overflow-x-auto border-b border-gray-100 bg-white sticky top-[57px] z-10">
        {filters.map(f => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={`px-3 py-1.5 rounded-full text-xs font-semibold whitespace-nowrap transition-colors ${
              filter === f
                ? 'bg-navy text-white'
                : 'bg-gray-100 text-gray-500 hover:bg-gray-200'
            }`}
          >
            {f === 'all' ? 'All' : STATUS_CONFIG[f]?.label || f}
          </button>
        ))}
      </div>

      <div className="px-4 pt-4 space-y-3 pb-24">
        {loading ? (
          [1,2,3].map(i => <div key={i} className="skeleton h-20 rounded-2xl" />)
        ) : invoices.length === 0 ? (
          <div className="card text-center py-12">
            <div className="text-3xl mb-3">🧾</div>
            <div className="text-gray-500 font-medium mb-1">No invoices yet</div>
            <div className="text-xs text-gray-400 mb-4">Create from a session or client record</div>
            <button
              onClick={() => navigate('/invoices/new')}
              className="btn-primary text-sm px-4 py-2"
            >
              Create Invoice
            </button>
          </div>
        ) : (
          invoices.map(inv => {
            const cfg = STATUS_CONFIG[inv.status] || STATUS_CONFIG.draft;
            return (
              <div
                key={inv.id}
                className="card cursor-pointer hover:shadow-card-hover transition-shadow"
                onClick={() => navigate(`/invoices/${inv.id}`)}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 mb-0.5">
                      <span className="text-sm font-bold text-navy">
                        {inv.invoice_number || `INV-${inv.id}`}
                      </span>
                      <span className={`text-xs px-2 py-0.5 rounded-full font-semibold ${cfg.bg} ${cfg.text}`}>
                        {cfg.label}
                      </span>
                      {isOverdue(inv.due_date, inv.status) && (
                        <span className="text-xs px-2 py-0.5 rounded-full font-semibold bg-red-100 text-red-600">
                          Overdue
                        </span>
                      )}
                    </div>
                    <div className="text-sm text-gray-700 truncate">
                      {inv.client_name || inv.client_name_ref || 'No client'}
                    </div>
                    <div className="text-xs text-gray-400 mt-0.5">
                      {inv.status === 'paid' && inv.paid_at ? (
                        <span className="text-green-600 font-medium">
                          ✓ Paid {fmtDate(inv.paid_at.slice(0, 10))}
                        </span>
                      ) : inv.due_date ? (
                        <span className={isOverdue(inv.due_date, inv.status) ? 'text-red-500 font-medium' : ''}>
                          Due {fmtDate(inv.due_date)}
                        </span>
                      ) : inv.issue_date ? (
                        `Issued ${fmtDate(inv.issue_date)}`
                      ) : null}
                    </div>
                  </div>
                  <div className="text-right shrink-0">
                    <div className="text-base font-bold text-navy">{fmt$(inv.total)}</div>
                    {inv.status !== 'paid' && inv.issue_date && (
                      <div className="text-xs text-gray-400 mt-0.5">
                        {fmtDate(inv.issue_date)}
                      </div>
                    )}
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
