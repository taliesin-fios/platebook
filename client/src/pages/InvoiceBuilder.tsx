// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, useCallback, useRef } from 'react';
import { useParams, useNavigate, useSearchParams } from 'react-router';
import { format } from 'date-fns';
import { localDateStrOffset, localTodayStr, parseLocalDate } from '../utils/dateUtils';
import PageHeader from '../components/PageHeader';
import EmailFailedBanner from '../components/EmailFailedBanner';
import { useToast } from '../components/Toast';
import { downloadServerPdf, pdfFilename } from '../utils/serverPdf';
import { parseNum, type NumInput } from '../utils/numbers';
import type { ClientLink, EmailSentResponse, Invoice, Settings } from '../types';

interface LineItemDraft {
  _key: string;
  id?: number;
  invoice_id?: number;
  sort_order: number;
  description: string;
  quantity: NumInput;
  unit_price: NumInput;
  amount: NumInput;
}

interface InvoiceForm {
  status: string;
  issue_date: string;
  due_date: string;
  client_name: string;
  client_email: string;
  client_phone: string;
  client_address: string;
  business_name: string;
  chef_name: string;
  chef_email: string;
  chef_phone: string;
  message: string;
  internal_notes: string;
  tax_rate: NumInput;
  invoice_number: string;
  line_items: LineItemDraft[];
}

type Preset = { description: string; unit_price: number };

type ApiResult<T> = T & { error?: string; code?: string };

// ── Constants ──────────────────────────────────────────────────────────────────

const LINE_ITEM_PRESETS: Preset[] = [
  { description: 'Meal Prep Session',              unit_price: 0 },
  { description: 'Grocery Shopping',               unit_price: 0 },
  { description: 'Recipe Consultation',            unit_price: 0 },
  { description: 'Custom Menu Planning',           unit_price: 0 },
  { description: 'Travel / Mileage',               unit_price: 0 },
  { description: 'Specialty Ingredients',          unit_price: 0 },
  { description: 'Equipment Fee',                  unit_price: 0 },
  { description: 'Cleaning & Prep',                unit_price: 0 },
  { description: 'Portioning & Labeling',          unit_price: 0 },
  { description: 'Delivery Fee',                   unit_price: 0 },
  { description: 'Gratuity',                       unit_price: 0 },
  { description: 'Discount',                       unit_price: 0 },
];

const STATUS_CONFIG: Record<string, { label: string; bg: string; text: string }> = {
  draft: { label: 'Draft',  bg: 'bg-gray-100',   text: 'text-gray-600' },
  sent:  { label: 'Sent',   bg: 'bg-blue-100',   text: 'text-blue-700' },
  paid:  { label: 'Paid',   bg: 'bg-green-100',  text: 'text-green-700' },
  void:  { label: 'Void',   bg: 'bg-red-100',    text: 'text-red-600' },
};

const fmt$ = (v: unknown) => `$${(parseNum(v) || 0).toFixed(2)}`;
const fmtDate = (str: string) => {
  try { return format(parseLocalDate(str), 'MMMM d, yyyy'); } catch { return str || ''; }
};

// Compares forms for unsaved edits; status only changes through a save.
const formKey = (f: InvoiceForm) => JSON.stringify({ ...f, status: null });

// ── Blank line item ────────────────────────────────────────────────────────────
const blankLine = (idx = 0): LineItemDraft => ({
  _key:        Math.random().toString(36).slice(2),
  sort_order:  idx,
  description: '',
  quantity:    1,
  unit_price:  0,
  amount:      0,
});

