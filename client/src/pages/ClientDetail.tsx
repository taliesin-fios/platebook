// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, type ReactNode } from 'react';
import { useNavigate, useParams } from 'react-router';
import { fmtSessionDate } from '../utils/dateUtils';
import PageHeader from '../components/PageHeader';
import { STATUS_COLORS, marginColor } from '../constants';
import { useToast } from '../components/Toast';
import { apiCall } from '../hooks/useApi';
import { parseNum } from '../utils/numbers';
import type { ClientDetail as ClientDetailData, InvoiceListItem } from '../types';

const INVOICE_STATUS: Record<string, { label: string; bg: string; text: string }> = {
  draft: { label: 'Draft', bg: 'bg-gray-100',  text: 'text-gray-600' },
  sent:  { label: 'Sent',  bg: 'bg-blue-100',  text: 'text-blue-700' },
  paid:  { label: 'Paid',  bg: 'bg-green-100', text: 'text-green-700' },
  void:  { label: 'Void',  bg: 'bg-red-100',   text: 'text-red-600' },
};
const fmt$ = (v: unknown) => `$${(parseNum(v) || 0).toLocaleString('en-US', { minimumFractionDigits: 0, maximumFractionDigits: 0 })}`;

const InfoRow = ({ label, value }: { label: string; value: ReactNode }) => value ? (
  <div className="flex justify-between py-2 border-b border-gray-100 last:border-0">
    <span className="text-sm text-gray-500">{label}</span>
    <span className="text-sm font-medium text-gray-800 text-right max-w-[60%]">{value}</span>
  </div>
) : null;

const ChipList = ({ items }: { items?: string[] | null }) => items && items.length > 0 ? (
  <div className="flex flex-wrap gap-1.5">
    {items.map(i => (
      <span key={i} className="text-xs px-2.5 py-1 bg-blue-light text-navy-mid rounded-full font-medium">{i}</span>
    ))}
  </div>
) : <span className="text-sm text-gray-400">None</span>;

