// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router';
import { format, addDays } from 'date-fns';
import { parseLocalDate, fmtSessionDate, fmtUtcTimestamp, localTodayStr } from '../utils/dateUtils';
import PageHeader from '../components/PageHeader';
import ConfirmModal from '../components/ConfirmModal';
import { useToast } from '../components/Toast';
import { apiCall, errorMessage } from '../hooks/useApi';
import { STATUS_COLORS, calcFeasibility, FEASIBILITY_COLORS, getReheatedInstructions, CATEGORY_BAR_COLORS, marginColor, REPEAT_OPTIONS } from '../constants';
import { downloadServerPdf, pdfFilename } from '../utils/serverPdf';
import { parseNum, type NumInput } from '../utils/numbers';
import type {
  CompletedSession, EmailSentResponse, Flag, GroceryItem, GroceryListResponse, InvoiceListItem, SessionDetail as SessionDetailData, SessionDish,
} from '../types';

/** uncheckAll passes a boolean is_checked; toggling only reads its truthiness. */
type GroceryToggle = Omit<GroceryItem, 'is_checked'> & { is_checked?: Flag | boolean };

interface TimelineStep {
  time: number;
  label: string;
  type: 'setup' | 'prep' | 'cook' | 'label' | 'cleanup';
  dish?: string;
  color?: string;
}

const STORE_SECTION_ORDER = ['Produce', 'Meat & Seafood', 'Dairy & Eggs', 'Pantry & Dry Goods', 'Canned & Jarred', 'Herbs & Spices', 'Other'];

