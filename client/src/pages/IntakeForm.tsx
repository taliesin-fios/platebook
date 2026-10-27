// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * IntakeForm — Public-facing client intake form
 * Route: /intake/new    — new submission (no auth required)
 * Route: /intake/:id    — edit existing intake (requires ?token= HMAC)
 *
 * On submit → creates or updates intake record, shows confirmation.
 */
import { useState, useEffect, type FormEvent, type InputHTMLAttributes, type ReactNode, type TextareaHTMLAttributes } from 'react';
import { useParams, useSearchParams } from 'react-router';
import { ALLERGENS_LIST, DIETARY_STYLES, PROTEINS, centsToDollars } from '../constants';
import type { ClientIntakeDetail, IntakeSubmitResponse, PaymentStatus, PublicProfile } from '../types';
import PoweredBy from '../components/PoweredBy';

interface IntakeFormData {
  name: string;
  email: string;
  phone: string;
  address: string;
  household_type: string;
  // '' = not answered (sent as null)
  adults: number | '';
  children: number | '';
  preferred_serving_size: number | '';
  allergens: string[];
  dietary_styles: string[];
  liked_proteins: string[];
  disliked_foods: string;
  spice_tolerance: string; // '' = not answered
  cuisine_preferences: string;
  preferred_day: string;
  preferred_time: string;
  scheduling_notes: string;
  kitchen_notes: string;
  client_containers: boolean | null; // null = not answered
  grocery_handling: string; // '' = not answered
  number_of_meals: number | string;
  notes_for_chef: string;
  terms_accepted: boolean;
}

/** Field errors keyed by form field (or a server-named field), plus `_global`. */
type FormErrors = Record<string, string | undefined> & { _global?: string };

type ListField = 'allergens' | 'dietary_styles' | 'liked_proteins';

const SERVING_OPTIONS = [2, 4, 6, 8];

/** A whole number typed into a count field, or '' when it's cleared. */
const countOrBlank = (v: string): number | '' => {
  const n = parseInt(v, 10);
  return Number.isNaN(n) ? '' : Math.max(0, n);
};
const DAYS = ['Sunday','Monday','Tuesday','Wednesday','Thursday','Friday','Saturday'];
const SPICE = ['Mild','Medium','Bold','No Preference'];

// ── Small helpers ─────────────────────────────────────────────────────────────

const Label = ({ children, required }: { children: ReactNode; required?: boolean }) => (
  <label className="block text-sm font-medium mb-1" style={{ color: '#28302A' }}>
    {children}{required && <span style={{ color: '#A94442' }} className="ml-1">*</span>}
  </label>
);

const Input = ({ error, ...props }: InputHTMLAttributes<HTMLInputElement> & { error?: string }) => (
  <input
    {...props}
    style={{ borderColor: error ? '#A94442' : undefined }}
    className={`w-full px-4 py-3 rounded-xl border text-sm focus:outline-hidden focus:ring-2 focus:ring-green-500 bg-white ${error ? 'border-red-400' : 'border-gray-200'}`}
  />
);

