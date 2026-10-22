// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useEffect, useRef, useState, useCallback, type ChangeEvent, type ReactNode } from 'react';
import { apiCall, errorMessage } from '../hooks/useApi';
import { useAppSettings } from '../App';
import {
  APP_NAME, SERVICE_TYPES, bookingUrl, dollarsToCents, centsToDollars,
} from '../constants';
import type { NumInput } from '../utils/numbers';
import type { OnboardingSeedResponse, Settings } from '../types';

interface OnboardingForm {
  business_name: string;
  chef_name: string;
  phone: string;
  location: string;
  website: string;
  logo_base64: string | null;
  service_types: string[];
  default_session_window: NumInput;
  default_serving_size: NumInput;
  default_service_fee: NumInput;
  deposit: string;
  cancellation_hours: NumInput;
  payment_methods: string;
  booking_terms: string;
}

type SeedChoice = { starter_library: boolean; sample_client: boolean };

const STEPS = ['Your business', 'How you work', 'Booking & payments', 'Head start', 'Ready'];

const defaultTerms = (depositDollars: string, hours: NumInput) => {
  const parts: string[] = [];
  if (Number(depositDollars) > 0) {
    parts.push(`A $${depositDollars} deposit holds your session date and is applied to your first invoice.`);
    parts.push(`Cancellations within ${hours} hours of your scheduled session forfeit the deposit.`);
  } else {
    parts.push(`Please give at least ${hours} hours’ notice if you need to cancel or reschedule.`);
  }
  parts.push('We may cancel for safety or allergen concerns, with a full refund of anything paid.');
  return parts.join(' ');
};

function StepHeader({ step }: { step: number }) {
  return (
    <div className="mb-6">
      <div className="flex gap-1.5 mb-3" aria-hidden="true">
        {STEPS.map((s, i) => (
          <div key={s} className={`h-1.5 flex-1 rounded-full ${i <= step ? 'bg-herb' : 'bg-gray-200'}`} />
        ))}
      </div>
      <div className="text-xs font-semibold text-gray-400 uppercase tracking-wide">
        Step {step + 1} of {STEPS.length} · {STEPS[step]}
      </div>
    </div>
  );
}

function Label({ children, hint }: { children: ReactNode; hint?: ReactNode }) {
  return (
    <span className="block mb-1.5">
      <span className="text-sm font-medium text-gray-700">{children}</span>
      {hint && <span className="block text-xs text-gray-400 mt-0.5">{hint}</span>}
    </span>
  );
}

interface NavProps {
  onBack?: () => void;
  onNext: () => void;
  nextLabel?: string;
  busy?: boolean;
  canSkip?: boolean;
  onSkip?: () => void;
}

function Nav({ onBack, onNext, nextLabel = 'Continue', busy, canSkip, onSkip }: NavProps) {
  return (
    <div className="flex items-center gap-3 mt-8">
      {onBack && (
        <button type="button" onClick={onBack} className="px-5 py-3 rounded-xl border border-gray-200 bg-white text-sm font-semibold text-gray-600">
          Back
        </button>
      )}
      {canSkip && (
        <button type="button" onClick={onSkip} className="text-sm text-gray-500 underline ml-auto mr-2">Skip for now</button>
      )}
      <button
        type="button"
        onClick={onNext}
        disabled={busy}
        className={`px-6 py-3 rounded-xl bg-herb text-white font-semibold text-sm disabled:opacity-60 ${canSkip ? '' : 'ml-auto'}`}
      >
        {busy ? 'Saving…' : nextLabel}
      </button>
    </div>
  );
}