// ── Main component ─────────────────────────────────────────────────────────────
export default function InvoiceBuilder() {
  const { id }           = useParams();
  const [searchParams]   = useSearchParams();
  const navigate         = useNavigate();
  const { show }         = useToast();
  const isNew            = !id || id === 'new';

  // ── State ────────────────────────────────────────────────────────────────────
  const [loading,     setLoading]     = useState(!isNew);
  const [saving,      setSaving]      = useState(false);
  const [emailing,    setEmailing]    = useState(false);
  const [showPresets, setShowPresets] = useState(false);
  const [logoBase64,  setLogoBase64]  = useState<string | null>(null);
  // Last failed email (server-recorded; not part of the editable form)
  const [emailFailedAt, setEmailFailedAt] = useState<string | null>(null);
  const [savedKey,    setSavedKey]    = useState('');
  const presetsRef = useRef<HTMLDivElement>(null);
  const createdFromRef = useRef<string | null>(null); // StrictMode runs effects twice: POST once per source

  // Convert a server invoice object → form state
  const invToForm = (inv: Invoice): InvoiceForm => ({
    status:         inv.status         || 'draft',
    issue_date:     inv.issue_date     || '',
    due_date:       inv.due_date       || '',
    client_name:    inv.client_name    || '',
    client_email:   inv.client_email   || '',
    client_phone:   inv.client_phone   || '',
    client_address: inv.client_address || '',
    business_name:  inv.business_name  || '',
    chef_name:      inv.chef_name      || '',
    chef_email:     inv.chef_email     || '',
    chef_phone:     inv.chef_phone     || '',
    message:        inv.message        || '',
    internal_notes: inv.internal_notes || '',
    tax_rate:       inv.tax_rate       || 0,
    invoice_number: inv.invoice_number || '',
    line_items:     (inv.line_items || []).map(li => ({
      ...li,
      _key: Math.random().toString(36).slice(2),
    })),
  });

  // Load logo + chef defaults from settings once on mount.
  // Chef defaults only apply to new invoices — never overwrite saved invoice fields.
  useEffect(() => {
    fetch('/api/settings')
      .then(r => r.ok ? r.json() : {})
      .then((d: Partial<Settings>) => {
        if (d.logo_base64) setLogoBase64(d.logo_base64);
        if (isNew) {
          setForm(f => ({
            ...f,
            business_name: f.business_name || d.business_name || '',
            chef_name:     f.chef_name     || d.chef_name     || '',
            chef_email:    f.chef_email    || d.email         || '',
            chef_phone:    f.chef_phone    || d.phone         || '',
          }));
        }
      })
      .catch(() => console.warn('Could not load settings for invoice'));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const [form, setForm] = useState<InvoiceForm>({
    status:          'draft',
    issue_date:      localTodayStr(),
    due_date:        localDateStrOffset(14),
    client_name:     '',
    client_email:    '',
    client_phone:    '',
    client_address:  '',
    business_name:   '',
    chef_name:       '',
    chef_email:      '',
    chef_phone:      '',
    message:         '',
    internal_notes:  '',
    tax_rate:        0,
    line_items:      [blankLine(0)],
    invoice_number:  '',
  });

  // Close preset dropdown on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (presetsRef.current && !presetsRef.current.contains(e.target as Node)) {
        setShowPresets(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  // ── Load existing invoice ────────────────────────────────────────────────────
  useEffect(() => {
    if (isNew) {
      // Pre-fill from session or client via query params
      const sessionId = searchParams.get('session_id');
      const clientId  = searchParams.get('client_id');

      if (sessionId) {
        if (createdFromRef.current === `s${sessionId}`) return;
        createdFromRef.current = `s${sessionId}`;
        fetch(`/api/invoices/from-session/${sessionId}`, { method: 'POST' })
          .then(r => r.ok ? r.json() : { error: `Request failed: ${r.status}` })
          .then((inv: ApiResult<Invoice>) => {
            if (inv.error) { show(inv.error, 'error'); return; }
            navigate(`/invoices/${inv.id}`, { replace: true });
          })
          .catch(() => show('Could not create invoice from session', 'error'));
      } else if (clientId) {
        if (createdFromRef.current === `c${clientId}`) return;
        createdFromRef.current = `c${clientId}`;
        fetch(`/api/invoices/from-client/${clientId}`, { method: 'POST' })
          .then(r => r.ok ? r.json() : { error: `Request failed: ${r.status}` })
          .then((inv: ApiResult<Invoice>) => {
            if (inv.error) { show(inv.error, 'error'); return; }
            navigate(`/invoices/${inv.id}`, { replace: true });
          })
          .catch(() => show('Could not create invoice from client', 'error'));
      }
      return;
    }

    setLoading(true);
    fetch(`/api/invoices/${id}`)
      .then(r => r.ok ? r.json() : { error: 'Invoice not found' })
      .then((inv: ApiResult<Invoice>) => {
        if (inv.error) { show('Invoice not found', 'error'); navigate('/invoices'); return; }
        const loaded = invToForm(inv);
        setEmailFailedAt(inv.email_failed_at);
        setForm(loaded);
        setSavedKey(formKey(loaded));
      })
      .finally(() => setLoading(false));
  }, [id]);

  // ── Derived totals ───────────────────────────────────────────────────────────
  const subtotal  = form.line_items.reduce((s, li) => s + (parseNum(li.amount) || 0), 0);
  const taxAmount = subtotal * (parseNum(form.tax_rate) / 100 || 0);
  const total     = subtotal + taxAmount;

  // ── Field helpers ────────────────────────────────────────────────────────────
  const setField = <K extends keyof InvoiceForm>(key: K, val: InvoiceForm[K]) => setForm(f => ({ ...f, [key]: val }));

  const updateLine = (key: string, field: 'description' | 'quantity' | 'unit_price', val: string) => {
    setForm(f => ({
      ...f,
      line_items: f.line_items.map(li => {
        if (li._key !== key) return li;
        const next: LineItemDraft = { ...li, [field]: val };
        if (field === 'quantity' || field === 'unit_price') {
          next.amount = (parseNum(field === 'quantity' ? val : next.quantity) || 0)
                      * (parseNum(field === 'unit_price' ? val : next.unit_price) || 0);
        }
        return next;
      }),
    }));
  };

  const addLine = (preset: Preset | null = null) => {
    const idx = form.line_items.length;
    if (preset) {
      setForm(f => ({
        ...f,
        line_items: [...f.line_items, {
          _key:        Math.random().toString(36).slice(2),
          sort_order:  idx,
          description: preset.description,
          quantity:    1,
          unit_price:  preset.unit_price,
          amount:      preset.unit_price,
        }],
      }));
    } else {
      setForm(f => ({ ...f, line_items: [...f.line_items, blankLine(idx)] }));
    }
    setShowPresets(false);
  };

  const removeLine = (key: string) => {
    setForm(f => ({ ...f, line_items: f.line_items.filter(li => li._key !== key) }));
  };

  // ── Save ──────────────────────────────────────────────────────────────────────
  const handleSave = async (nextStatus?: string): Promise<boolean> => {
    setSaving(true);
    const payload = {
      ...form,
      status: nextStatus || form.status,
      line_items: form.line_items.map((li, i) => ({ ...li, sort_order: i })),
    };

    try {
      const url    = isNew ? '/api/invoices' : `/api/invoices/${id}`;
      const method = isNew ? 'POST' : 'PUT';
      const r      = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const inv: ApiResult<Invoice> = await r.json();
      if (inv.error) { show(inv.error, 'error'); return false; }

      if (isNew) {
        navigate(`/invoices/${inv.id}`, { replace: true });
      } else {
        const saved = invToForm(inv);
        setForm(saved);
        setSavedKey(formKey(saved));
        show('Invoice saved', 'success');
      }
      return true;
    } catch {
      show('Save failed', 'error');
      return false;
    } finally {
      setSaving(false);
    }
  };

  const handleMarkPaid = () => handleSave('paid');

  // The server renders the saved invoice, so save pending edits first
  // (paid and void invoices can't change).
  const saveIfEdited = async () => {
    const editable = form.status === 'draft' || form.status === 'sent';
    if (!editable || formKey(form) === savedKey) return true;
    return handleSave();
  };

  const handlePrint = async () => {
    if (isNew) {
      show('Save the invoice first, then download the PDF', 'error');
      return;
    }
    try {
      if (!(await saveIfEdited())) return;
      const number = form.invoice_number || `INV-${id}`;
      await downloadServerPdf(`/api/invoices/${id}/pdf`, pdfFilename.invoice(number));
    } catch {
      // fallback to browser print if PDF gen fails
      window.print();
    }
  };

  const copyClientLink = async () => {
    try {
      const r = await fetch(`/api/invoices/${id}/client-link`);
      const data: ApiResult<ClientLink> = await r.json();
      if (!r.ok || !data.url) throw new Error(data.error || 'Could not get the link');
      await navigator.clipboard.writeText(data.url);
      show('Invoice link copied', 'success');
    } catch (err) {
      show(err instanceof Error ? err.message : 'Could not copy the link', 'error');
    }
  };

  const handleEmail = async () => {
    if (!form.client_email) {
      show('Add a client email address before sending', 'error');
      return;
    }
    if (isNew) {
      show('Save the invoice first, then send it', 'error');
      return;
    }
    setEmailing(true);
    try {
      if (!(await saveIfEdited())) return;
      // The server renders and attaches the saved invoice itself.
      const r = await fetch(`/api/invoices/${id}/email`, { method: 'POST' });
      const data: ApiResult<EmailSentResponse> = await r.json();
      if (!r.ok) {
        if (data.code !== 'NO_EMAIL') setEmailFailedAt(new Date().toISOString());
        show(data.error || 'Failed to send invoice', 'error');
        return;
      }
      setEmailFailedAt(null);
      show(`Invoice sent to ${data.sent_to}`, 'success');
      // Reflect status change (draft → sent) in form
      if (form.status === 'draft') setForm(f => ({ ...f, status: 'sent' }));
    } catch {
      show('Could not generate or send the invoice PDF', 'error');
    } finally {
      setEmailing(false);
    }
  };

  // ── Loading ────────────────────────────────────────────────────────────────
  if (loading) return (
    <div className="page">
      <PageHeader title="Invoice" back="/invoices" />
      <div className="px-4 pt-4 space-y-3">
        {[1,2,3].map(i => <div key={i} className="skeleton h-20 rounded-2xl" />)}
      </div>
    </div>
  );

  const statusCfg = STATUS_CONFIG[form.status] || STATUS_CONFIG.draft;

  return (
    <div className="page pb-0">
      <div className="invoice-no-print">
      <PageHeader
        title={form.invoice_number ? `Invoice ${form.invoice_number}` : 'New Invoice'}
        back="/invoices"
        action={
          <span className={`text-xs font-semibold px-2.5 py-1 rounded-full ${statusCfg.bg} ${statusCfg.text}`}>
            {statusCfg.label}
          </span>
        }
      />
      <EmailFailedBanner what="invoice" failedAt={emailFailedAt} onRetry={handleEmail} retrying={emailing} />

      </div>{/* end invoice-no-print (page header) */}

      {/* ── Split layout ──────────────────────────────────────────────────── */}
      <div className="invoice-builder-shell px-4 pt-4 pb-28 md:pb-6">

        {/* ══ LEFT: BUILDER ══════════════════════════════════════════════════ */}
        <div className="invoice-builder-panel space-y-4">

          {/* Status selector */}
          <div className="card">
            <div className="section-header mb-3">Invoice Status</div>
            <div className="flex gap-2 flex-wrap">
              {Object.entries(STATUS_CONFIG).map(([s, cfg]) => (
                <button
                  key={s}
                  onClick={() => setField('status', s)}
                  className={`px-3 py-1.5 rounded-full text-xs font-semibold border transition-all ${
                    form.status === s
                      ? `${cfg.bg} ${cfg.text} border-transparent ring-2 ring-offset-1 ${s === 'paid' ? 'ring-green-500' : s === 'sent' ? 'ring-blue-400' : s === 'void' ? 'ring-red-400' : 'ring-gray-400'}`
                      : 'bg-white text-gray-500 border-gray-200 hover:border-gray-400'
                  }`}
                >
                  {cfg.label}
                </button>
              ))}
            </div>
          </div>

          {/* Dates */}
          <div className="card">
            <div className="section-header mb-3">Dates</div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-xs text-gray-500 mb-1 block">Issue Date</label>
                <input
                  type="date"
                  value={form.issue_date}
                  onChange={e => setField('issue_date', e.target.value)}
                  className="input-field w-full text-sm"
                />
              </div>
              <div>
                <label className="text-xs text-gray-500 mb-1 block">Due Date</label>
                <input
                  type="date"
                  value={form.due_date}
                  onChange={e => setField('due_date', e.target.value)}
                  className="input-field w-full text-sm"
                />
              </div>
            </div>
          </div>

          {/* Bill To */}
          <div className="card">
            <div className="section-header mb-3">Bill To (Client)</div>
            <div className="space-y-2.5">
              <div>
                <label className="text-xs text-gray-500 mb-1 block">Client Name</label>
                <input
                  type="text"
                  value={form.client_name}
                  onChange={e => setField('client_name', e.target.value)}
                  placeholder="Client name"
                  className="input-field w-full text-sm"
                />
              </div>
              <div>
                <label className="text-xs text-gray-500 mb-1 block">Email</label>
                <input
                  type="email"
                  value={form.client_email}
                  onChange={e => setField('client_email', e.target.value)}
                  placeholder="client@email.com"
                  className="input-field w-full text-sm"
                />
              </div>
              <div>
                <label className="text-xs text-gray-500 mb-1 block">Phone</label>
                <input
                  type="tel"
                  value={form.client_phone}
                  onChange={e => setField('client_phone', e.target.value)}
                  placeholder="(555) 000-0000"
                  className="input-field w-full text-sm"
                />
              </div>
              <div>
                <label className="text-xs text-gray-500 mb-1 block">Address</label>
                <textarea
                  value={form.client_address}
                  onChange={e => setField('client_address', e.target.value)}
                  placeholder="Client address"
                  rows={2}
                  className="input-field w-full text-sm resize-none"
                />
              </div>
            </div>
          </div>

          {/* From (Chef/Business) */}
          <div className="card">
            <div className="section-header mb-3">From (Your Business)</div>
            <div className="space-y-2.5">
              <div>
                <label className="text-xs text-gray-500 mb-1 block">Business Name</label>
                <input
                  type="text"
                  value={form.business_name}
                  onChange={e => setField('business_name', e.target.value)}
                  className="input-field w-full text-sm"
                />
              </div>
              <div>
                <label className="text-xs text-gray-500 mb-1 block">Chef / Contact Name</label>
                <input
                  type="text"
                  value={form.chef_name}
                  onChange={e => setField('chef_name', e.target.value)}
                  placeholder="Your name"
                  className="input-field w-full text-sm"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="text-xs text-gray-500 mb-1 block">Email</label>
                  <input
                    type="email"
                    value={form.chef_email}
                    onChange={e => setField('chef_email', e.target.value)}
                    placeholder="you@email.com"
                    className="input-field w-full text-sm"
                  />
                </div>
                <div>
                  <label className="text-xs text-gray-500 mb-1 block">Phone</label>
                  <input
                    type="tel"
                    value={form.chef_phone}
                    onChange={e => setField('chef_phone', e.target.value)}
                    placeholder="(555) 000-0000"
                    className="input-field w-full text-sm"
                  />
                </div>
              </div>
            </div>
          </div>

          {/* Line Items */}
          <div className="card">
            <div className="section-header mb-3">Line Items</div>

            {/* Column headers */}
            <div className="grid text-xs text-gray-400 font-medium pb-1 border-b border-gray-100 mb-2"
                 style={{ gridTemplateColumns: '1fr 52px 80px 80px 32px' }}>
              <span>Description</span>
              <span className="text-center">Qty</span>
              <span className="text-right">Price</span>
              <span className="text-right">Amount</span>
              <span />
            </div>

            {form.line_items.map((li, idx) => (
              <div key={li._key} className="grid gap-x-2 gap-y-1 mb-3 items-start"
                   style={{ gridTemplateColumns: '1fr 52px 80px 80px 32px' }}>
                <textarea
                  value={li.description}
                  onChange={e => updateLine(li._key, 'description', e.target.value)}
                  rows={2}
                  placeholder="Service description…"
                  className="input-field text-sm resize-none py-2 leading-snug"
                />
                <input
                  type="number"
                  value={li.quantity}
                  min="0"
                  step="0.5"
                  onChange={e => updateLine(li._key, 'quantity', e.target.value)}
                  className="input-field text-sm text-center px-1"
                />
                <input
                  type="number"
                  value={li.unit_price}
                  min="0"
                  step="0.01"
                  onChange={e => updateLine(li._key, 'unit_price', e.target.value)}
                  className="input-field text-sm text-right px-2"
                />
                <div className="input-field text-sm text-right px-2 flex items-center justify-end text-navy font-medium bg-warm-white">
                  {fmt$(li.amount)}
                </div>
                <button
                  onClick={() => removeLine(li._key)}
                  className="text-gray-300 hover:text-red-400 transition-colors text-lg leading-none mt-2"
                  title="Remove line"
                >
                  ×
                </button>
              </div>
            ))}

            {/* Add line */}
            <div className="relative mt-2" ref={presetsRef}>
              <button
                onClick={() => setShowPresets(v => !v)}
                className="w-full py-2.5 border-2 border-dashed border-gray-200 rounded-xl text-sm text-gray-400 hover:border-green-mid hover:text-green-dark transition-colors flex items-center justify-center gap-2"
              >
                <span className="text-lg leading-none">+</span> Add Line Item
              </button>

              {showPresets && (
                <div className="absolute left-0 right-0 top-full mt-1 bg-white border border-gray-200 rounded-xl shadow-lg z-30 overflow-hidden">
                  <div className="p-2 border-b border-gray-100">
                    <button
                      onClick={() => addLine()}
                      className="w-full text-left px-3 py-2 text-sm text-navy font-medium hover:bg-blue-light rounded-lg transition-colors"
                    >
                      ✏️ Blank line item
                    </button>
                  </div>
                  <div className="p-2 max-h-60 overflow-y-auto">
                    <div className="text-xs text-gray-400 px-3 pb-1">Quick presets</div>
                    {LINE_ITEM_PRESETS.map(preset => (
                      <button
                        key={preset.description}
                        onClick={() => addLine(preset)}
                        className="w-full text-left px-3 py-2 text-sm text-gray-700 hover:bg-blue-light rounded-lg transition-colors"
                      >
                        {preset.description}
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Totals */}
            <div className="mt-4 pt-3 border-t border-gray-100 space-y-2">
              <div className="flex justify-between text-sm text-gray-600">
                <span>Subtotal</span>
                <span className="font-medium">{fmt$(subtotal)}</span>
              </div>
              <div className="flex items-center justify-between text-sm text-gray-600">
                <div className="flex items-center gap-2">
                  <span>Tax</span>
                  <input
                    type="number"
                    value={form.tax_rate}
                    min="0"
                    max="100"
                    step="0.5"
                    onChange={e => setField('tax_rate', e.target.value)}
                    className="w-14 text-center text-xs border border-gray-200 rounded-lg py-1 px-1 focus:outline-hidden focus:ring-1 focus:ring-green-mid"
                  />
                  <span>%</span>
                </div>
                <span className="font-medium">{fmt$(taxAmount)}</span>
              </div>
              <div className="flex justify-between text-base font-bold text-navy pt-1 border-t border-gray-200">
                <span>Total</span>
                <span>{fmt$(total)}</span>
              </div>
            </div>
          </div>

          {/* Message to client */}
          <div className="card">
            <div className="section-header mb-3">Message to Client</div>
            <textarea
              value={form.message}
              onChange={e => setField('message', e.target.value)}
              placeholder="Thank you! Payment is due within 14 days."
              rows={3}
              className="input-field w-full text-sm resize-none"
            />
          </div>

          {/* Internal notes */}
          <div className="card">
            <div className="section-header mb-3">Internal Notes <span className="text-gray-400 font-normal">(chef only, not shown to client)</span></div>
            <textarea
              value={form.internal_notes}
              onChange={e => setField('internal_notes', e.target.value)}
              placeholder="Notes for your records…"
              rows={2}
              className="input-field w-full text-sm resize-none"
            />
          </div>
        </div>

        {/* ══ RIGHT: CLIENT-FACING PREVIEW ═══════════════════════════════════ */}
        <div className="invoice-preview-panel">
          <div className="sticky top-4">
            <div className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">
              Client Preview
            </div>
            <InvoicePreview form={form} subtotal={subtotal} taxAmount={taxAmount} total={total} logoBase64={logoBase64} />
          </div>
        </div>

      </div>

      {/* ── Fixed action bar ──────────────────────────────────────────────── */}
      <div className="fixed fixed-action-bar left-0 right-0 bg-white border-t border-gray-100 px-4 py-3 z-20 md:left-[220px] invoice-no-print">
        <div className="max-w-5xl mx-auto flex gap-2">
          <button
            onClick={() => handleSave()}
            disabled={saving || emailing}
            className="flex-1 py-3.5 bg-navy text-white rounded-xl font-semibold text-sm disabled:opacity-60"
          >
            {saving ? 'Saving…' : '💾 Save'}
          </button>
          {(form.status === 'draft' || form.status === 'sent') && (
            <button
              onClick={handleEmail}
              disabled={saving || emailing}
              title={form.client_email ? `Email to ${form.client_email}` : 'Add a client email first'}
              className="flex-1 py-3.5 bg-blue-600 text-white rounded-xl font-semibold text-sm disabled:opacity-60"
            >
              {emailing ? 'Sending…' : '📧 Email'}
            </button>
          )}
          {(form.status === 'sent' || form.status === 'draft') && (
            <button
              onClick={handleMarkPaid}
              disabled={saving || emailing}
              className="flex-1 py-3.5 bg-green-dark text-white rounded-xl font-semibold text-sm disabled:opacity-60"
            >
              ✅ Paid
            </button>
          )}
          {!isNew && (
            <button
              onClick={copyClientLink}
              disabled={saving || emailing}
              title="Copy the client's invoice link: they can view it there and, when online payments are on, pay it"
              aria-label="Copy the client's invoice link"
              className="px-4 py-3.5 bg-warm-white border border-gray-200 text-navy rounded-xl font-semibold text-sm hover:bg-blue-light transition-colors disabled:opacity-60"
            >
              🔗
            </button>
          )}
          <button
            onClick={handlePrint}
            disabled={saving || emailing}
            title="Download as PDF"
            className="px-4 py-3.5 bg-warm-white border border-gray-200 text-navy rounded-xl font-semibold text-sm hover:bg-blue-light transition-colors disabled:opacity-60"
          >
            ⬇️
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Invoice Preview ────────────────────────────────────────────────────────────
interface InvoicePreviewProps {
  form: InvoiceForm;
  subtotal: number;
  taxAmount: number;
  total: number;
  logoBase64: string | null;
}

function InvoicePreview({ form, subtotal, taxAmount, total, logoBase64 }: InvoicePreviewProps) {
  const fmt$ = (v: unknown) => `$${(parseNum(v) || 0).toFixed(2)}`;

  return (
    <div className="invoice-preview-card bg-white rounded-2xl shadow-lg overflow-hidden border border-gray-100">
      {/* Header band */}
      <div className="invoice-preview-header px-6 pt-6 pb-5" style={{ background: 'linear-gradient(135deg, #1B2B4B 0%, #2D6A4F 100%)' }}>
        {/* Logo */}
        <div className="flex items-start justify-between mb-5">
          <div className="w-20 h-20 rounded-xl border-2 border-white/20 flex items-center justify-center overflow-hidden bg-white/10 shrink-0">
            {logoBase64 ? (
              <img
                src={logoBase64}
                alt="Business logo"
                className="w-full h-full object-contain"
              />
            ) : (
              <div className="flex flex-col items-center text-white/50">
                <span className="text-2xl">🍽️</span>
                <span className="text-[10px] mt-0.5 font-medium">Add logo</span>
              </div>
            )}
          </div>
          <div className="text-right">
            <div className="text-white text-xl font-bold">{form.business_name || 'Your Business'}</div>
            {form.chef_name    && <div className="text-white/80 text-xs mt-0.5">{form.chef_name}</div>}
            {form.chef_email   && <div className="text-white/70 text-xs">{form.chef_email}</div>}
            {form.chef_phone   && <div className="text-white/70 text-xs">{form.chef_phone}</div>}
          </div>
        </div>

        {/* Invoice meta */}
        <div className="flex items-end justify-between">
          <div>
            <div className="text-white/60 text-xs uppercase tracking-widest font-semibold">Invoice</div>
            <div className="text-white text-2xl font-bold mt-0.5">
              {form.invoice_number || 'DRAFT'}
            </div>
          </div>
          <div className="text-right text-xs text-white/70 space-y-0.5">
            {form.issue_date && <div>Issued: <span className="text-white font-medium">{fmtDate(form.issue_date)}</span></div>}
            {form.due_date   && <div>Due: <span className="text-white font-medium">{fmtDate(form.due_date)}</span></div>}
          </div>
        </div>
      </div>

      <div className="px-6 py-5 space-y-5">
        {/* Bill to */}
        {(form.client_name || form.client_email || form.client_phone || form.client_address) && (
          <div>
            <div className="text-xs text-gray-400 uppercase tracking-wide font-semibold mb-1">Bill To</div>
            {form.client_name    && <div className="text-sm font-semibold text-navy">{form.client_name}</div>}
            {form.client_email   && <div className="text-xs text-gray-500">{form.client_email}</div>}
            {form.client_phone   && <div className="text-xs text-gray-500">{form.client_phone}</div>}
            {form.client_address && (
              <div className="text-xs text-gray-500 whitespace-pre-line">{form.client_address}</div>
            )}
          </div>
        )}

        {/* Line items */}
        <div>
          <div className="grid text-xs text-gray-400 font-semibold uppercase tracking-wide pb-1.5 border-b border-gray-100"
               style={{ gridTemplateColumns: '1fr 40px 70px 70px' }}>
            <span>Description</span>
            <span className="text-center">Qty</span>
            <span className="text-right">Price</span>
            <span className="text-right">Amount</span>
          </div>

          {form.line_items.filter(li => li.description || li.amount).map((li, i) => (
            <div key={li._key || i}
                 className="grid py-2.5 border-b border-gray-50 text-sm gap-x-2"
                 style={{ gridTemplateColumns: '1fr 40px 70px 70px' }}>
              <span className="text-navy whitespace-pre-line leading-snug">{li.description || '—'}</span>
              <span className="text-center text-gray-500">{li.quantity}</span>
              <span className="text-right text-gray-500">{fmt$(li.unit_price)}</span>
              <span className="text-right font-medium text-navy">{fmt$(li.amount)}</span>
            </div>
          ))}
        </div>

        {/* Totals */}
        <div className="space-y-1.5">
          <div className="flex justify-between text-sm text-gray-500">
            <span>Subtotal</span>
            <span>{fmt$(subtotal)}</span>
          </div>
          {parseNum(form.tax_rate) > 0 && (
            <div className="flex justify-between text-sm text-gray-500">
              <span>Tax ({form.tax_rate}%)</span>
              <span>{fmt$(taxAmount)}</span>
            </div>
          )}
          <div className="flex justify-between text-base font-bold text-navy pt-2 border-t border-gray-200">
            <span>Total Due</span>
            <span className="text-green-dark">{fmt$(total)}</span>
          </div>
        </div>

        {/* Message */}
        {form.message && (
          <div className="bg-warm-white rounded-xl px-4 py-3">
            <div className="text-xs text-gray-400 font-semibold mb-1">Note</div>
            <p className="text-xs text-gray-600 leading-relaxed">{form.message}</p>
          </div>
        )}

        {/* Footer */}
        <div className="text-center pt-2 pb-1 border-t border-gray-100">
          <p className="text-xs text-gray-400">
            {form.business_name || 'Your Business'}
          </p>
        </div>
      </div>
    </div>
  );
}