const TextArea = ({ ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) => (
  <textarea
    {...props}
    className="w-full px-4 py-3 rounded-xl border border-gray-200 text-sm focus:outline-hidden focus:ring-2 focus:ring-green-500 bg-white resize-none"
  />
);

const ChipGroup = ({ options, selected, onChange }: { options: string[]; selected: string[]; onChange: (next: string[]) => void }) => (
  <div className="flex flex-wrap gap-2 overflow-x-hidden">
    {options.map(o => {
      const active = selected.includes(o);
      return (
        <button key={o} type="button"
          onClick={() => onChange(active ? selected.filter(x => x !== o) : [...selected, o])}
          className={`px-3 py-1.5 rounded-full text-sm font-medium border transition-colors ${
            active
              ? 'border-transparent text-white'
              : 'border-gray-200 bg-white text-gray-600 hover:border-gray-400'
          }`}
          style={active ? { background: '#5E7F57', borderColor: '#5E7F57' } : {}}
        >
          {o}
        </button>
      );
    })}
  </div>
);

const Section = ({ number, title, children }: { number: ReactNode; title: ReactNode; children: ReactNode }) => (
  <div className="mb-8">
    <div className="flex items-center gap-3 mb-4">
      <div className="w-7 h-7 rounded-full flex items-center justify-center text-white text-xs font-bold shrink-0"
        style={{ background: '#5E7F57' }}>
        {number}
      </div>
      <h2 className="font-semibold text-lg" style={{ color: '#28302A' }}>{title}</h2>
    </div>
    <div className="space-y-4 pl-10">{children}</div>
  </div>
);

/**
 * The booking deposit on the client's own pages: pay it online when the chef
 * takes card payments (PAYMENTS_MODE=stripe), or say it's paid or waived.
 */
const DepositPanel = ({ amount, status, online, methods, notice, paying, error, onPay }: {
  /** Dollars, formatted. */
  amount: string;
  status: PaymentStatus | null;
  online: boolean;
  methods?: string | null;
  /** After coming back from checkout. */
  notice?: { text: string; hidePay: boolean } | null;
  paying: boolean;
  error: string;
  onPay: () => void;
}) => {
  const settled = status === 'paid' || status === 'paid_offline';
  return (
    <div className="rounded-2xl p-5 mb-6 border" style={{ background: '#EEF3EA', borderColor: '#5E7F57' }}>
      <h2 className="font-semibold mb-1" style={{ color: '#3A5C32' }}>Booking Deposit</h2>
      {settled ? (
        <p className="text-sm" style={{ color: '#3A5C32' }}>✓ Your ${amount} deposit is paid. Thank you!</p>
      ) : status === 'waived' ? (
        <p className="text-sm text-gray-600">Your chef has waived the deposit.</p>
      ) : (
        <>
          {notice && <p className="text-sm font-medium mb-2" style={{ color: '#3A5C32' }}>{notice.text}</p>}
          <p className="text-sm text-gray-600">
            A <strong>${amount} booking deposit</strong> holds your session date.
            {!online && <> Your chef will be in touch to arrange payment{methods ? <> ({methods})</> : null}.</>}
          </p>
          {online && !notice?.hidePay && (
            <button type="button" onClick={onPay} disabled={paying}
              className="mt-3 w-full py-3 rounded-xl text-white font-semibold text-sm disabled:opacity-60"
              style={{ background: '#5E7F57' }}>
              {paying ? 'Opening secure checkout…' : `Pay $${amount} deposit →`}
            </button>
          )}
          {error && <p className="text-xs mt-2" style={{ color: '#A94442' }}>{error}</p>}
        </>
      )}
    </div>
  );
};

// ── Main component ─────────────────────────────────────────────────────────────

export default function IntakeForm() {
  const { id: editId, slug } = useParams();
  const [searchParams] = useSearchParams();
  const editToken = searchParams.get('token');
  const isEditMode = !!editId && !!editToken;

  const [step, setStep] = useState<'loading' | 'form' | 'success' | 'error'>('loading');
  const [submitting, setSubmitting] = useState(false);
  const [errors, setErrors] = useState<FormErrors>({});
  const [result, setResult] = useState<IntakeSubmitResponse | ClientIntakeDetail | null>(null);
  // The chef's public profile: name, logo and booking policy. Comes from the
  // /book/:slug link, from the intake itself in edit mode, or (legacy
  // /intake/new links) from the original account.
  const [profile, setProfile] = useState<PublicProfile | null>(null);
  const bizName = profile?.business_name || '';
  const depositCents = Number(profile?.deposit_cents) || 0;
  const cancelHours  = profile?.cancellation_hours ?? 48;
  const onlineDeposits = profile?.payments_mode === 'stripe';
  // The client's own deposit, from their intake (edit mode).
  const [deposit, setDeposit] = useState<{ status: PaymentStatus | null; amount: number | null } | null>(null);
  const [paying, setPaying] = useState(false);
  const [payError, setPayError] = useState('');
  // Stripe sends the client back here with &paid=1 or &cancelled=1.
  const returnNotice = searchParams.get('paid') === '1'
    ? { text: 'Thanks! Your payment went through. It can take a minute to show here.', hidePay: true }
    : searchParams.get('cancelled') === '1'
      ? { text: 'Payment cancelled. You can try again whenever you’re ready.', hidePay: false }
      : null;
  const depositAmount = (dollars: number | null | undefined) =>
    centsToDollars(dollars ? Math.round(dollars * 100) : depositCents);

  const payDeposit = async (intakeId: number, token: string) => {
    setPaying(true);
    setPayError('');
    try {
      const res = await fetch(`/api/intake/${intakeId}/checkout?token=${encodeURIComponent(token)}`, { method: 'POST' });
      const data: { checkout_url?: string | null; error?: string; code?: string } = await res.json();
      if (res.ok && data.checkout_url) {
        window.location.assign(data.checkout_url);
        return;
      }
      setPayError(data.code === 'ALREADY_PAID'
        ? 'Your payment has gone through. It can take a minute to show here.'
        : data.error || 'Could not start the payment. Please try again.');
    } catch {
      setPayError('Network error — please check your connection and try again');
    }
    setPaying(false);
  };
  const responseTime = profile?.response_time_text || 'soon';

  // Tab title follows the chef being booked, not whoever is signed in
  useEffect(() => { if (bizName) document.title = bizName; }, [bizName]);

  const loadProfile = (chefSlug: string | null | undefined) => {
    const url = chefSlug ? `/api/public/${encodeURIComponent(chefSlug)}/settings` : '/api/settings/public';
    return fetch(url).then(r => {
      if (!r.ok) throw new Error('not_found');
      return r.json();
    }).then(setProfile);
  };

  useEffect(() => {
    if (isEditMode) return; // loaded from the intake's chef_slug below
    loadProfile(slug).catch(() => {
      if (slug) {
        setErrors({ _global: 'This booking link isn’t active. Please check the link with your chef.' });
        setStep('error');
      }
    });
  }, [slug, isEditMode]);

  // In edit mode, fetch the existing intake and prefill the form
  useEffect(() => {
    if (!isEditMode) { setStep('form'); return; }
    fetch(`/api/intake/${editId}?token=${editToken}`)
      .then(r => {
        if (r.status === 403) throw new Error('expired');
        if (!r.ok) throw new Error('not_found');
        return r.json();
      })
      .then((data: ClientIntakeDetail) => {
        loadProfile(data.chef_slug).catch(() => {});
        setDeposit({ status: data.payment_status, amount: data.booking_fee_amount });
        setForm(prev => ({
          ...prev,
          name: data.name || '',
          email: data.email || '',
          phone: data.phone || '',
          address: data.address || '',
          household_type: data.household_type || '',
          adults: data.adults ?? '',
          children: data.children ?? '',
          preferred_serving_size: data.preferred_serving_size ?? '',
          allergens: data.allergens || [],
          dietary_styles: data.dietary_styles || [],
          liked_proteins: data.liked_proteins || [],
          disliked_foods: data.disliked_foods || '',
          spice_tolerance: data.spice_tolerance || '',
          cuisine_preferences: data.cuisine_preferences || '',
          preferred_day: data.preferred_day || '',
          preferred_time: data.preferred_time || '',
          scheduling_notes: data.scheduling_notes || '',
          kitchen_notes: data.kitchen_notes || '',
          // Older rows may hold a boolean
          client_containers: data.client_containers == null
            ? null
            : data.client_containers === 1 || (data.client_containers as unknown) === true,
          grocery_handling: data.grocery_handling || '',
          number_of_meals: data.number_of_meals || '',
          notes_for_chef: data.notes_for_chef || '',
          terms_accepted: true,
        }));
        setStep('form');
      })
      .catch((err: Error) => {
        setStep('error');
        setErrors({ _global: err.message === 'expired' ? 'This edit link is invalid or has expired.' : 'Could not load your intake form.' });
      });
  }, [editId, editToken, isEditMode]);

  const [form, setForm] = useState<IntakeFormData>({
    // Contact
    name: '', email: '', phone: '', address: '',
    // Household
    // Household starts blank: an untouched answer shouldn't read as one.
    household_type: '', adults: '', children: '', preferred_serving_size: '',
    // Dietary
    allergens: [], dietary_styles: [], liked_proteins: [],
    disliked_foods: '', spice_tolerance: '', cuisine_preferences: '',
    // Scheduling
    preferred_day: '', preferred_time: '', scheduling_notes: '',
    // Kitchen / service
    // No choice is made for the client: unanswered stays blank.
    kitchen_notes: '', client_containers: null, grocery_handling: '',
    number_of_meals: '',
    // Notes
    notes_for_chef: '',
    // Legal
    terms_accepted: false,
  });

  const set = <K extends keyof IntakeFormData>(f: K, v: IntakeFormData[K]) => {
    setForm(p => ({ ...p, [f]: v }));
    setErrors(prev => { if (!prev._global && !prev[f]) return prev; const { _global, [f]: _, ...rest } = prev; return rest; });
  };
  const toggle = (f: ListField, v: string) => set(f, form[f].includes(v) ? form[f].filter(x => x !== v) : [...form[f], v]);

  const validate = () => {
    const e: FormErrors = {};
    if (!form.name.trim()) e.name = 'Your name is required';
    if (!form.email.trim()) e.email = 'Email is required';
    else if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(form.email)) e.email = 'Enter a valid email';
    if (!isEditMode && !form.terms_accepted) e.terms = 'Please accept the terms to continue';
    setErrors(e);
    return Object.keys(e).length === 0;
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!validate()) {
      document.getElementById('intake-name')?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      return;
    }
    setSubmitting(true);
    try {
      const payload = {
        ...form,
        ...(slug && !isEditMode ? { slug } : {}),
        household_type: form.household_type || null,
        adults: form.adults === '' ? null : form.adults,
        children: form.children === '' ? null : form.children,
        preferred_serving_size: form.preferred_serving_size === '' ? null : form.preferred_serving_size,
        spice_tolerance: form.spice_tolerance || null,
        client_containers: form.client_containers === null ? null : form.client_containers ? 1 : 0,
        grocery_handling: form.grocery_handling || null,
        number_of_meals: form.number_of_meals ? parseInt(String(form.number_of_meals)) : null,
      };
      const url = isEditMode ? `/api/intake/${editId}?token=${editToken}` : '/api/intake';
      const method = isEditMode ? 'PUT' : 'POST';

      const res = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const data: IntakeSubmitResponse & { error?: string; field?: string } = await res.json();
      if (!res.ok) {
        if (data.field) {
          setErrors(prev => ({ ...prev, [data.field!]: data.error }));
          setTimeout(() => document.getElementById(`intake-${data.field}`)?.scrollIntoView({ behavior: 'smooth', block: 'center' }), 50);
        } else {
          setErrors(prev => ({ ...prev, _global: data.error || 'Something went wrong' }));
          setTimeout(() => document.getElementById('intake-global-error')?.scrollIntoView({ behavior: 'smooth', block: 'center' }), 50);
        }
        return;
      }
      setResult(data);
      setStep('success');
      window.scrollTo({ top: 0, behavior: 'smooth' });
    } catch {
      setErrors(prev => ({ ...prev, _global: 'Network error — please check your connection and try again' }));
      setTimeout(() => document.getElementById('intake-global-error')?.scrollIntoView({ behavior: 'smooth', block: 'center' }), 50);
    } finally {
      setSubmitting(false);
    }
  };

  // ── Loading state (edit mode) ────────────────────────────────────────────────
  if (step === 'loading') {
    return (
      <div className="min-h-screen flex items-center justify-center" style={{ background: '#FBF8F1' }}>
        <p style={{ color: '#9CA3AF', fontSize: '14px' }}>Loading your booking request…</p>
      </div>
    );
  }

  // ── Error state (bad/expired edit link) ────────────────────────────────────
  if (step === 'error') {
    return (
      <div className="min-h-screen flex flex-col items-center justify-center px-6 py-12"
        style={{ background: '#FBF8F1' }}>
        <div className="max-w-md w-full text-center">
          <div className="text-5xl mb-4">🔒</div>
          <h1 className="text-xl font-bold mb-2" style={{ color: '#A94442' }}>
            Unable to load
          </h1>
          <p className="text-gray-600 text-sm mb-6">{errors._global || 'This link may be invalid or expired.'}</p>
          {isEditMode && (
            <a href={profile?.slug ? `/book/${profile.slug}` : '/intake/new'} className="inline-block px-6 py-3 rounded-xl text-white font-semibold text-sm"
              style={{ background: '#5E7F57' }}>Submit a New Request →</a>
          )}
        </div>
      </div>
    );
  }

  // ── Success screen ──────────────────────────────────────────────────────────
  if (step === 'success' && result) {
    return (
      <div className="min-h-screen flex flex-col items-center justify-center px-6 py-12"
        style={{ background: '#FBF8F1' }}>
        <div className="max-w-md w-full">
          <div className="text-center mb-8">
            <div className="text-5xl mb-4">{isEditMode ? '✅' : '🌿'}</div>
            <h1 className="text-2xl font-bold mb-2" style={{ color: '#3A5C32' }}>
              {isEditMode ? 'Preferences updated!' : "You're on your way!"}
            </h1>
            <p className="text-gray-600 text-sm">
              {isEditMode
                ? <>Your changes have been saved, <strong>{result.name}</strong>. Your chef will see the updated preferences.</>
                : <>Thanks, <strong>{result.name}</strong>. {bizName || 'Your chef'} has received your request and will be in touch {responseTime} to confirm your booking.</>
              }
            </p>
          </div>

          {/* Booking deposit — shown only when this chef takes one */}
          {!isEditMode && depositCents > 0 && 'edit_token' in result && (
            <DepositPanel amount={depositAmount(null)} status={null} online={onlineDeposits}
              methods={profile?.payment_methods} paying={paying} error={payError}
              onPay={() => payDeposit(result.id, result.edit_token)} />
          )}

          <p className="text-xs text-center text-gray-400">
            Your intake is saved — reference #{result.id}.
            {result.email && <> Confirmation sent to <strong>{result.email}</strong>.</>}
          </p>
          <PoweredBy className="mt-8" />
        </div>
      </div>
    );
  }

  // ── Form ─────────────────────────────────────────────────────────────────────
  return (
    <div className="min-h-screen px-4 py-8" style={{ background: '#FBF8F1', color: '#28302A' }}>
      <div className="max-w-lg mx-auto">

        {/* Header */}
        <div className="text-center mb-8">
          {profile?.logo_base64 && (
            <img src={profile.logo_base64} alt="" className="h-16 w-auto mx-auto mb-3 object-contain" />
          )}
          <h1 className="text-3xl font-bold font-serif mb-2" style={{ color: '#3A5C32' }}>
            {bizName || (isEditMode ? 'Update Your Preferences' : 'New Client Intake')}
          </h1>
          <p className="text-gray-500 text-sm">
            {isEditMode
              ? 'Review and update your preferences below. Changes are saved when you hit Update.'
              : 'New client booking: tell us about your household so we can tailor your meals.'}
          </p>
          {!isEditMode && (profile?.chef_name || profile?.location) && (
            <p className="text-gray-400 text-xs mt-1">
              {[profile.chef_name, profile.location].filter(Boolean).join(' · ')}
            </p>
          )}
        </div>

        {isEditMode && deposit && (depositCents > 0 || (deposit.amount ?? 0) > 0) && (
          <DepositPanel amount={depositAmount(deposit.amount)} status={deposit.status} online={onlineDeposits}
            methods={profile?.payment_methods} notice={returnNotice} paying={paying} error={payError}
            onPay={() => payDeposit(Number(editId), editToken!)} />
        )}

        {errors._global && (
          <div id="intake-global-error" className="mb-6 px-4 py-3 rounded-xl text-sm font-medium" style={{ background: '#FEE2E2', color: '#A94442' }}>
            {errors._global}
          </div>
        )}

        <form onSubmit={handleSubmit} noValidate>

          {/* 1 — Contact */}
          <Section number="1" title="Your Contact Info">
            <div id="intake-name">
              <Label required>Full Name / Family Name</Label>
              <Input value={form.name} onChange={e => set('name', e.target.value)}
                placeholder="e.g., The Johnson Family" error={errors.name} />
              {errors.name && <p className="text-xs mt-1" style={{ color: '#A94442' }}>{errors.name}</p>}
            </div>
            <div>
              <Label required>Email</Label>
              <Input type="email" value={form.email} onChange={e => set('email', e.target.value)}
                placeholder="you@example.com" error={errors.email} />
              {errors.email && <p className="text-xs mt-1" style={{ color: '#A94442' }}>{errors.email}</p>}
            </div>
            <div>
              <Label>Phone</Label>
              <Input type="tel" value={form.phone} onChange={e => set('phone', e.target.value)}
                placeholder="(555) 000-0000" />
            </div>
            <div>
              <Label>Home Address</Label>
              <Input value={form.address} onChange={e => set('address', e.target.value)}
                placeholder="123 Main St, City, CA" />
              <p className="text-xs text-gray-400 mt-1">Used to estimate travel — not shared.</p>
            </div>
          </Section>

          {/* 2 — Household */}
          <Section number="2" title="Household">
            <div>
              <Label>Household Type</Label>
              <div className="flex gap-2 flex-wrap">
                {['Individual','Couple','Small Family','Large Family'].map(t => (
                  <button key={t} type="button"
                    aria-pressed={form.household_type === t}
                    onClick={() => set('household_type', form.household_type === t ? '' : t)}
                    className="px-3 py-2 rounded-xl text-sm font-medium border transition-colors"
                    style={form.household_type === t
                      ? { background: '#5E7F57', color: 'white', borderColor: '#5E7F57' }
                      : { background: 'white', borderColor: '#D1D5DB', color: '#374151' }}
                  >{t}</button>
                ))}
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <Label>Adults</Label>
                <Input type="number" min="1" max="20" value={form.adults}
                  onChange={e => set('adults', countOrBlank(e.target.value))} placeholder="e.g., 2" />
              </div>
              <div>
                <Label>Children</Label>
                <Input type="number" min="0" max="20" value={form.children}
                  onChange={e => set('children', countOrBlank(e.target.value))} placeholder="e.g., 0" />
              </div>
            </div>
            <div>
              <Label>Preferred Serving Size</Label>
              <select value={form.preferred_serving_size}
                onChange={e => set('preferred_serving_size', countOrBlank(e.target.value))}
                className="w-full px-4 py-3 rounded-xl border border-gray-200 text-sm bg-white focus:outline-hidden focus:ring-2 focus:ring-green-500"
              >
                <option value="">Choose…</option>
                {SERVING_OPTIONS.map(n => <option key={n} value={n}>{n} servings</option>)}
                <option value={10}>10 servings</option>
                <option value={12}>12 servings</option>
              </select>
            </div>
            <div>
              <Label>Number of Different Meals per Visit</Label>
              <Input type="number" min="1" max="20" value={form.number_of_meals}
                onChange={e => set('number_of_meals', e.target.value)}
                placeholder="e.g., 4" />
            </div>
          </Section>

          {/* 3 — Dietary */}
          <Section number="3" title="Dietary Profile">
            <div>
              <Label>Allergens (select all that apply)</Label>
              <ChipGroup options={ALLERGENS_LIST} selected={form.allergens}
                onChange={v => set('allergens', v)} />
            </div>
            <div>
              <Label>Dietary Style</Label>
              <ChipGroup options={DIETARY_STYLES} selected={form.dietary_styles}
                onChange={v => set('dietary_styles', v)} />
            </div>
            <div>
              <Label>Proteins You Enjoy</Label>
              <ChipGroup options={PROTEINS} selected={form.liked_proteins}
                onChange={v => set('liked_proteins', v)} />
            </div>
            <div>
              <Label>Foods to Avoid</Label>
              <TextArea rows={2} value={form.disliked_foods}
                onChange={e => set('disliked_foods', e.target.value)}
                placeholder="e.g., mushrooms, cilantro, olives..." />
            </div>
            <div>
              <Label>Spice Tolerance</Label>
              <div className="flex gap-2 flex-wrap">
                {SPICE.map(s => (
                  <button key={s} type="button"
                    aria-pressed={form.spice_tolerance === s}
                    onClick={() => set('spice_tolerance', form.spice_tolerance === s ? '' : s)}
                    className="px-3 py-2 rounded-xl text-sm border"
                    style={form.spice_tolerance === s
                      ? { background: '#5E7F57', color: 'white', borderColor: '#5E7F57' }
                      : { background: 'white', borderColor: '#D1D5DB', color: '#374151' }}
                  >{s}</button>
                ))}
              </div>
            </div>
            <div>
              <Label>Cuisine Preferences</Label>
              <TextArea rows={2} value={form.cuisine_preferences}
                onChange={e => set('cuisine_preferences', e.target.value)}
                placeholder="e.g., Mediterranean, Asian-inspired, comfort food..." />
            </div>
          </Section>

          {/* 4 — Scheduling */}
          <Section number="4" title="Scheduling Preferences">
            <div>
              <Label>Preferred Day</Label>
              <select value={form.preferred_day} onChange={e => set('preferred_day', e.target.value)}
                className="w-full px-4 py-3 rounded-xl border border-gray-200 text-sm bg-white focus:outline-hidden">
                <option value="">No preference</option>
                {DAYS.map(d => <option key={d} value={d}>{d}</option>)}
              </select>
            </div>
            <div>
              <Label>Preferred Time</Label>
              <input type="time" value={form.preferred_time}
                onChange={e => set('preferred_time', e.target.value)}
                className="w-full px-4 py-3 rounded-xl border border-gray-200 text-sm bg-white focus:outline-hidden" />
            </div>
            <div>
              <Label>Scheduling Notes</Label>
              <TextArea rows={2} value={form.scheduling_notes}
                onChange={e => set('scheduling_notes', e.target.value)}
                placeholder="Any flexibility constraints, travel windows, etc." />
            </div>
          </Section>

          {/* 5 — Kitchen & Service */}
          <Section number="5" title="Kitchen & Service">
            <div>
              <Label>Will you provide food storage containers?</Label>
              <div className="flex gap-3">
                {[true, false].map(v => (
                  <button key={String(v)} type="button"
                    aria-pressed={form.client_containers === v}
                    onClick={() => set('client_containers', form.client_containers === v ? null : v)}
                    className="flex-1 py-2.5 rounded-xl text-sm font-medium border transition-colors"
                    style={form.client_containers === v
                      ? { background: '#5E7F57', color: 'white', borderColor: '#5E7F57' }
                      : { background: 'white', borderColor: '#D1D5DB', color: '#374151' }}
                  >{v ? 'Yes, I will' : 'No, chef provides'}</button>
                ))}
              </div>
              {form.client_containers === false && (
                <p className="mt-2 text-xs px-3 py-2 rounded-xl" style={{ background: '#FEF3C7', color: '#92400E' }}>
                  📦 Chef-provided containers may add a small fee to your session cost.
                </p>
              )}
            </div>
            <div>
              <Label>Grocery Purchasing</Label>
              <div className="flex gap-2 flex-wrap">
                {['Chef shops','Client shops','Split'].map(o => (
                  <button key={o} type="button"
                    aria-pressed={form.grocery_handling === o}
                    onClick={() => set('grocery_handling', form.grocery_handling === o ? '' : o)}
                    className="px-3 py-2 rounded-xl text-sm border"
                    style={form.grocery_handling === o
                      ? { background: '#5E7F57', color: 'white', borderColor: '#5E7F57' }
                      : { background: 'white', borderColor: '#D1D5DB', color: '#374151' }}
                  >{o}{o === 'Chef shops' ? ' (+ fee)' : ''}</button>
                ))}
              </div>
            </div>
            <div>
              <Label>Kitchen Notes</Label>
              <TextArea rows={3} value={form.kitchen_notes}
                onChange={e => set('kitchen_notes', e.target.value)}
                placeholder="Parking, gate code, key location, appliance quirks, pets..." />
            </div>
          </Section>

          {/* 6 — Notes for chef */}
          <Section number="6" title="Anything Else?">
            <div>
              <Label>Notes for Your Chef</Label>
              <TextArea rows={4} value={form.notes_for_chef}
                onChange={e => set('notes_for_chef', e.target.value)}
                placeholder="Share anything that would help us create the perfect meals for your household..." />
            </div>
          </Section>

          {/* Terms — only shown on new submissions (already accepted in edit mode) */}
          {!isEditMode && <div className="mb-6 p-4 rounded-2xl border" style={{ background: '#EEF3EA', borderColor: '#5E7F57' }}>
            <h3 className="font-semibold mb-2 text-sm" style={{ color: '#3A5C32' }}>Booking Terms</h3>
            <p className="text-xs text-gray-600 mb-3 whitespace-pre-line">
              {profile?.booking_terms || (depositCents > 0
                ? `A $${centsToDollars(depositCents)} deposit is required to confirm your booking. Cancellations within ${cancelHours} hours of the scheduled session forfeit the deposit.`
                : `Please give at least ${cancelHours} hours’ notice if you need to cancel or reschedule.`)}
            </p>
            <label className="flex items-start gap-3 cursor-pointer">
              <input
                type="checkbox"
                checked={form.terms_accepted}
                onChange={e => set('terms_accepted', e.target.checked)}
                className="mt-0.5 w-5 h-5 rounded accent-green-700 shrink-0"
              />
              <span className="text-xs text-gray-700">
                I have read and agree to the booking terms above{depositCents > 0 ? ', including the deposit and cancellation policy' : ''}.
              </span>
            </label>
            {errors.terms && (
              <p className="text-xs mt-2" style={{ color: '#A94442' }}>{errors.terms}</p>
            )}
          </div>}

          <button
            type="submit"
            disabled={submitting}
            className="w-full py-4 rounded-2xl font-bold text-white text-base transition-opacity disabled:opacity-60"
            style={{ background: '#5E7F57' }}
          >
            {submitting ? (isEditMode ? 'Saving…' : 'Submitting…') : (isEditMode ? 'Update My Preferences →' : 'Submit My Intake →')}
          </button>

          <p className="text-xs text-center text-gray-400 mt-4">
            Your information is private and only shared with your assigned chef.
          </p>
          <PoweredBy className="mt-6" />
        </form>
      </div>
    </div>
  );
}
