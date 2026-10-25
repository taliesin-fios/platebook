// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * IntakeAdmin — Chef view of all client intake submissions
 * Route: /intake  (chef/admin only — inside the main app)
 */
import { useState, useEffect, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { fmtUtcTimestamp } from '../utils/dateUtils';
import PageHeader from '../components/PageHeader';
import EmailFailedBanner from '../components/EmailFailedBanner';
import { useToast } from '../components/Toast';
import { apiCall, errorMessage } from '../hooks/useApi';
import { useAppSettings } from '../App';
import { bookingUrl } from '../constants';
import type { EmailSentResponse, Id, Intake, IntakeConvertResponse, IntakeRow, IntakeStatus } from '../types';

/** After a payment action the server returns the raw row, with JSON columns as strings. */
type IntakeState = Intake | IntakeRow;

const STATUS_LABELS: Record<string, { label: string; color: string }> = {
  submitted:       { label: 'Submitted',      color: 'bg-blue-100 text-blue-700' },
  deposit_pending: { label: 'Deposit Pending', color: 'bg-amber-100 text-amber-700' },
  deposit_paid:    { label: 'Deposit Paid',   color: 'bg-green-light text-green-dark' },
  reviewed_by_chef:{ label: 'Reviewed',        color: 'bg-navy text-white' },
  booked:          { label: 'Booked',          color: 'bg-green-dark text-white' },
  cancelled:       { label: 'Cancelled',       color: 'bg-gray-100 text-gray-500' },
};

const PAYMENT_LABELS: Record<string, { label: string; color: string }> = {
  // Manual statuses
  not_requested:     { label: 'Not Requested',     color: 'text-gray-400'                        },
  deposit_requested: { label: 'Deposit Requested', color: 'text-amber-600'                       },
  invoice_sent:      { label: 'Invoice Sent',      color: 'text-blue-600'                        },
  payment_pending:   { label: 'Payment Pending',   color: 'text-amber-600 font-medium'           },
  paid_offline:      { label: '✓ Paid (Offline)',  color: 'text-green-dark font-semibold'        },
  waived:            { label: 'Waived',            color: 'text-purple-600'                      },
  // Legacy Stripe statuses — kept for backward compat with existing DB rows
  unpaid:            { label: 'Unpaid',            color: 'text-gray-400'                        },
  pending:           { label: 'Pending',           color: 'text-amber-600'                       },
  paid:              { label: '✓ Paid',            color: 'text-green-dark font-semibold'        },
  failed:            { label: 'Failed',            color: 'text-danger'                          },
  refunded:          { label: 'Refunded',          color: 'text-gray-500'                        },
};

const ALL_STATUSES: IntakeStatus[] = ['submitted','deposit_pending','deposit_paid','reviewed_by_chef','booked','cancelled'];

function IntakeCard({ intake, onSelect }: { intake: Intake; onSelect: (intake: Intake) => void }) {
  const sl = STATUS_LABELS[intake.status] || STATUS_LABELS.submitted;
  const pl = PAYMENT_LABELS[intake.payment_status] || PAYMENT_LABELS.unpaid;

  return (
    <div
      className="card mb-3 cursor-pointer hover:shadow-card-hover transition-shadow"
      onClick={() => onSelect(intake)}
    >
      <div className="flex items-start justify-between mb-2">
        <div>
          <p className="font-semibold text-navy">{intake.name}</p>
          <p className="text-xs text-gray-400 mt-0.5">{intake.email}</p>
        </div>
        <span className={`text-xs px-2 py-1 rounded-full font-medium ${sl.color}`}>
          {sl.label}
        </span>
      </div>
      <div className="flex items-center gap-3 text-xs text-gray-500 flex-wrap">
        {intake.preferred_serving_size && (
          <span>👥 {intake.preferred_serving_size} servings</span>
        )}
        {intake.preferred_day && <span>📅 {intake.preferred_day}</span>}
        <span className={pl.color}>{pl.label}</span>
        {intake.booking_fee_amount! > 0 && (
          <span>${(intake.booking_fee_amount || 0).toFixed(0)} deposit</span>
        )}
        {intake.confirmation_email_failed_at && (
          <span className="text-danger font-medium">⚠ Confirmation email failed</span>
        )}
        {intake.chef_alert_failed_at && (
          <span className="text-amber-600 font-medium">⚠ Alert email failed</span>
        )}
      </div>
      <p className="text-xs text-gray-300 mt-2">
        {fmtUtcTimestamp(intake.created_at, 'MMM d, yyyy', intake.created_at)}
      </p>
    </div>
  );
}

interface IntakeDetailProps {
  intake: Intake;
  onClose: () => void;
  onStatusChange: (id: Id, status: IntakeStatus, notes: string) => void;
  onConvert: (intakeId: Id, clientId: Id) => void;
  onEmailChange: (id: Id, patch: Pick<Intake, 'confirmation_emailed_at' | 'confirmation_email_failed_at'>) => void;
}

function IntakeDetail({ intake: initialIntake, onClose, onStatusChange, onConvert, onEmailChange }: IntakeDetailProps) {
  const [intake, setIntake]         = useState<IntakeState>(initialIntake);
  const [status, setStatus]         = useState(initialIntake.status);
  const [notes, setNotes]           = useState(initialIntake.internal_notes || '');
  const [saving, setSaving]         = useState(false);
  const [paymentNote, setPaymentNote] = useState('');
  const [paymentAmount, setPaymentAmount] = useState('');
  const [paymentBusy, setPaymentBusy] = useState(false);
  const [resending, setResending]   = useState(false);
  const { show } = useToast();

  const allergens      = Array.isArray(intake.allergens)      ? intake.allergens      : [];
  const dietaryStyles  = Array.isArray(intake.dietary_styles) ? intake.dietary_styles : [];
  const likedProteins  = Array.isArray(intake.liked_proteins) ? intake.liked_proteins : [];

  const saveNotes = async () => {
    setSaving(true);
    try {
      await apiCall(`/api/intake/${intake.id}`, 'PUT', { status, internal_notes: notes });
      show('Intake updated', 'success');
      onStatusChange(intake.id, status, notes);
    } catch (err) {
      show(errorMessage(err) || 'Could not save', 'error');
    } finally {
      setSaving(false);
    }
  };

  const handlePaymentAction = async (action: string) => {
    if (action === 'mark_paid_offline') {
      if (!confirm('Mark this deposit as paid offline? This cannot be easily undone.')) return;
    }
    setPaymentBusy(true);
    try {
      const body: { intake_id: Id; action: string; notes?: string; amount?: number } = { intake_id: intake.id, action };
      if (paymentNote.trim()) body.notes = paymentNote.trim();
      if (paymentAmount && !isNaN(Number(paymentAmount))) body.amount = Number(paymentAmount);

      const res = await fetch('/api/payments/manual/action', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      const data: IntakeRow & { error?: string } = await res.json();
      if (!res.ok) throw new Error(data.error || 'Action failed');

      setIntake(data);
      setStatus(data.status);
      setPaymentNote('');
      setPaymentAmount('');
      show(PAYMENT_LABELS[data.payment_status]?.label
        ? `Payment status → ${PAYMENT_LABELS[data.payment_status].label}`
        : 'Payment status updated', 'success');
      onStatusChange(intake.id, data.status, notes);
    } catch (err) {
      show(errorMessage(err) || 'Could not update payment', 'error');
    } finally {
      setPaymentBusy(false);
    }
  };

  const resendConfirmation = async () => {
    setResending(true);
    const now = new Date().toISOString();
    try {
      const res = await apiCall<EmailSentResponse>(`/api/intake/${intake.id}/resend-confirmation`, 'POST');
      const patch = { confirmation_emailed_at: now, confirmation_email_failed_at: null };
      setIntake(prev => ({ ...prev, ...patch }));
      onEmailChange(intake.id, patch);
      show(`Confirmation sent to ${res.sent_to}`, 'success');
    } catch (err) {
      show(errorMessage(err) || 'Could not send the confirmation', 'error');
      // Not every error is a failed send (rate limit, no address): show what the server recorded.
      try {
        const fresh = await apiCall<Intake>(`/api/intake/${intake.id}`);
        const patch = {
          confirmation_emailed_at: fresh.confirmation_emailed_at,
          confirmation_email_failed_at: fresh.confirmation_email_failed_at,
        };
        setIntake(prev => ({ ...prev, ...patch }));
        onEmailChange(intake.id, patch);
      } catch { /* keep what's shown */ }
    } finally {
      setResending(false);
    }
  };

  const handleConvert = async () => {
    if (!confirm(`Convert ${intake.name}'s intake to a client record?`)) return;
    setSaving(true);
    try {
      const res = await apiCall<IntakeConvertResponse>(`/api/intake/${intake.id}/convert`, 'POST');
      show(`Client record created: ${res.client.name}`, 'success');
      onConvert(intake.id, res.client.id);
      onClose();
    } catch (err) {
      show(errorMessage(err) || 'Could not convert', 'error');
    } finally {
      setSaving(false);
    }
  };

  const InfoRow = ({ label, value }: { label: string; value: ReactNode }) => value ? (
    <div className="flex justify-between text-sm py-1.5 border-b border-gray-50 last:border-0">
      <span className="text-gray-500">{label}</span>
      <span className="font-medium text-navy text-right max-w-[55%]">{value}</span>
    </div>
  ) : null;

  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <div className="flex items-start justify-between mb-4">
          <div>
            <h2 className="font-display font-bold text-green-dark text-lg">{intake.name}</h2>
            <p className="text-xs text-gray-400">{intake.email} · #{intake.id}</p>
          </div>
          <button onClick={onClose} className="text-gray-400 text-xl leading-none" style={{ minHeight: 'auto' }}>×</button>
        </div>

        <div className="space-y-4 overflow-y-auto" style={{ maxHeight: '65vh' }}>
          <EmailFailedBanner what="booking confirmation" failedAt={intake.confirmation_email_failed_at}
            onRetry={resendConfirmation} retrying={resending} className="" />
          {intake.chef_alert_failed_at && (
            <div role="status" className="rounded-xl border border-amber-200 bg-amber-50 px-3.5 py-3 text-sm text-amber-700">
              ⚠ We couldn’t email you about this inquiry ({fmtUtcTimestamp(intake.chef_alert_failed_at, "MMM d 'at' h:mm a")}).
              Check the business email in Settings so you hear about the next one.
            </div>
          )}

          {/* Contact */}
          <div className="card">
            <p className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Contact</p>
            <InfoRow label="Phone"   value={intake.phone} />
            <InfoRow label="Address" value={intake.address} />
          </div>

          {/* Household */}
          <div className="card">
            <p className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Household</p>
            <InfoRow label="Type"          value={intake.household_type} />
            <InfoRow label="Adults"        value={intake.adults} />
            <InfoRow label="Children"      value={intake.children} />
            <InfoRow label="Serving Size"  value={`${intake.preferred_serving_size} servings`} />
            <InfoRow label="# of Meals"    value={intake.number_of_meals} />
          </div>

          {/* Dietary */}
          <div className="card">
            <p className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Dietary</p>
            {allergens.length > 0 && (
              <div className="mb-2">
                <p className="text-xs text-gray-500 mb-1">Allergens</p>
                <div className="flex flex-wrap gap-1">
                  {allergens.map(a => (
                    <span key={a} className="px-2 py-0.5 text-xs rounded-full bg-amber-50 text-amber-700 border border-amber-200">{a}</span>
                  ))}
                </div>
              </div>
            )}
            {dietaryStyles.length > 0 && (
              <div className="mb-2">
                <p className="text-xs text-gray-500 mb-1">Dietary Style</p>
                <div className="flex flex-wrap gap-1">
                  {dietaryStyles.map(d => (
                    <span key={d} className="px-2 py-0.5 text-xs rounded-full bg-green-light text-green-dark">{d}</span>
                  ))}
                </div>
              </div>
            )}
            {likedProteins.length > 0 && (
              <div className="mb-2">
                <p className="text-xs text-gray-500 mb-1">Likes</p>
                <p className="text-sm text-navy">{likedProteins.join(', ')}</p>
              </div>
            )}
            <InfoRow label="Avoids"     value={intake.disliked_foods} />
            <InfoRow label="Spice"      value={intake.spice_tolerance} />
            <InfoRow label="Cuisines"   value={intake.cuisine_preferences} />
          </div>

          {/* Scheduling */}
          <div className="card">
            <p className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Scheduling</p>
            <InfoRow label="Day"    value={intake.preferred_day} />
            <InfoRow label="Time"   value={intake.preferred_time} />
            <InfoRow label="Notes"  value={intake.scheduling_notes} />
          </div>

          {/* Kitchen / Service */}
          <div className="card">
            <p className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Kitchen & Service</p>
            <InfoRow label="Containers"  value={intake.client_containers ? 'Client provides' : 'Chef provides (+ fee)'} />
            <InfoRow label="Grocery"     value={intake.grocery_handling} />
            <InfoRow label="Kitchen Notes" value={intake.kitchen_notes} />
          </div>

          {/* Notes for chef */}
          {intake.notes_for_chef && (
            <div className="card">
              <p className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Client Notes</p>
              <p className="text-sm text-gray-700">{intake.notes_for_chef}</p>
            </div>
          )}

          {/* Payment */}
          <div className="card">
            <p className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Payment</p>

            {/* Current status row */}
            <div className="flex items-center justify-between mb-3">
              <span className="text-sm text-gray-500">Status</span>
              <span className={`text-sm font-medium ${PAYMENT_LABELS[intake.payment_status]?.color || 'text-gray-400'}`}>
                {PAYMENT_LABELS[intake.payment_status]?.label || intake.payment_status || 'Not Requested'}
              </span>
            </div>
            {intake.booking_fee_amount! > 0 && (
              <div className="flex items-center justify-between mb-1">
                <span className="text-sm text-gray-500">Amount</span>
                <span className="text-sm font-medium text-navy">${intake.booking_fee_amount!.toFixed(2)}</span>
              </div>
            )}
            {intake.booking_fee_paid_at && (
              <div className="flex items-center justify-between mb-1">
                <span className="text-sm text-gray-500">Paid At</span>
                <span className="text-sm text-gray-700">
                  {fmtUtcTimestamp(intake.booking_fee_paid_at, 'MMM d, yyyy', intake.booking_fee_paid_at)}
                </span>
              </div>
            )}
            {intake.payment_notes && (
              <div className="mt-2 p-2 bg-gray-50 rounded-lg text-xs text-gray-600">
                💬 {intake.payment_notes}
              </div>
            )}

            {/* Admin actions */}
            <div className="mt-4 pt-3 border-t border-gray-100">
              <p className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Payment Actions</p>

              {/* Optional note + amount for the action */}
              <div className="flex gap-2 mb-2">
                <input
                  type="text"
                  value={paymentNote}
                  onChange={e => setPaymentNote(e.target.value)}
                  placeholder="Note (e.g. paid via card, ref #1234)"
                  className="flex-1 text-xs px-3 py-2 border border-gray-200 rounded-lg"
                />
                <input
                  type="number"
                  value={paymentAmount}
                  onChange={e => setPaymentAmount(e.target.value)}
                  placeholder="$amt"
                  className="w-20 text-xs px-3 py-2 border border-gray-200 rounded-lg"
                  min="0"
                  step="0.01"
                />
              </div>

              {/* Action buttons */}
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-1.5">
                <button
                  onClick={() => handlePaymentAction('request_deposit')}
                  disabled={paymentBusy}
                  className="py-2 text-xs font-medium border border-amber-200 text-amber-700 bg-amber-50 rounded-lg disabled:opacity-50 hover:bg-amber-100 transition-colors"
                >
                  📨 Request Deposit
                </button>
                <button
                  onClick={() => handlePaymentAction('mark_invoice_sent')}
                  disabled={paymentBusy}
                  className="py-2 text-xs font-medium border border-blue-200 text-blue-700 bg-blue-50 rounded-lg disabled:opacity-50 hover:bg-blue-100 transition-colors"
                >
                  🧾 Invoice Sent
                </button>
                <button
                  onClick={() => handlePaymentAction('mark_paid_offline')}
                  disabled={paymentBusy}
                  className="py-2 text-xs font-medium border border-green-200 text-green-dark bg-green-light rounded-lg disabled:opacity-50 hover:opacity-80 transition-colors"
                >
                  ✓ Mark Deposit Paid
                </button>
                <button
                  onClick={() => handlePaymentAction('waive')}
                  disabled={paymentBusy}
                  className="py-2 text-xs font-medium border border-purple-200 text-purple-700 bg-purple-50 rounded-lg disabled:opacity-50 hover:bg-purple-100 transition-colors"
                >
                  🎁 Waive Deposit
                </button>
              </div>

              <button
                onClick={() => handlePaymentAction('reset')}
                disabled={paymentBusy}
                className="mt-1.5 w-full py-3 min-h-[44px] flex items-center justify-center text-xs text-gray-400 hover:text-gray-600 transition-colors"
              >
                Reset to Not Requested
              </button>
            </div>
          </div>

          {/* Admin controls */}
          <div className="card">
            <p className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-3">Admin Actions</p>
            <div className="mb-3">
              <label className="block text-sm font-medium text-gray-700 mb-1.5">Status</label>
              <select value={status} onChange={e => setStatus(e.target.value as IntakeStatus)}
                className="w-full">
                {ALL_STATUSES.map(s => (
                  <option key={s} value={s}>{STATUS_LABELS[s]?.label || s}</option>
                ))}
              </select>
            </div>
            <div className="mb-3">
              <label className="block text-sm font-medium text-gray-700 mb-1.5">Internal Notes</label>
              <textarea rows={3} value={notes} onChange={e => setNotes(e.target.value)}
                placeholder="Private notes (not visible to client)..." />
            </div>
            <button
              onClick={saveNotes}
              disabled={saving}
              className="w-full py-2.5 bg-navy text-white rounded-xl text-sm font-semibold mb-2 disabled:opacity-60"
            >
              {saving ? 'Saving…' : 'Save Status & Notes'}
            </button>
            {!intake.converted_client_id && (
              <button
                onClick={handleConvert}
                disabled={saving}
                className="w-full py-2.5 border-2 border-green-mid text-green-dark rounded-xl text-sm font-semibold disabled:opacity-60"
              >
                ✓ Convert to Client Record
              </button>
            )}
            {intake.converted_client_id && (
              <p className="text-xs text-center text-green-dark font-medium">
                ✓ Converted — Client #{intake.converted_client_id}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Main page ──────────────────────────────────────────────────────────────────

export default function IntakeAdmin() {
  const navigate = useNavigate();
  const { show } = useToast();
  const { slug } = useAppSettings();
  const [intakes, setIntakes]       = useState<Intake[]>([]);
  const [loading, setLoading]       = useState(true);
  const [selected, setSelected]     = useState<Intake | null>(null);
  const [filterStatus, setFilter]   = useState('');

  const load = () => {
    setLoading(true);
    const qs = filterStatus ? `?status=${filterStatus}` : '';
    fetch(`/api/intake${qs}`)
      .then(r => r.ok ? r.text() : '[]').then(t => t ? JSON.parse(t) : [])
      .then(d => setIntakes(Array.isArray(d) ? d : []))
      .finally(() => setLoading(false));
  };

  useEffect(load, [filterStatus]);

  const handleStatusChange = (id: Id, newStatus: IntakeStatus, newNotes: string) => {
    setIntakes(prev => prev.map(i =>
      i.id === id ? { ...i, status: newStatus, internal_notes: newNotes } : i
    ));
  };

  const handleEmailChange: IntakeDetailProps['onEmailChange'] = (id, patch) => {
    setIntakes(prev => prev.map(i => (i.id === id ? { ...i, ...patch } : i)));
  };

  const handleConvert = (intakeId: Id, clientId: Id) => {
    setIntakes(prev => prev.map(i =>
      i.id === intakeId ? { ...i, converted_client_id: clientId, status: 'booked' } : i
    ));
    navigate(`/clients/${clientId}`);
  };

  const newCount = intakes.filter(i => i.status === 'submitted').length;
  const emailFailedCount = intakes.filter(i => i.confirmation_email_failed_at || i.chef_alert_failed_at).length;

  return (
    <div className="page pb-24">
      <PageHeader
        title="Client Intakes"
        action={
          <span className="text-xs text-white opacity-70">
            {intakes.length} total
          </span>
        }
      />

      {newCount > 0 && (
        <div className="mx-4 mt-3 px-3 py-2 bg-amber-50 border border-amber-200 rounded-xl text-xs text-amber-700 font-medium">
          📥 {newCount} new submission{newCount !== 1 ? 's' : ''} awaiting review
        </div>
      )}

      {emailFailedCount > 0 && (
        <div role="alert" className="mx-4 mt-3 px-3 py-2 bg-red-50 border border-red-200 rounded-xl text-xs text-red-700 font-medium">
          ⚠ Booking emails didn’t go through for {emailFailedCount} inquir{emailFailedCount !== 1 ? 'ies' : 'y'}. Open one to see what failed and try again.
        </div>
      )}

      {/* Filter bar */}
      <div className="px-4 pt-3 pb-2">
        <div className="flex gap-2 overflow-x-auto no-scrollbar">
          {[{ value: '', label: 'All' }, ...ALL_STATUSES.map(s => ({ value: s, label: STATUS_LABELS[s].label }))].map(f => (
            <button
              key={f.value}
              onClick={() => setFilter(f.value)}
              className={`chip shrink-0 ${filterStatus === f.value ? 'chip-active' : 'chip-inactive'}`}
              style={{ minHeight: '36px' }}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      <div className="px-4 pt-2">
        {loading ? (
          <div className="space-y-3">
            {[1,2,3].map(i => <div key={i} className="skeleton h-20 rounded-2xl" />)}
          </div>
        ) : intakes.length === 0 ? (
          <div className="card text-center py-10">
            <div className="text-3xl mb-2">📥</div>
            <p className="text-gray-500 text-sm font-medium">No intakes yet</p>
            <p className="text-gray-400 text-xs mt-1">
              Share the intake form link with potential clients
            </p>
            <div className="mt-4 px-4 py-2 bg-green-light rounded-xl text-xs text-green-dark font-mono break-all">
              {bookingUrl(slug)}
            </div>
          </div>
        ) : (
          intakes.map(intake => (
            <IntakeCard key={intake.id} intake={intake} onSelect={setSelected} />
          ))
        )}
      </div>

      {selected && (
        <IntakeDetail
          key={selected.id}
          intake={selected}
          onClose={() => setSelected(null)}
          onStatusChange={handleStatusChange}
          onConvert={handleConvert}
          onEmailChange={handleEmailChange}
        />
      )}

      {/* Intake form link card */}
      <div className="px-4 mt-2">
        <div className="card bg-green-light border border-green-mid">
          <p className="text-xs font-semibold text-green-dark mb-1">📋 Share Your Booking Link</p>
          <p className="text-xs text-gray-600 mb-2">Send this link to new clients — it's safe, public, and requires no login.</p>
          <div className="flex items-center gap-2">
            <code className="text-xs bg-white px-2 py-1 rounded-lg border border-green-mid flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
              {bookingUrl(slug)}
            </code>
            <button
              onClick={() => { navigator.clipboard.writeText(bookingUrl(slug)).then(() => show('Booking link copied', 'success')).catch(() => {}); }}
              className="px-3 py-1.5 bg-navy text-white text-xs rounded-lg font-medium"
              style={{ minHeight: 'auto' }}
            >
              Copy
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