export default function ClientDetail() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { show } = useToast();
  const [client,       setClient]       = useState<ClientDetailData | { error: true } | null>(null);
  const [invoices,     setInvoices]     = useState<InvoiceListItem[]>([]);
  const [tab,          setTab]          = useState('overview');
  const [loading,      setLoading]      = useState(true);
  const [editingNotes, setEditingNotes] = useState(false);
  const [notesDraft,   setNotesDraft]   = useState('');
  const [savingNotes,  setSavingNotes]  = useState(false);

  useEffect(() => {
    // C4: reset prior state when navigating between clients
    setClient(null);
    setInvoices([]);
    setLoading(true);
    fetch(`/api/clients/${id}`)
      .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
      .then(d => { setClient(d); })
      .catch(() => setClient({ error: true }))
      .finally(() => setLoading(false));
    fetch(`/api/invoices?client_id=${id}`)
      .then(r => r.ok ? r.json() : Promise.resolve([]))
      .then(d => setInvoices(Array.isArray(d) ? d : []))
      .catch(() => setInvoices([]));
  }, [id]);

  const handleSaveNotes = async () => {
    setSavingNotes(true);
    try {
      await apiCall(`/api/clients/${id}`, 'PUT', { chef_notes: notesDraft });
      setClient(prev => ({ ...prev, chef_notes: notesDraft }) as ClientDetailData);
      setEditingNotes(false);
      show('Notes saved', 'success');
    } catch {
      show('Could not save notes', 'error');
    } finally {
      setSavingNotes(false);
    }
  };

  if (loading) return (
    <div className="page">
      <PageHeader title="Client" back="/clients" />
      <div className="px-4 pt-4"><div className="skeleton h-6 w-1/2 mb-3" /><div className="skeleton h-4 w-full mb-2" /></div>
    </div>
  );

  if (!client || 'error' in client) return (
    <div className="page">
      <PageHeader title="Client" back="/clients" />
      <div className="px-4 pt-8 text-center text-gray-500">Client not found</div>
    </div>
  );

  return (
    <div className="page">
      <PageHeader
        title={client.name}
        back="/clients"
        action={
          <div className="flex items-center gap-2">
            <button
              onClick={() => navigate(`/sessions/new?client_id=${id}`)}
              className="text-white text-sm font-medium bg-white/20 px-3 py-1 rounded-lg"
              style={{ minHeight: 'auto' }}
            >
              + Session
            </button>
            <button
              onClick={() => navigate(`/invoices/new?client_id=${id}`)}
              className="text-white text-sm font-medium bg-white/20 px-3 py-1 rounded-lg"
              style={{ minHeight: 'auto' }}
            >
              🧾 Invoice
            </button>
            <button onClick={() => navigate(`/clients/${id}/edit`)} className="text-white text-sm font-medium" style={{ minHeight: 'auto' }}>
              Edit
            </button>
          </div>
        }
      />

      {/* Tabs */}
      <div className="flex border-b border-gray-200 bg-white sticky top-[57px] z-10">
        {[
          { key: 'overview',  label: 'Overview' },
          { key: 'sessions',  label: `Sessions (${client.sessions?.length || 0})` },
          { key: 'invoices',  label: `Invoices (${invoices.length})` },
        ].map(t => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`flex-1 py-3 text-sm font-medium transition-colors ${
              tab === t.key ? 'text-green-dark border-b-2 border-green-dark' : 'text-gray-500'
            }`}
            style={{ minHeight: '44px' }}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div className="px-4 pt-4">
        {client.is_sample === 1 && (
          <div className="mb-3 px-3 py-2 bg-amber-50 border border-amber-200 rounded-lg text-xs text-amber-700 font-medium">
            ⚠ This is a sample client — replace or delete before real use.
          </div>
        )}

        {tab === 'overview' && (
          <div className="space-y-3">
            {/* Financial summary — computed from sessions data */}
            {(() => {
              const allSessions = client.sessions || [];
              const completed = allSessions.filter(s => s.status === 'Completed');
              const totalRevenue = completed.reduce((n, s) => n + (s.service_fee || 0), 0);
              const totalCosts   = completed.reduce((n, s) => n + (s.ingredient_cost || 0), 0);
              const avgMarginPct = totalRevenue > 0
                ? Math.round(((totalRevenue - totalCosts) / totalRevenue) * 100)
                : null;
              const unpaidInvoices = invoices.filter(i => i.status !== 'paid' && i.status !== 'void');
              const outstanding = unpaidInvoices.reduce((n, i) => n + (i.total || 0), 0);

              return (
                <div className="card">
                  <div className="section-header">At a Glance</div>
                  <div className="grid grid-cols-3 gap-3 text-center">
                    <div>
                      <div className="text-xl font-bold font-display text-navy">{allSessions.length}</div>
                      <div className="text-xs text-gray-500">Sessions</div>
                    </div>
                    <div>
                      <div className="text-xl font-bold font-display text-green-dark">
                        {fmt$(totalRevenue)}
                      </div>
                      <div className="text-xs text-gray-500">Earned</div>
                    </div>
                    <div>
                      <div className={`text-xl font-bold font-display ${
                        avgMarginPct === null ? 'text-gray-400' : marginColor(avgMarginPct)
                      }`}>
                        {avgMarginPct !== null ? `${avgMarginPct}%` : '—'}
                      </div>
                      <div className="text-xs text-gray-500">Avg margin</div>
                    </div>
                  </div>
                  {outstanding > 0 && (
                    <div className="mt-3 flex items-center gap-2 px-3 py-2 bg-amber-50 border border-amber-200 rounded-xl">
                      <span className="text-amber-600 text-sm">⚠</span>
                      <span className="text-xs text-amber-700 font-medium">
                        {fmt$(outstanding)} outstanding across {unpaidInvoices.length} invoice{unpaidInvoices.length !== 1 ? 's' : ''}
                      </span>
                    </div>
                  )}
                </div>
              );
            })()}

            <div className="card">
              <div className="section-header">Personal Info</div>
              <InfoRow label="Household" value={client.household_type} />
              <InfoRow label="Adults / Children" value={`${client.adults} / ${client.children}`} />
              <InfoRow label="Serving Size" value={`${client.serving_size} servings`} />
              <InfoRow label="Phone" value={client.phone} />
              <InfoRow label="Email" value={client.email} />
            </div>

            {(client.allergens?.length > 0 || client.dietary_styles?.length > 0 || client.liked_proteins?.length > 0 || client.disliked_foods) && (
              <div className="card">
                <div className="section-header">Dietary Profile</div>
                {client.allergens?.length > 0 && (
                  <div className="mb-3">
                    <div className="text-xs text-gray-500 mb-1.5">Allergens</div>
                    <div className="flex flex-wrap gap-1.5">
                      {client.allergens.map(a => (
                        <span key={a} className="text-xs px-2.5 py-1 bg-red-50 text-danger rounded-full font-medium">{a}</span>
                      ))}
                    </div>
                  </div>
                )}
                {client.dietary_styles?.length > 0 && (
                  <div className="mb-3">
                    <div className="text-xs text-gray-500 mb-1.5">Dietary Style</div>
                    <ChipList items={client.dietary_styles} />
                  </div>
                )}
                {client.liked_proteins?.length > 0 && (
                  <div className="mb-3">
                    <div className="text-xs text-gray-500 mb-1.5">Preferred Proteins</div>
                    <ChipList items={client.liked_proteins} />
                  </div>
                )}
                {client.disliked_foods && <InfoRow label="Foods to Avoid" value={client.disliked_foods} />}
                <InfoRow label="Spice Tolerance" value={client.spice_tolerance} />
              </div>
            )}

            <div className="card">
              <div className="section-header">Service Preferences</div>
              <InfoRow label="Preferred Day" value={client.preferred_day} />
              <InfoRow label="Preferred Time" value={client.preferred_time} />
              <InfoRow label="Grocery Purchasing" value={client.grocery_handling} />
              <InfoRow label="Stovetop" value={client.stovetop_type} />
              <InfoRow label="Oven Available" value={client.oven_available ? 'Yes' : 'No'} />
              <InfoRow label="Parking" value={client.parking_notes} />
            </div>

            {/* Chef Notes — always visible, inline-editable */}
            <div className="card">
              <div className="flex items-center justify-between mb-2">
                <div className="section-header mb-0">Chef Notes</div>
                {!editingNotes && (
                  <button
                    onClick={() => { setNotesDraft(client.chef_notes || ''); setEditingNotes(true); }}
                    className="text-xs text-gray-400 hover:text-navy flex items-center gap-1"
                    style={{ minHeight: 'auto' }}
                  >
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/>
                      <path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/>
                    </svg>
                    {client.chef_notes ? 'Edit' : 'Add'}
                  </button>
                )}
              </div>

              {editingNotes ? (
                <div>
                  <textarea
                    rows={4}
                    autoFocus
                    value={notesDraft}
                    onChange={e => setNotesDraft(e.target.value)}
                    placeholder="Access notes, kitchen layout, pet info, preferences, special instructions…"
                    className="w-full text-sm border border-gray-200 rounded-xl px-3 py-2.5 focus:outline-hidden focus:ring-2 focus:ring-green-mid resize-none"
                  />
                  <div className="flex gap-2 mt-2">
                    <button
                      onClick={handleSaveNotes}
                      disabled={savingNotes}
                      className="flex-1 py-2 bg-navy text-white rounded-xl text-sm font-semibold disabled:opacity-60"
                      style={{ minHeight: '40px' }}
                    >
                      {savingNotes ? 'Saving…' : 'Save'}
                    </button>
                    <button
                      onClick={() => setEditingNotes(false)}
                      className="px-4 py-2 border border-gray-200 text-gray-600 rounded-xl text-sm"
                      style={{ minHeight: '40px' }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ) : client.chef_notes ? (
                <p className="text-sm text-gray-700 whitespace-pre-wrap">{client.chef_notes}</p>
              ) : (
                <p className="text-sm text-gray-400 italic">No notes for this client</p>
              )}
            </div>
          </div>
        )}

        {tab === 'sessions' && (
          <div>
            <button
              onClick={() => navigate(`/sessions/new?client_id=${id}`)}
              className="w-full mb-4 py-3 border-2 border-dashed border-gray-200 rounded-xl text-sm text-gray-400 hover:border-green-mid hover:text-green-dark transition-colors"
            >
              + Schedule Session for {client.name}
            </button>

            {!client.sessions?.length ? (
              <div className="card text-center py-8">
                <div className="text-3xl mb-2">📅</div>
                <div className="text-gray-500 text-sm font-medium mb-1">No sessions yet</div>
                <div className="text-xs text-gray-400">Schedule the first session above</div>
              </div>
            ) : client.sessions.map(s => (
              <div
                key={s.id}
                className="card mb-3 cursor-pointer hover:shadow-card-hover transition-shadow"
                onClick={() => navigate(`/sessions/${s.id}`)}
              >
                <div className="flex items-center justify-between">
                  <div>
                    <div className="text-sm font-semibold text-navy">
                      {fmtSessionDate(s.session_date, 'EEEE, MMM d, yyyy')}
                    </div>
                    <div className="text-xs text-gray-500 mt-0.5">{s.dish_count} dishes · ${(parseNum(s.service_fee) || 0).toFixed(0)}</div>
                  </div>
                  <span className={`text-xs px-2.5 py-1 rounded-full font-medium ${STATUS_COLORS[s.status] || 'bg-gray-100'}`}>
                    {s.status}
                  </span>
                </div>
              </div>
            ))}
          </div>
        )}

        {tab === 'invoices' && (
          <div>
            <button
              onClick={() => navigate(`/invoices/new?client_id=${id}`)}
              className="w-full mb-4 py-3 border-2 border-dashed border-gray-200 rounded-xl text-sm text-gray-400 hover:border-green-mid hover:text-green-dark transition-colors"
            >
              + Generate Invoice for {client.name}
            </button>

            {invoices.length === 0 ? (
              <div className="card text-center py-8">
                <div className="text-2xl mb-2">🧾</div>
                <div className="text-gray-400 text-sm">No invoices yet</div>
              </div>
            ) : invoices.map(inv => {
              const cfg = INVOICE_STATUS[inv.status] || INVOICE_STATUS.draft;
              return (
                <div
                  key={inv.id}
                  className="card mb-3 cursor-pointer hover:shadow-card-hover transition-shadow"
                  onClick={() => navigate(`/invoices/${inv.id}`)}
                >
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="flex items-center gap-2 mb-0.5">
                        <span className="text-sm font-bold text-navy">{inv.invoice_number || `INV-${inv.id}`}</span>
                        <span className={`text-xs px-2 py-0.5 rounded-full font-semibold ${cfg.bg} ${cfg.text}`}>{cfg.label}</span>
                      </div>
                      {inv.issue_date && (
                        <div className="text-xs text-gray-400">
                          {fmtSessionDate(inv.issue_date, 'MMM d, yyyy')}
                        </div>
                      )}
                    </div>
                    <div className="text-base font-bold text-navy">{fmt$(inv.total)}</div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