export default function Onboarding() {
  const { refreshSettings } = useAppSettings();
  const logoRef = useRef<HTMLInputElement>(null);

  const [step, setStep] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [copied, setCopied] = useState(false);
  const [slug, setSlug] = useState('');
  const [termsTouched, setTermsTouched] = useState(false);
  const [seed, setSeed] = useState<SeedChoice>({ starter_library: true, sample_client: true });
  const [seedResult, setSeedResult] = useState<OnboardingSeedResponse | null>(null);
  const [form, setForm] = useState<OnboardingForm>({
    business_name: '', chef_name: '', phone: '', location: '', website: '', logo_base64: null,
    service_types: [],
    default_session_window: 3, default_serving_size: 4, default_service_fee: '',
    deposit: '0', cancellation_hours: 48, payment_methods: '', booking_terms: '',
  });

  const set = (k: keyof OnboardingForm) => (e: ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => setForm(prev => ({ ...prev, [k]: e.target.value }));

  useEffect(() => {
    apiCall<Settings>('/api/settings').then(s => {
      setSlug(s.slug || '');
      setForm(prev => ({
        ...prev,
        business_name: s.business_name || '',
        chef_name: s.chef_name || '',
        phone: s.phone || '',
        location: s.location || '',
        website: s.website || '',
        logo_base64: s.logo_base64 || null,
        service_types: Array.isArray(s.service_types) ? s.service_types : [],
        default_session_window: s.default_session_window || 3,
        default_serving_size: s.default_serving_size || 4,
        default_service_fee: s.default_service_fee || '',
        deposit: centsToDollars(s.deposit_cents),
        cancellation_hours: s.cancellation_hours ?? 48,
        payment_methods: s.payment_methods || '',
        booking_terms: s.booking_terms || '',
      }));
      if (s.booking_terms) setTermsTouched(true);
    }).catch(() => {});
  }, []);

  // Keep the suggested terms in sync with deposit / window until the chef edits them
  useEffect(() => {
    if (!termsTouched) {
      setForm(prev => ({ ...prev, booking_terms: defaultTerms(prev.deposit, prev.cancellation_hours) }));
    }
  }, [form.deposit, form.cancellation_hours, termsTouched]);

  const save = useCallback(async (fields: Partial<Record<keyof Settings, unknown>>) => {
    setBusy(true);
    setError('');
    try {
      const s = await apiCall<Settings>('/api/settings', 'PUT', fields);
      setSlug(s.slug || '');
      refreshSettings?.();
      return true;
    } catch (err) {
      setError(errorMessage(err));
      return false;
    } finally {
      setBusy(false);
    }
  }, [refreshSettings]);

  const next = () => { setError(''); setStep(s => Math.min(s + 1, STEPS.length - 1)); window.scrollTo(0, 0); };
  const back = () => { setError(''); setStep(s => Math.max(s - 1, 0)); };

  const saveBusiness = async () => {
    if (!form.business_name.trim()) { setError('Please enter your business name.'); return; }
    const ok = await save({
      business_name: form.business_name, chef_name: form.chef_name, phone: form.phone,
      location: form.location, website: form.website, logo_base64: form.logo_base64,
    });
    if (ok) next();
  };

  const saveServices = async () => {
    const ok = await save({
      service_types: form.service_types,
      default_session_window: form.default_session_window,
      default_serving_size: form.default_serving_size,
      default_service_fee: form.default_service_fee === '' ? 0 : form.default_service_fee,
    });
    if (ok) next();
  };

  const saveBooking = async () => {
    const ok = await save({
      deposit_cents: dollarsToCents(form.deposit),
      cancellation_hours: form.cancellation_hours,
      payment_methods: form.payment_methods,
      booking_terms: form.booking_terms,
    });
    if (ok) next();
  };

  const runSeed = async () => {
    if (!seed.starter_library && !seed.sample_client) { next(); return; }
    setBusy(true);
    setError('');
    try {
      setSeedResult(await apiCall<OnboardingSeedResponse>('/api/onboarding/seed', 'POST', seed));
      next();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const finish = async (to: string) => {
    setBusy(true);
    try {
      await apiCall('/api/onboarding/complete', 'POST');
      // Full reload so the auth gate re-reads onboarding state
      window.location.href = to;
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  const handleLogo = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (file.size > 2 * 1024 * 1024) { setError('Logo must be under 2 MB.'); return; }
    const reader = new FileReader();
    // readAsDataURL always yields a string
    reader.onload = (ev) => setForm(prev => ({ ...prev, logo_base64: ev.target?.result as string }));
    reader.readAsDataURL(file);
  };

  const toggleService = (label: string) => setForm(prev => ({
    ...prev,
    service_types: prev.service_types.includes(label)
      ? prev.service_types.filter(t => t !== label)
      : [...prev.service_types, label],
  }));

  const link = bookingUrl(slug);
  const copyLink = () => {
    navigator.clipboard?.writeText(link).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }).catch(() => {});
  };

  return (
    <div className="min-h-dvh bg-ivory font-sans px-4 py-8">
      <div className="max-w-xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <div className="font-display text-lg font-bold text-green-dark">🍳 {APP_NAME}</div>
          {step < STEPS.length - 1 && (
            <button type="button" onClick={() => finish('/')} className="text-xs text-gray-400 underline">
              Finish later
            </button>
          )}
        </div>

        <div className="bg-white rounded-2xl shadow-card p-6 sm:p-8">
          <StepHeader step={step} />
          {error && <div className="border rounded-lg px-3.5 py-3 mb-5 text-sm bg-red-50 border-red-200 text-red-700" role="alert">{error}</div>}

          {step === 0 && (
            <>
              <h1 className="font-display text-2xl font-semibold text-olive mb-1">Let’s set up your business</h1>
              <p className="text-sm text-gray-500 mb-6">This appears on your booking page, meal-plan PDFs and invoices.</p>
              <label className="block mb-4"><Label>Business name *</Label>
                <input type="text" value={form.business_name} onChange={set('business_name')} maxLength={120} autoFocus />
              </label>
              <label className="block mb-4"><Label>Your name</Label>
                <input type="text" value={form.chef_name} onChange={set('chef_name')} maxLength={120} autoComplete="name" />
              </label>
              <div className="grid sm:grid-cols-2 gap-x-4">
                <label className="block mb-4"><Label>Phone</Label>
                  <input type="tel" value={form.phone} onChange={set('phone')} autoComplete="tel" />
                </label>
                <label className="block mb-4"><Label>City / service area</Label>
                  <input type="text" value={form.location} onChange={set('location')} placeholder="e.g. Austin, TX" />
                </label>
              </div>
              <label className="block mb-4"><Label>Website</Label>
                <input type="text" value={form.website} onChange={set('website')} placeholder="https://" />
              </label>
              <div>
                <Label hint="PNG or JPG, under 2 MB. Optional.">Logo</Label>
                <div className="flex items-center gap-3">
                  {form.logo_base64
                    ? <img src={form.logo_base64} alt="Logo preview" className="h-14 w-14 object-contain rounded-lg border border-gray-200" />
                    : <div className="h-14 w-14 rounded-lg bg-sage flex items-center justify-center text-2xl" aria-hidden="true">🍳</div>}
                  <button type="button" onClick={() => logoRef.current?.click()} className="text-sm font-semibold text-herb underline">
                    {form.logo_base64 ? 'Change' : 'Upload logo'}
                  </button>
                  {form.logo_base64 && (
                    <button type="button" onClick={() => setForm(p => ({ ...p, logo_base64: null }))} className="text-sm text-gray-400 underline">Remove</button>
                  )}
                  <input ref={logoRef} type="file" accept="image/*" className="hidden" onChange={handleLogo} />
                </div>
              </div>
              <Nav onNext={saveBusiness} busy={busy} />
            </>
          )}

          {step === 1 && (
            <>
              <h1 className="font-display text-2xl font-semibold text-olive mb-1">How do you work?</h1>
              <p className="text-sm text-gray-500 mb-5">Pick everything that applies. It helps tailor menu suggestions.</p>
              <div className="grid sm:grid-cols-2 gap-2.5 mb-7">
                {SERVICE_TYPES.map(t => {
                  const on = form.service_types.includes(t.label);
                  return (
                    <button
                      key={t.id}
                      type="button"
                      aria-pressed={on}
                      onClick={() => toggleService(t.label)}
                      className={`flex items-center gap-3 text-left px-4 py-3 rounded-xl border-2 text-sm font-medium transition-colors ${on ? 'border-herb bg-sage text-green-dark' : 'border-gray-200 bg-white text-gray-700'}`}
                    >
                      <span className="text-xl" aria-hidden="true">{t.icon}</span>{t.label}
                    </button>
                  );
                })}
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                <label className="block"><Label hint="Hours per visit">Typical session</Label>
                  <input type="number" min="0.5" max="24" step="0.5" value={form.default_session_window} onChange={set('default_session_window')} />
                </label>
                <label className="block"><Label hint="Portions per dish">Servings</Label>
                  <input type="number" min="1" max="100" value={form.default_serving_size} onChange={set('default_serving_size')} />
                </label>
                <label className="block"><Label hint="Per session, $">Service fee</Label>
                  <input type="number" min="0" step="1" value={form.default_service_fee} onChange={set('default_service_fee')} placeholder="0" />
                </label>
              </div>
              <Nav onBack={back} onNext={saveServices} busy={busy} />
            </>
          )}

          {step === 2 && (
            <>
              <h1 className="font-display text-2xl font-semibold text-olive mb-1">Booking &amp; payments</h1>
              <p className="text-sm text-gray-500 mb-6">Shown to clients on your booking form before they submit.</p>
              <div className="grid sm:grid-cols-2 gap-x-4">
                <label className="block mb-4"><Label hint="$0 = no deposit">Booking deposit ($)</Label>
                  <input type="number" min="0" step="5" value={form.deposit} onChange={set('deposit')} />
                </label>
                <label className="block mb-4"><Label hint="Hours before a session">Cancellation notice</Label>
                  <input type="number" min="0" max="720" value={form.cancellation_hours} onChange={set('cancellation_hours')} />
                </label>
              </div>
              <label className="block mb-4"><Label hint="How clients can pay you">Payment methods you accept</Label>
                <input type="text" value={form.payment_methods} onChange={set('payment_methods')} placeholder="e.g. Venmo, Zelle, card, or check" maxLength={300} />
              </label>
              <label className="block"><Label hint="Edit freely. Clients must agree to these before booking.">Booking terms</Label>
                <textarea
                  rows={5}
                  value={form.booking_terms}
                  onChange={e => { setTermsTouched(true); set('booking_terms')(e); }}
                  maxLength={4000}
                />
              </label>
              {termsTouched && (
                <button type="button" className="text-xs text-herb underline mt-1" onClick={() => setTermsTouched(false)}>
                  Reset to suggested terms
                </button>
              )}
              <Nav onBack={back} onNext={saveBooking} busy={busy} />
            </>
          )}

          {step === 3 && (
            <>
              <h1 className="font-display text-2xl font-semibold text-olive mb-1">Get a head start</h1>
              <p className="text-sm text-gray-500 mb-6">Start with some content so you can explore right away. You can delete any of it later.</p>
              {([
                ['starter_library', '📚', 'Starter recipe library', '20 meal-prep-friendly recipes with ingredients, plus 4 ready-made menu templates.'],
                ['sample_client', '👨‍👩‍👧', 'A sample client', 'A demo family with allergies and preferences, to try planning a session. Remove it any time in Settings.'],
              ] as Array<[keyof SeedChoice, string, string, string]>).map(([key, icon, title, body]) => (
                <label key={key} className={`flex gap-3 items-start p-4 rounded-xl border-2 mb-3 cursor-pointer ${seed[key] ? 'border-herb bg-sage' : 'border-gray-200'}`}>
                  <input type="checkbox" className="mt-1" checked={seed[key]} onChange={e => setSeed(s => ({ ...s, [key]: e.target.checked }))} />
                  <span>
                    <span className="block font-semibold text-sm text-olive"><span aria-hidden="true">{icon}</span> {title}</span>
                    <span className="block text-xs text-gray-500 mt-0.5 leading-relaxed">{body}</span>
                  </span>
                </label>
              ))}
              <Nav onBack={back} onNext={runSeed} busy={busy} nextLabel={seed.starter_library || seed.sample_client ? 'Add & continue' : 'Continue'} />
            </>
          )}

          {step === 4 && (
            <>
              <div className="text-center">
                <div className="text-5xl mb-3" aria-hidden="true">🎉</div>
                <h1 className="font-display text-2xl font-semibold text-olive mb-1">You’re ready to cook</h1>
                <p className="text-sm text-gray-500 mb-6">
                  {seedResult?.library?.recipes ? `${seedResult.library.recipes} recipes added. ` : ''}
                  Share your booking link and new clients will land in your Intake inbox.
                </p>
              </div>
              <div className="bg-sage rounded-xl p-4 mb-6">
                <div className="text-xs font-semibold text-green-dark uppercase tracking-wide mb-2">Your booking link</div>
                <div className="flex gap-2">
                  <input type="text" readOnly value={link} onFocus={e => e.target.select()} className="text-sm bg-white" aria-label="Booking link" />
                  <button type="button" onClick={copyLink} className="shrink-0 px-4 rounded-lg bg-herb text-white text-sm font-semibold">
                    {copied ? 'Copied!' : 'Copy'}
                  </button>
                </div>
                <a href={link} target="_blank" rel="noreferrer" className="inline-block text-xs text-green-dark underline mt-2">Preview it →</a>
              </div>
              <div className="text-xs font-semibold text-gray-400 uppercase tracking-wide mb-2">Next steps</div>
              <div className="grid gap-2">
                {[
                  ['/clients/new', '👤', 'Add your first client'],
                  ['/recipes/import', '✨', 'Import a favorite recipe'],
                  ['/sessions/new', '🗓️', 'Plan a session'],
                ].map(([to, icon, label]) => (
                  <button key={to} type="button" disabled={busy} onClick={() => finish(to)}
                    className="flex items-center gap-3 px-4 py-3 rounded-xl border border-gray-200 text-sm font-medium text-olive text-left hover:border-herb">
                    <span aria-hidden="true">{icon}</span>{label}<span className="ml-auto text-gray-300">›</span>
                  </button>
                ))}
              </div>
              <Nav onNext={() => finish('/')} busy={busy} nextLabel="Go to my dashboard" />
            </>
          )}
        </div>
        {step === 0 && (
          <p className="text-center text-xs text-gray-400 mt-4">
            Not you? <button type="button" className="underline" onClick={() => { fetch('/api/auth/logout', { method: 'POST' }).finally(() => { window.location.href = '/login'; }); }}>Sign out</button>
          </p>
        )}
      </div>
    </div>
  );
}