export default function SessionDetail() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { show } = useToast();
  const [session, setSession] = useState<SessionDetailData | null>(null);
  const [invoice, setInvoice] = useState<InvoiceListItem | null>(null);   // session's invoice, if any
  const [groceryItems, setGroceryItems] = useState<GroceryItem[]>([]);
  const [groceryLoading, setGroceryLoading] = useState(false);
  const [collapsedSections, setCollapsedSections] = useState<Record<string, boolean>>({});
  const [tab, setTab] = useState('overview');
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [emailing, setEmailing] = useState(false);
  const [repeatOpen, setRepeatOpen] = useState(false);
  const [cloning, setCloning] = useState(false);
  const [showEmailPrompt, setShowEmailPrompt] = useState(false);
  const [editingNotes, setEditingNotes] = useState(false);
  const [notesDraft, setNotesDraft] = useState('');
  const [savingNotes, setSavingNotes] = useState(false);
  const [editingCosts, setEditingCosts] = useState(false);
  const [costDraft, setCostDraft] = useState<{ service_fee: NumInput; ingredient_cost: NumInput }>({ service_fee: '', ingredient_cost: '' });
  const [savingCosts, setSavingCosts] = useState(false);

  const load = useCallback(() => {
    fetch(`/api/sessions/${id}`)
      .then(r => r.ok ? r.json() : Promise.reject(new Error(`Request failed: ${r.status}`)))
      .then(d => setSession(d))
      .catch(() => {});
    // Load the most recent non-void invoice for this session
    fetch(`/api/invoices?session_id=${id}`)
      .then(r => r.ok ? r.json() : Promise.resolve([]))
      .then(data => {
        const list = Array.isArray(data) ? (data as InvoiceListItem[]).filter(i => i.status !== 'void') : [];
        setInvoice(list[0] || null);
      })
      .catch(() => {});
  }, [id]);

  useEffect(() => { load(); }, [load]);

  useEffect(() => {
    if (tab === 'grocery' && session) loadGrocery();
  }, [tab, session?.id]);

  const loadGrocery = async () => {
    setGroceryLoading(true);
    try {
      const data = await apiCall<GroceryListResponse>(`/api/sessions/${id}/grocery`);
      setGroceryItems(data.items || []);
    } catch {
      // Generate fresh
      try {
        const data = await apiCall<GroceryListResponse>(`/api/sessions/${id}/grocery/generate`, 'POST');
        setGroceryItems(data.items || []);
      } catch {
        show('Could not load grocery list', 'error');
      }
    } finally {
      setGroceryLoading(false);
    }
  };

  const toggleGroceryItem = async (item: GroceryToggle) => {
    try {
      const updated = await apiCall<GroceryItem>(`/api/grocery/${item.id}`, 'PUT', { is_checked: item.is_checked ? 0 : 1 });
      setGroceryItems(prev => prev.map(i => i.id === item.id ? { ...i, is_checked: updated.is_checked } : i));
    } catch {
      show('Could not update item', 'error');
    }
  };

  const uncheckAll = async () => {
    const checkedItems = groceryItems.filter(i => i.id && i.is_checked);
    for (const item of checkedItems) {
      await toggleGroceryItem({ ...item, is_checked: true });
    }
  };

  const handleMarkComplete = async () => {
    try {
      const updated = await apiCall<CompletedSession>(`/api/sessions/${id}/complete`, 'POST');
      setSession(updated);
      if (updated.next_session_id) {
        show(`Next session booked for ${fmtSessionDate(updated.next_session_date, 'EEE, MMM d')}`, 'success', 5000);
      }
      if (updated.client_email) {
        setShowEmailPrompt(true);
      } else if (!updated.next_session_id) {
        show('Session marked complete', 'success');
      }
    } catch {
      show('Could not update session', 'error');
    }
  };

  const handleRepeatChange = async (value: string) => {
    try {
      const updated = await apiCall<SessionDetailData>(`/api/sessions/${id}`, 'PUT', {
        repeat_every_weeks: value ? Number(value) : null,
      });
      setSession(updated);
      show(value ? 'Recurring — the next session is booked when this one is completed' : 'No longer recurring', 'success');
    } catch {
      show('Could not update the recurring setting', 'error');
    }
  };

  const handleDelete = async () => {
    try {
      await apiCall(`/api/sessions/${id}`, 'DELETE');
      show('Session deleted', 'info');
      navigate('/sessions');
    } catch (err) {
      if (errorMessage(err)?.includes('INVOICE_ATTACHED') || errorMessage(err)?.includes('invoice')) {
        show('Void the invoice before deleting this session', 'error', 5000);
      } else {
        show('Could not delete session', 'error');
      }
    }
  };

  const handleGeneratePDF = async () => {
    if (!session) return;
    setGenerating(true);
    try {
      await downloadServerPdf(`/api/sessions/${id}/pdf`, pdfFilename.mealPlan(session));
      show('PDF ready — tap to open', 'success');
    } catch {
      show('PDF could not be generated — try again', 'error');
    } finally {
      setGenerating(false);
    }
  };

  const handleEmailPDF = async () => {
    if (!session) return;
    const clientEmail = session.client_email;
    if (!clientEmail) {
      show('No email on file for this client — add one in the client profile first', 'error', 5000);
      return;
    }
    setEmailing(true);
    try {
      // The server renders and attaches the meal plan itself.
      const result = await apiCall<EmailSentResponse>(`/api/sessions/${id}/email-pdf`, 'POST');
      show(`Meal plan sent to ${result.sent_to}`, 'success', 4000);
    } catch (err) {
      if (errorMessage(err)?.includes('NO_EMAIL') || errorMessage(err)?.includes('no email')) {
        show('No email on file for this client', 'error', 4000);
      } else {
        show(errorMessage(err) || 'Could not send email — check server email settings', 'error', 5000);
      }
    } finally {
      setEmailing(false);
    }
  };

  const handleGroceryPDF = async () => {
    if (!session) return;
    try {
      await downloadServerPdf(`/api/sessions/${id}/grocery/pdf`, pdfFilename.grocery(session));
      show('PDF ready', 'success');
    } catch {
      show('PDF could not be generated', 'error');
    }
  };

  const handleClone = async (daysOffset: number) => {
    setCloning(true);
    try {
      const result = await apiCall<SessionDetailData>(`/api/sessions/${id}/clone`, 'POST', { days_offset: daysOffset });
      show(`Session cloned for ${result.session_date}`, 'success');
      navigate(`/sessions/${result.id}`);
    } catch {
      show('Could not clone session', 'error');
    } finally {
      setCloning(false);
      setRepeatOpen(false);
    }
  };

  const handleCopyGroceryList = () => {
    if (!session) return;
    if (groceryItems.length === 0) { show('Generate a grocery list first', 'error'); return; }
    const STORE_ORDER = ['Produce', 'Meat & Seafood', 'Dairy & Eggs', 'Pantry & Dry Goods', 'Canned & Jarred', 'Herbs & Spices', 'Other'];
    const lines = [`🛒 Grocery list for ${session.client_name || 'session'}`, ''];
    const grouped: Record<string, GroceryItem[]> = {};
    for (const item of groceryItems) {
      const sec = item.store_section || 'Other';
      if (!grouped[sec]) grouped[sec] = [];
      grouped[sec].push(item);
    }
    const order = [...STORE_ORDER, ...Object.keys(grouped).filter(k => !STORE_ORDER.includes(k))];
    for (const sec of order) {
      if (!grouped[sec]) continue;
      lines.push(`📍 ${sec}`);
      for (const item of grouped[sec]) {
        lines.push(`  • ${item.ingredient_name} — ${item.total_quantity}`);
      }
      lines.push('');
    }
    const text = lines.join('\n');
    const copy = () => navigator.clipboard.writeText(text)
      .then(() => show('Grocery list copied to clipboard', 'success'))
      .catch(() => show('Could not copy to clipboard', 'error'));
    // Phones get the share sheet (Messages, Notes, …); desktops copy.
    if (typeof navigator.share === 'function' && window.matchMedia?.('(pointer: coarse)').matches) {
      navigator.share({ title: 'Grocery list', text }).catch((e: unknown) => {
        if (!(e instanceof DOMException && e.name === 'AbortError')) copy();
      });
    } else {
      copy();
    }
  };

  const handleSaveCosts = async () => {
    setSavingCosts(true);
    try {
      const fee  = parseNum(costDraft.service_fee)    || 0;
      const cost = parseNum(costDraft.ingredient_cost) || 0;
      await apiCall(`/api/sessions/${id}`, 'PUT', { ...session, service_fee: fee, ingredient_cost: cost });
      setSession(prev => ({
        ...prev!,
        service_fee: fee,
        ingredient_cost: cost,
        gross_margin: fee - cost,
        margin_pct: fee > 0 ? parseFloat(((fee - cost) / fee * 100).toFixed(1)) : 0,
      }));
      setEditingCosts(false);
      show('Costs updated', 'success');
    } catch {
      show('Could not save costs', 'error');
    } finally {
      setSavingCosts(false);
    }
  };

  const handleSaveNotes = async () => {
    setSavingNotes(true);
    try {
      await apiCall(`/api/sessions/${id}`, 'PUT', { ...session, chef_notes: notesDraft });
      setSession(prev => ({ ...prev!, chef_notes: notesDraft }));
      setEditingNotes(false);
      show('Notes saved', 'success');
    } catch {
      show('Could not save notes', 'error');
    } finally {
      setSavingNotes(false);
    }
  };

  const regenerateGrocery = async () => {
    setGroceryLoading(true);
    try {
      const data = await apiCall<GroceryListResponse>(`/api/sessions/${id}/grocery/generate`, 'POST');
      setGroceryItems(data.items || []);
      show('Grocery list regenerated', 'success');
    } catch {
      show('Could not regenerate', 'error');
    } finally {
      setGroceryLoading(false);
    }
  };

  // Prep timeline generation
  const generateTimeline = (dishes: SessionDish[]): { steps: TimelineStep[]; ovenConflicts: string[] } => {
    if (!dishes || dishes.length === 0) return { steps: [], ovenConflicts: [] };
    const sorted = [...dishes].sort((a, b) => (b.cook_time_min || 0) - (a.cook_time_min || 0));
    const steps: TimelineStep[] = [{ time: 0, label: 'Setup: review kitchen, organize stations, check pantry, preheat oven', type: 'setup' }];
    let cursor = 15;
    const ovenConflicts: string[] = [];
    const activeWindows: Array<{ start: number; end: number; name: string }> = [];

    for (const dish of sorted) {
      const prep = dish.prep_time_min || 0;
      const cook = dish.cook_time_min || 0;
      const cookStart = cursor + prep;
      const cookEnd = cookStart + cook;

      // Check oven conflict
      for (const w of activeWindows) {
        if (cookStart < w.end && cookEnd > w.start) {
          ovenConflicts.push(`${dish.name} and ${w.name} overlap. Stagger start or use two racks.`);
        }
      }
      activeWindows.push({ start: cookStart, end: cookEnd, name: dish.name });

      const hStart = Math.floor(cursor / 60);
      const mStart = cursor % 60;
      steps.push({
        time: cursor,
        label: `${hStart}:${String(mStart).padStart(2, '0')} — Begin ${dish.name} prep (${prep} min active)`,
        type: 'prep',
        dish: dish.name,
        color: CATEGORY_BAR_COLORS[dish.category ?? ''] || '#6b7280',
      });
      steps.push({
        time: cookStart,
        label: `${Math.floor(cookStart / 60)}:${String(cookStart % 60).padStart(2, '0')} — ${dish.name} in oven/stovetop (${cook} min passive)`,
        type: 'cook',
        dish: dish.name,
      });
      cursor = cookStart;
    }

    const lastEnd = Math.max(...sorted.map(d => (d.prep_time_min || 0) + (d.cook_time_min || 0))) + 15;
    steps.push({
      time: lastEnd,
      label: `${Math.floor(lastEnd / 60)}:${String(lastEnd % 60).padStart(2, '0')} — Begin labeling and portioning as dishes finish`,
      type: 'label',
    });
    steps.push({
      time: lastEnd + 15,
      label: `${Math.floor((lastEnd + 15) / 60)}:${String((lastEnd + 15) % 60).padStart(2, '0')} — Cleanup, pack containers, client handoff`,
      type: 'cleanup',
    });

    return { steps, ovenConflicts };
  };

  if (!session) {
    return (
      <div className="page">
        <PageHeader title="Session" back="/sessions" />
        <div className="px-4 pt-4 space-y-3">
          <div className="skeleton h-6 w-2/3" /><div className="skeleton h-4 w-full" /><div className="skeleton h-4 w-1/2" />
        </div>
      </div>
    );
  }

  const feasibility = calcFeasibility(session.dishes || []);
  const useByDate = session.session_date
    ? fmtSessionDate(
        new Date(parseLocalDate(session.session_date).getTime() + 4 * 86400000).toISOString().slice(0, 10),
        'MMMM d, yyyy', '')
    : '';

  const grouped = STORE_SECTION_ORDER.reduce<Record<string, GroceryItem[]>>((acc, section) => {
    const items = groceryItems.filter(i => i.store_section === section);
    if (items.length > 0) acc[section] = items;
    return acc;
  }, {});
  const otherItems = groceryItems.filter(i => !STORE_SECTION_ORDER.includes(i.store_section));
  if (otherItems.length > 0) grouped['Other'] = [...(grouped['Other'] || []), ...otherItems];

  const { steps: timelineSteps, ovenConflicts } = generateTimeline(session.dishes || []);

  return (
    <div className="page">
      <PageHeader
        title={session.client_name || 'Session'}
        back="/sessions"
        subtitle={session.session_date ? fmtSessionDate(session.session_date, 'EEEE, MMMM d') : ''}
        action={
          <span className={`text-xs px-2 py-1 rounded-full font-medium ${STATUS_COLORS[session.status] || 'bg-gray-100'}`}>
            {session.status}
          </span>
        }
      />

      {/* Stat row */}
      <div className="grid grid-cols-4 gap-0 bg-white border-b border-gray-100">
        {[
          ['Serves', session.serving_size],
          ['Dishes', (session.dishes || []).length],
          ['~Time', `${Math.round(feasibility.estimated_min)}m`],
          ['Margin', `${(session.margin_pct || 0).toFixed(0)}%`],
        ].map(([label, val], i) => (
          <div key={i} className="text-center py-3 border-r border-gray-100 last:border-0">
            <div className="text-[10px] text-gray-500">{label}</div>
            <div className="text-sm font-semibold text-navy">{val}</div>
          </div>
        ))}
      </div>

      {/* Feasibility badge */}
      <div className="flex items-center justify-center py-2 border-b border-gray-100">
        <span className={`text-xs px-3 py-1 rounded-full font-semibold ${FEASIBILITY_COLORS[feasibility.level]}`}>
          {feasibility.level} — {(feasibility.estimated_min || 0) > 60 ? `~${((feasibility.estimated_min || 0) / 60).toFixed(1)}h` : `${feasibility.estimated_min || 0}min`}
        </span>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-gray-200 bg-white sticky top-[130px] z-10">
        {['overview', 'grocery', 'timeline'].map(t => (
          <button key={t} onClick={() => setTab(t)}
            className={`flex-1 py-3 text-xs font-medium capitalize ${tab === t ? 'text-green-dark border-b-2 border-green-dark' : 'text-gray-500'}`}
            style={{ minHeight: '44px' }}
          >
            {t.charAt(0).toUpperCase() + t.slice(1)}
          </button>
        ))}
      </div>

      <div className="px-4 pt-4 pb-36">
        {/* OVERVIEW TAB */}
        {tab === 'overview' && (
          <div className="space-y-4">
            <div className="card">
              <div className="flex items-center justify-between mb-2">
                <div className="section-header mb-0">Session Info</div>
                {!editingCosts && (
                  <button
                    onClick={() => { setCostDraft({ service_fee: session.service_fee || '', ingredient_cost: session.ingredient_cost || '' }); setEditingCosts(true); }}
                    className="text-xs text-gray-400 hover:text-navy flex items-center gap-1"
                    style={{ minHeight: 'auto' }}
                  >
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/>
                      <path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/>
                    </svg>
                    Edit costs
                  </button>
                )}
              </div>
              <div className="space-y-2">
                <div className="flex justify-between text-sm"><span className="text-gray-500">Date</span><span>{session.session_date}</span></div>
                <div className="flex justify-between text-sm"><span className="text-gray-500">Time</span><span>{session.session_time || '—'}</span></div>
                <div className="flex justify-between text-sm"><span className="text-gray-500">Window</span><span>{session.session_window_hours}h</span></div>
                <div className="flex justify-between items-center text-sm">
                  <label htmlFor="repeat" className="text-gray-500">Recurring</label>
                  <select id="repeat" value={session.repeat_every_weeks ?? ''} onChange={e => handleRepeatChange(e.target.value)}
                    className="w-auto py-1 pl-2 pr-7 text-sm text-right border-gray-200 rounded-lg" style={{ minHeight: 'auto' }}>
                    {REPEAT_OPTIONS.map(([w, label]) => <option key={label} value={w ?? ''}>{label}</option>)}
                  </select>
                </div>
                {(session.client_reminder_sent_at || session.chef_reminder_sent_at) && (
                  <div className="text-xs text-gray-400">
                    ⏰ Reminder {session.client_reminder_sent_at ? `emailed to client ${fmtUtcTimestamp(session.client_reminder_sent_at, 'MMM d, h:mm a')}` : `prep sheet sent ${fmtUtcTimestamp(session.chef_reminder_sent_at, 'MMM d, h:mm a')}`}
                  </div>
                )}
                {session.repeat_of && (
                  <div className="text-xs text-gray-400">
                    Booked automatically from <button className="underline" style={{ minHeight: 'auto' }} onClick={() => navigate(`/sessions/${session.repeat_of}`)}>the previous session</button>
                  </div>
                )}

                {editingCosts ? (
                  <>
                    <div className="flex items-center justify-between text-sm gap-2">
                      <span className="text-gray-500 shrink-0">Service Fee</span>
                      <div className="relative w-28">
                        <span className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400 text-xs">$</span>
                        <input
                          type="number" min="0" step="10"
                          value={costDraft.service_fee}
                          onChange={e => setCostDraft(p => ({ ...p, service_fee: e.target.value }))}
                          className="pl-5 pr-2 py-1.5 w-full text-sm text-right rounded-lg border border-gray-200 focus:outline-hidden focus:ring-2 focus:ring-green-mid"
                        />
                      </div>
                    </div>
                    <div className="flex items-center justify-between text-sm gap-2">
                      <span className="text-gray-500 shrink-0">Ingredient Cost</span>
                      <div className="relative w-28">
                        <span className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400 text-xs">$</span>
                        <input
                          type="number" min="0" step="5"
                          value={costDraft.ingredient_cost}
                          onChange={e => setCostDraft(p => ({ ...p, ingredient_cost: e.target.value }))}
                          className="pl-5 pr-2 py-1.5 w-full text-sm text-right rounded-lg border border-gray-200 focus:outline-hidden focus:ring-2 focus:ring-green-mid"
                        />
                      </div>
                    </div>
                    <div className="flex gap-2 pt-1">
                      <button
                        onClick={handleSaveCosts}
                        disabled={savingCosts}
                        className="flex-1 py-2 bg-navy text-white rounded-xl text-sm font-semibold disabled:opacity-60"
                        style={{ minHeight: '40px' }}
                      >
                        {savingCosts ? 'Saving…' : 'Save'}
                      </button>
                      <button
                        onClick={() => setEditingCosts(false)}
                        className="px-4 py-2 border border-gray-200 text-gray-600 rounded-xl text-sm"
                        style={{ minHeight: '40px' }}
                      >
                        Cancel
                      </button>
                    </div>
                  </>
                ) : (
                  <>
                    <div className="flex justify-between text-sm"><span className="text-gray-500">Service Fee</span><span className="font-semibold">${(session.service_fee || 0).toFixed(2)}</span></div>
                    <div className="flex justify-between text-sm"><span className="text-gray-500">Ingredient Cost</span><span>${(session.ingredient_cost || 0).toFixed(2)}</span></div>
                    <div className="flex justify-between text-sm">
                      <span className="text-gray-500">Gross Margin</span>
                      <span className={`font-bold ${marginColor(session.margin_pct)}`}>
                        ${(session.gross_margin || 0).toFixed(2)} ({(session.margin_pct || 0).toFixed(1)}%)
                      </span>
                    </div>
                  </>
                )}
              </div>
            </div>

            {/* Invoice status card */}
            {(() => {
              const INV_CFG: Record<string, { label: string; bg: string; text: string }> = {
                draft: { label: 'Draft',  bg: 'bg-gray-100',   text: 'text-gray-600'  },
                sent:  { label: 'Sent',   bg: 'bg-blue-100',   text: 'text-blue-700'  },
                paid:  { label: 'Paid',   bg: 'bg-green-100',  text: 'text-green-700' },
                void:  { label: 'Void',   bg: 'bg-red-100',    text: 'text-red-600'   },
              };
              const today = localTodayStr();
              const overdue = invoice && invoice.status !== 'paid' && invoice.due_date && invoice.due_date < today;
              const cfg = INV_CFG[invoice?.status ?? ''] || INV_CFG.draft;

              return (
                <div className="card">
                  <div className="flex items-center justify-between">
                    <div className="section-header mb-0">Invoice</div>
                    {invoice ? (
                      <span className={`text-xs px-2 py-0.5 rounded-full font-semibold ${cfg.bg} ${cfg.text}`}>
                        {cfg.label}
                      </span>
                    ) : (
                      <span className="text-xs text-gray-400">None yet</span>
                    )}
                  </div>

                  {invoice ? (
                    <div className="mt-2 space-y-1">
                      <div className="flex justify-between text-sm">
                        <span className="text-gray-500">{invoice.invoice_number}</span>
                        <span className="font-semibold text-navy">${(invoice.total || 0).toFixed(2)}</span>
                      </div>
                      {invoice.status === 'paid' && invoice.paid_at && (
                        <div className="text-xs text-green-600 font-medium">
                          ✓ Paid {(() => { try { return format(parseLocalDate(invoice.paid_at), 'MMM d, yyyy'); } catch { return ''; } })()}
                        </div>
                      )}
                      {overdue && (
                        <div className="text-xs text-danger font-medium">⚠ Overdue since {invoice.due_date}</div>
                      )}
                      {!overdue && invoice.due_date && invoice.status !== 'paid' && (
                        <div className="text-xs text-gray-400">Due {invoice.due_date}</div>
                      )}
                      <button
                        onClick={() => navigate(`/invoices/${invoice.id}`)}
                        className="w-full mt-2 py-2 text-xs font-semibold text-navy border border-gray-200 rounded-xl hover:bg-blue-light transition-colors"
                        style={{ minHeight: 'auto' }}
                      >
                        View / Edit Invoice →
                      </button>
                    </div>
                  ) : (
                    <p className="text-xs text-gray-400 mt-1">No invoice created for this session yet.</p>
                  )}
                </div>
              );
            })()}

            <div className="card">
              <div className="section-header">Menu</div>
              {(session.dishes || []).length === 0 ? (
                <p className="text-sm text-gray-400">No dishes added</p>
              ) : (
                <div className="space-y-2">
                  {session.dishes.map(d => (
                    <div key={d.id} className="flex items-center justify-between py-2 border-b border-gray-100 last:border-0">
                      <div>
                        <div className="text-sm font-medium text-navy">{d.name}</div>
                        <div className="text-xs text-gray-500">{d.category} · {(d.prep_time_min || 0) + (d.cook_time_min || 0)} min</div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Chef Notes — always visible, inline-editable */}
            <div className="card">
              <div className="flex items-center justify-between mb-2">
                <div className="section-header mb-0">Chef Notes</div>
                {!editingNotes && (
                  <button
                    onClick={() => { setNotesDraft(session.chef_notes || ''); setEditingNotes(true); }}
                    className="text-xs text-gray-400 hover:text-navy flex items-center gap-1"
                    style={{ minHeight: 'auto' }}
                  >
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/>
                      <path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/>
                    </svg>
                    {session.chef_notes ? 'Edit' : 'Add'}
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
                    placeholder="Session-specific reminders, client preferences, access notes…"
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
              ) : session.chef_notes ? (
                <p className="text-sm text-gray-700 whitespace-pre-wrap">{session.chef_notes}</p>
              ) : (
                <p className="text-sm text-gray-400 italic">No notes for this session</p>
              )}
            </div>

            {session.status !== 'Completed' && session.status !== 'Cancelled' && (
              <button onClick={handleMarkComplete} className="w-full py-3 border-2 border-green-mid text-green-dark rounded-xl font-semibold text-sm">
                ✓ Mark as Completed
              </button>
            )}

            <button onClick={() => setDeleteOpen(true)} className="w-full py-3 border border-danger text-danger rounded-xl font-medium text-sm">
              Delete Session
            </button>
          </div>
        )}

        {/* GROCERY TAB */}
        {tab === 'grocery' && (
          <div>
            <div className="flex gap-2 mb-4">
              <button onClick={regenerateGrocery} className="flex-1 py-2.5 border border-gray-200 text-gray-600 rounded-xl text-xs font-medium">
                ↺ Regenerate
              </button>
              <button onClick={uncheckAll} className="flex-1 py-2.5 border border-gray-200 text-gray-600 rounded-xl text-xs font-medium">
                Uncheck All
              </button>
              <button onClick={handleCopyGroceryList} className="flex-1 py-2.5 border border-gray-200 text-gray-600 rounded-xl text-xs font-medium">
                📋 Copy / Share
              </button>
              <button onClick={handleGroceryPDF} className="flex-1 py-2.5 bg-navy text-white rounded-xl text-xs font-medium">
                PDF
              </button>
            </div>

            {groceryLoading ? (
              <div className="space-y-2">{[1,2,3,4].map(i => <div key={i} className="skeleton h-12 rounded-xl" />)}</div>
            ) : Object.keys(grouped).length === 0 ? (
              <div className="card text-center py-8">
                <div className="text-gray-400 text-sm">No grocery items — tap Regenerate</div>
              </div>
            ) : Object.entries(grouped).map(([section, items]) => {
              const collapsed = collapsedSections[section];
              const checkedCount = items.filter(i => i.is_checked).length;
              return (
                <div key={section} className="mb-4">
                  <div className="flex items-center justify-between mb-2">
                    <button
                      onClick={() => setCollapsedSections(prev => ({ ...prev, [section]: !prev[section] }))}
                      className="flex items-center gap-2 font-display font-semibold text-green-dark text-sm"
                      style={{ minHeight: 'auto' }}
                    >
                      <span>{collapsed ? '▶' : '▼'}</span>
                      {section} <span className="text-xs font-sans font-normal text-gray-400">({checkedCount}/{items.length})</span>
                    </button>
                    <button
                      onClick={() => { const allChecked = items.every(i => i.is_checked); items.forEach(i => { if (i.id && i.is_checked === (allChecked ? 1 : 0)) toggleGroceryItem(i); }); }}
                      className="text-xs text-navy-mid font-medium"
                      style={{ minHeight: 'auto' }}
                    >
                      Mark All
                    </button>
                  </div>
                  {!collapsed && items.map((item, idx) => {
                    const sourceList = Array.isArray(item.source_dishes) ? item.source_dishes : (() => { try { return JSON.parse(item.source_dishes || '[]'); } catch { return []; } })();
                    return (
                      <div
                        key={item.id || idx}
                        className={`flex items-start gap-3 py-2.5 px-3 mb-1.5 rounded-xl cursor-pointer transition-colors ${item.is_checked ? 'bg-gray-50' : 'bg-white border border-gray-100'}`}
                        onClick={() => item.id && toggleGroceryItem(item)}
                      >
                        <div className={`w-5 h-5 rounded border-2 shrink-0 mt-0.5 flex items-center justify-center ${item.is_checked ? 'bg-green-mid border-green-mid' : 'border-gray-300'}`}>
                          {item.is_checked ? <span className="text-white text-xs">✓</span> : null}
                        </div>
                        <div className="flex-1">
                          <div className={`text-sm font-medium ${item.is_checked ? 'line-through text-gray-400' : 'text-navy'}`}>
                            {item.ingredient_name} — <span className="font-semibold">{item.total_quantity}</span>
                          </div>
                          {sourceList.length > 0 && (
                            <div className="text-xs text-gray-400 mt-0.5">{sourceList.join(' · ')}</div>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
        )}

        {/* TIMELINE TAB */}
        {tab === 'timeline' && (
          <div>
            {ovenConflicts.length > 0 && (
              <div className="mb-4 p-3 bg-amber-50 border border-amber-200 rounded-xl">
                <div className="text-xs font-semibold text-amber-800 mb-1">⚠ Oven Conflicts</div>
                {ovenConflicts.map((c, i) => <div key={i} className="text-xs text-amber-700">{c}</div>)}
              </div>
            )}

            <div className="card mb-4">
              <div className="section-header">Session Timeline</div>
              <div className="space-y-1">
                {timelineSteps.map((step, i) => (
                  <div
                    key={i}
                    className={`flex items-start gap-3 py-2.5 px-3 rounded-xl cursor-pointer transition-colors ${
                      step.type === 'setup' || step.type === 'cleanup' || step.type === 'label'
                        ? 'bg-gray-50'
                        : step.type === 'prep' ? 'bg-blue-light'
                        : 'bg-warm-white'
                    }`}
                  >
                    <div className={`w-2 h-2 rounded-full shrink-0 mt-2 ${
                      step.type === 'prep' ? 'bg-navy' :
                      step.type === 'cook' ? 'bg-green-mid' :
                      'bg-gray-400'
                    }`} />
                    <p className="text-sm text-gray-700">{step.label}</p>
                  </div>
                ))}
              </div>
            </div>

            <div className="section-header">Estimated Total: ~{Math.round(feasibility.estimated_min)} min</div>
          </div>
        )}
      </div>

      {/* Fixed bottom actions */}
      <div className="fixed fixed-action-bar left-0 right-0 bg-white border-t border-gray-100 px-4 py-3 z-20 md:left-[220px]">
        <div className="max-w-3xl mx-auto flex gap-2">
          <button
            onClick={handleGeneratePDF}
            disabled={generating || emailing}
            className="flex-1 py-3.5 bg-navy text-white rounded-xl font-semibold text-sm disabled:opacity-60"
          >
            {generating ? 'Generating...' : '📄 PDF'}
          </button>
          <button
            onClick={handleEmailPDF}
            disabled={emailing || generating || !session?.client_email}
            className="flex-1 py-3.5 bg-green-dark text-white rounded-xl font-semibold text-sm disabled:opacity-60"
            title={session?.client_email ? `Send to ${session.client_email}` : 'Add email to client profile first'}
          >
            {emailing ? (
              <span className="flex items-center justify-center gap-1.5">
                <span className="animate-spin inline-block">⟳</span>
              </span>
            ) : '✉️ Email'}
          </button>
          <button
            onClick={() => setRepeatOpen(true)}
            disabled={cloning}
            className="flex-1 py-3.5 bg-warm-white border border-gray-200 text-navy rounded-xl font-semibold text-sm hover:bg-blue-light transition-colors disabled:opacity-60"
            title="Clone this session to a future date"
          >
            🔁 Repeat
          </button>
          <button
            onClick={() => invoice
              ? navigate(`/invoices/${invoice.id}`)
              : navigate(`/invoices/new?session_id=${id}`)
            }
            className={`px-4 py-3.5 rounded-xl font-semibold text-sm transition-colors ${
              invoice
                ? 'bg-green-light text-green-dark border border-green-mid hover:bg-green-mid hover:text-white'
                : 'bg-warm-white border border-gray-200 text-navy hover:bg-blue-light'
            }`}
            title={invoice ? `View invoice ${invoice.invoice_number}` : 'Create invoice for this session'}
          >
            🧾
          </button>
        </div>
        {session?.pdf_emailed_at && (
          <div className="text-center text-xs text-gray-400 mt-1.5">
            Last sent {fmtUtcTimestamp(session.pdf_emailed_at, "MMM d 'at' h:mm a")}
          </div>
        )}
      </div>

      <ConfirmModal
        open={deleteOpen}
        title="Delete Session"
        message="This will permanently delete this session and all associated data."
        onConfirm={handleDelete}
        onCancel={() => setDeleteOpen(false)}
      />

      {/* Email prompt — shown after marking complete when client has an email */}
      {showEmailPrompt && (
        <div className="overlay" onClick={() => { setShowEmailPrompt(false); show('Session marked complete', 'success'); }}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <div className="text-2xl mb-2 text-center">✅</div>
            <div className="font-display font-semibold text-navy text-lg mb-1 text-center">Session Complete!</div>
            <p className="text-sm text-gray-500 mb-5 text-center">
              Send the meal plan PDF to{' '}
              <span className="font-medium text-navy">{session.client_name}</span>?
            </p>
            <button
              onClick={async () => {
                setShowEmailPrompt(false);
                await handleEmailPDF();
              }}
              disabled={emailing}
              className="w-full py-3.5 bg-green-dark text-white rounded-xl font-semibold text-sm mb-2 disabled:opacity-60"
            >
              {emailing ? 'Sending…' : `✉️ Send to ${session.client_email}`}
            </button>
            <button
              onClick={() => { setShowEmailPrompt(false); show('Session marked complete', 'success'); }}
              className="w-full py-3 text-sm text-gray-500 font-medium"
              style={{ minHeight: 'auto' }}
            >
              Skip for now
            </button>
          </div>
        </div>
      )}

      {/* Repeat Session modal */}
      {repeatOpen && (
        <div className="overlay" onClick={() => setRepeatOpen(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <div className="font-display font-semibold text-navy text-lg mb-1">🔁 Repeat Session</div>
            <p className="text-sm text-gray-500 mb-5">
              Clone this session with the same client, menu, and costs — shifted forward by:
            </p>
            <div className="space-y-2">
              {[
                { label: '+1 week', days: 7,  sub: session.session_date ? (() => { try { const d = new Date(session.session_date + 'T12:00:00'); d.setDate(d.getDate() + 7); return format(d, 'EEEE, MMM d'); } catch { return ''; } })() : '' },
                { label: '+2 weeks', days: 14, sub: session.session_date ? (() => { try { const d = new Date(session.session_date + 'T12:00:00'); d.setDate(d.getDate() + 14); return format(d, 'EEEE, MMM d'); } catch { return ''; } })() : '' },
                { label: '+4 weeks', days: 28, sub: session.session_date ? (() => { try { const d = new Date(session.session_date + 'T12:00:00'); d.setDate(d.getDate() + 28); return format(d, 'EEEE, MMM d'); } catch { return ''; } })() : '' },
              ].map(opt => (
                <button
                  key={opt.days}
                  onClick={() => handleClone(opt.days)}
                  disabled={cloning}
                  className="w-full flex items-center justify-between px-4 py-3.5 bg-warm-white border border-gray-200 rounded-xl hover:border-navy hover:bg-blue-light transition-colors disabled:opacity-50"
                >
                  <span className="font-semibold text-navy text-sm">{opt.label}</span>
                  {opt.sub && <span className="text-xs text-gray-500">{opt.sub}</span>}
                </button>
              ))}
            </div>
            <button
              onClick={() => setRepeatOpen(false)}
              className="w-full mt-3 py-3 text-sm text-gray-500 font-medium"
              style={{ minHeight: 'auto' }}
            >
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
