// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useRef, useState, useEffect, useContext, type ChangeEvent, type ReactNode } from 'react';
import PageHeader from '../components/PageHeader';
import ConfirmModal from '../components/ConfirmModal';
import { useToast } from '../components/Toast';
import { apiCall, errorMessage } from '../hooks/useApi';
import { SettingsContext } from '../App';
import { SERVICE_TYPES, APP_AUTHOR, APP_NAME, APP_REPO_URL, APP_TAGLINE, APP_VERSION, bookingUrl, dollarsToCents, centsToDollars, TIMEZONE_OPTIONS, browserTimeZone } from '../constants';
import type { Settings as SettingsData } from '../types';

const Label = ({ children, htmlFor }: { children: ReactNode; htmlFor?: string }) => (
  <label htmlFor={htmlFor} className="block text-sm font-medium text-gray-700 mb-1.5">{children}</label>
);

const SectionHeader = ({ children }: { children: ReactNode }) => (
  <div className="font-display text-base font-semibold text-green-dark mb-3 mt-1">{children}</div>
);

export default function Settings() {
  const { show } = useToast();
  const { refreshSettings } = useContext(SettingsContext);
  const [settings,     setSettings]     = useState<SettingsData | null>(null);
  const [aiStatus,     setAiStatus]     = useState<boolean | null>(null);
  const [deposit,      setDeposit]      = useState('0');
  const [testing,      setTesting]      = useState(false);
  const [clearConfirm, setClearConfirm] = useState(false);
  const [saving,       setSaving]       = useState(false);
  const [logoPreview,  setLogoPreview]  = useState<string | null>(null);
  const logoInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    fetch('/api/settings')
      .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
      .then((d: SettingsData) => {
        setSettings(d);
        setDeposit(centsToDollars(d.deposit_cents));
        setAiStatus(d.ai_configured);
        if (d.logo_base64) setLogoPreview(d.logo_base64);
      })
      .catch(() => {});
  }, []);

  // Inputs store what was typed (numbers can be strings mid-edit); the server validates on save
  const set = (field: keyof SettingsData, val: unknown) => setSettings(prev => ({ ...prev, [field]: val }) as SettingsData);

  const handleSave = async () => {
    if (!settings) return;
    setSaving(true);
    try {
      const { slug: _slug, account_email: _email, ai_configured: _ai, ...fields } = settings;
      const saved = await apiCall<SettingsData>('/api/settings', 'PUT', { ...fields, deposit_cents: dollarsToCents(deposit) });
      setSettings(saved);
      show('Settings saved', 'success');
      refreshSettings?.(); // Refresh global context so Dashboard header updates immediately
    } catch (err) {
      show(errorMessage(err) || 'Could not save settings', 'error');
    } finally {
      setSaving(false);
    }
  };

  // Turning a reminder on also fills in this device's time zone if none is set,
  // so "24 hours before" lines up with the chef's own clock.
  const setReminder = (field: 'remind_client' | 'remind_chef', on: boolean) => setSettings(prev => prev && ({
    ...prev,
    [field]: on ? 1 : 0,
    timezone: on && !prev.timezone ? (browserTimeZone() || prev.timezone) : prev.timezone,
  }));

  const toggleService = (label: string) => {
    if (!settings) return;
    const current = Array.isArray(settings.service_types) ? settings.service_types : [];
    set('service_types', current.includes(label) ? current.filter(t => t !== label) : [...current, label]);
  };

  const copyBookingLink = () => {
    navigator.clipboard?.writeText(bookingUrl(settings?.slug))
      .then(() => show('Booking link copied', 'success'))
      .catch(() => {});
  };

  const handleTestAI = async () => {
    setTesting(true);
    try {
      const result = await apiCall<{ configured: boolean; working: boolean }>('/api/ai/test', 'POST');
      if (result.working) {
        show('AI connection successful ✓', 'success');
        setAiStatus(true);
      } else {
        show('AI key invalid — check and retry', 'error');
      }
    } catch {
      show('AI not configured', 'error');
    } finally {
      setTesting(false);
    }
  };

  const handleLogoUpload = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (file.size > 2 * 1024 * 1024) {
      show('Logo file must be under 2 MB', 'error');
      return;
    }
    const reader = new FileReader();
    reader.onload = (ev) => {
      // readAsDataURL always yields a string
      const dataUrl = ev.target?.result as string;
      setLogoPreview(dataUrl);
      setSettings(prev => ({ ...prev, logo_base64: dataUrl }) as SettingsData);
    };
    reader.readAsDataURL(file);
  };

  const handleRemoveLogo = () => {
    setLogoPreview(null);
    setSettings(prev => ({ ...prev, logo_base64: null }) as SettingsData);
    if (logoInputRef.current) logoInputRef.current.value = '';
  };

  const handleClearSample = async () => {
    try {
      await apiCall('/api/settings/clear-sample', 'DELETE');
      show('Sample data cleared', 'success');
    } catch {
      show('Could not clear sample data', 'error');
    }
    setClearConfirm(false);
  };

  if (!settings) return (
    <div className="page">
      <PageHeader title="Settings" back="/" />
      <div className="px-4 pt-4 space-y-3">
        <div className="skeleton h-6 w-1/3" /><div className="skeleton h-12 w-full" /><div className="skeleton h-12 w-full" />
      </div>
    </div>
  );

  return (
    <div className="page">
      <PageHeader title="Settings" back="/" />

      <div className="px-4 pt-4 pb-28 space-y-4">
        {/* Business Info */}
        <div className="card">
          <SectionHeader>Business Info</SectionHeader>
          <div className="space-y-3">
            <div>
              <Label>Business Name</Label>
              <input value={settings.business_name || ''} onChange={e => set('business_name', e.target.value)} />
            </div>
            <div>
              <Label>Chef Name</Label>
              <input value={settings.chef_name || ''} onChange={e => set('chef_name', e.target.value)} placeholder="Your name" />
            </div>
            <div>
              <Label>Phone</Label>
              <input type="tel" value={settings.phone || ''} onChange={e => set('phone', e.target.value)} />
            </div>
            <div>
              <Label>Business Email</Label>
              <input type="email" value={settings.email || ''} onChange={e => set('email', e.target.value)} placeholder={settings.account_email} />
              <p className="text-xs text-gray-400 mt-1">Shown to clients and used for new-booking alerts. You sign in with {settings.account_email}.</p>
            </div>
            <div>
              <Label>Website</Label>
              <input value={settings.website || ''} onChange={e => set('website', e.target.value)} placeholder="yourchefwebsite.com" />
            </div>
            <div>
              <Label>Location / City</Label>
              <input value={settings.location || ''} onChange={e => set('location', e.target.value)} placeholder="e.g. Austin, TX" />
            </div>
          </div>
        </div>

        {/* Logo */}
        <div className="card">
          <SectionHeader>Business Logo</SectionHeader>
          <p className="text-xs text-gray-400 mb-3">
            Shown in the sidebar and on invoices. PNG or JPG, under 2 MB.
          </p>

          {logoPreview ? (
            <div className="flex items-center gap-4">
              <img
                src={logoPreview}
                alt="Business logo"
                className="w-20 h-20 object-contain rounded-xl border border-gray-200 bg-warm-white"
              />
              <div className="flex flex-col gap-2">
                <button
                  onClick={() => logoInputRef.current?.click()}
                  className="px-4 py-2 bg-navy text-white rounded-xl text-sm font-medium"
                  style={{ minHeight: '36px' }}
                >
                  Replace Logo
                </button>
                <button
                  onClick={handleRemoveLogo}
                  className="px-4 py-2 border border-gray-200 text-danger rounded-xl text-sm font-medium hover:bg-red-50 transition-colors"
                  style={{ minHeight: '36px' }}
                >
                  Remove Logo
                </button>
              </div>
            </div>
          ) : (
            <button
              onClick={() => logoInputRef.current?.click()}
              className="w-full py-8 border-2 border-dashed border-gray-200 rounded-xl text-center hover:border-green-mid hover:bg-green-light transition-colors"
              style={{ minHeight: 'auto' }}
            >
              <div className="text-3xl mb-2">🖼️</div>
              <div className="text-sm font-medium text-gray-600">Upload Logo</div>
              <div className="text-xs text-gray-400 mt-0.5">PNG or JPG · Max 2 MB</div>
            </button>
          )}

          <input
            ref={logoInputRef}
            type="file"
            accept="image/png,image/jpeg,image/jpg,image/webp"
            onChange={handleLogoUpload}
            className="hidden"
          />
        </div>

        {/* Booking & payments */}
        <div className="card">
          <SectionHeader>Booking &amp; Payments</SectionHeader>
          <div className="p-3 bg-green-light rounded-xl mb-4">
            <div className="text-xs font-semibold text-green-dark mb-1.5">Your booking link</div>
            <div className="flex items-center gap-2">
              <code className="text-xs bg-white px-2 py-1.5 rounded-lg border border-green-mid flex-1 overflow-hidden text-ellipsis whitespace-nowrap">
                {bookingUrl(settings.slug)}
              </code>
              <button onClick={copyBookingLink} className="px-3 py-1.5 bg-navy text-white text-xs rounded-lg font-medium" style={{ minHeight: 'auto' }}>
                Copy
              </button>
            </div>
          </div>
          <div className="space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <div>
                <Label>Booking Deposit ($)</Label>
                <input type="number" min="0" step="5" value={deposit} onChange={e => setDeposit(e.target.value)} />
              </div>
              <div>
                <Label>Cancellation Notice (hrs)</Label>
                <input type="number" min="0" max="720" value={settings.cancellation_hours ?? 48}
                  onChange={e => set('cancellation_hours', e.target.value)} />
              </div>
            </div>
            <div>
              <Label>Payment Methods Accepted</Label>
              <input value={settings.payment_methods || ''} onChange={e => set('payment_methods', e.target.value)} placeholder="e.g. Venmo, Zelle, card, or check" />
            </div>
            <div>
              <Label>Response Time (shown after a client books)</Label>
              <input value={settings.response_time_text || ''} onChange={e => set('response_time_text', e.target.value)} placeholder="within 24 hours" />
            </div>
            <div>
              <Label>Booking Terms</Label>
              <textarea rows={4} value={settings.booking_terms || ''} onChange={e => set('booking_terms', e.target.value)}
                placeholder="Deposit, cancellation and refund policy clients must accept before booking" />
            </div>
          </div>
        </div>

        {/* Services */}
        <div className="card">
          <SectionHeader>Services You Offer</SectionHeader>
          <div className="flex flex-wrap gap-2">
            {SERVICE_TYPES.map(t => {
              const on = (settings.service_types || []).includes(t.label);
              return (
                <button key={t.id} type="button" aria-pressed={on} onClick={() => toggleService(t.label)}
                  className={`chip ${on ? 'chip-active' : 'chip-inactive'}`}>
                  <span className="mr-1" aria-hidden="true">{t.icon}</span>{t.label}
                </button>
              );
            })}
          </div>
        </div>

        {/* Session Defaults */}
        <div className="card">
          <SectionHeader>Session Defaults</SectionHeader>
          <div className="space-y-3">
            <div>
              <Label>Default Session Window (hours)</Label>
              <input type="number" step="0.5" min="1" max="8"
                value={settings.default_session_window || 3.0}
                onChange={e => set('default_session_window', parseFloat(e.target.value))} />
            </div>
            <div>
              <Label>Default Serving Size</Label>
              <input type="number" min="1" max="100" value={settings.default_serving_size || 4}
                onChange={e => set('default_serving_size', parseInt(e.target.value, 10) || '')} />
            </div>
            <div>
              <Label>Default Service Fee</Label>
              <div className="relative">
                <span className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400">$</span>
                <input
                  type="number"
                  min="0"
                  step="10"
                  value={settings.default_service_fee || 0}
                  onChange={e => set('default_service_fee', parseFloat(e.target.value))}
                  className="pl-7"
                />
              </div>
            </div>
            <div>
              <Label>Invoice Number Prefix</Label>
              <input value={settings.invoice_prefix || ''} maxLength={8}
                onChange={e => set('invoice_prefix', e.target.value.toUpperCase().replace(/[^A-Z0-9]/g, ''))} placeholder="INV" />
              <p className="text-xs text-gray-400 mt-1">Invoices are numbered {settings.invoice_prefix || 'INV'}-{new Date().getFullYear()}-0001, 0002…</p>
            </div>
          </div>
        </div>

        {/* AI Configuration */}
        {/* Reminder emails */}
        <div className="card">
          <SectionHeader>Reminder Emails</SectionHeader>
          <p className="text-xs text-gray-500 mb-3">
            Sent automatically about 24 hours before each pending or confirmed session. Off until you turn them on.
          </p>
          <div className="space-y-3">
            {([
              ['remind_client', 'Remind clients', 'Date, time and menu. Replies go to your business email.'],
              ['remind_chef', 'Send me a prep sheet', 'Address, parking, phone, allergens and menu, to your business email.'],
            ] as const).map(([field, title, hint]) => (
              <label key={field} className="flex items-start gap-3 cursor-pointer">
                <input type="checkbox" className="mt-1 w-4 h-4 shrink-0" style={{ minHeight: 'auto' }}
                  checked={!!settings[field]} onChange={e => setReminder(field, e.target.checked)} />
                <span>
                  <span className="block text-sm font-medium text-gray-800">{title}</span>
                  <span className="block text-xs text-gray-500">{hint}</span>
                </span>
              </label>
            ))}
            <div>
              <Label htmlFor="timezone">Your time zone</Label>
              <select id="timezone" value={settings.timezone ?? ''} onChange={e => set('timezone', e.target.value || null)}>
                <option value="">Not set (UTC)</option>
                {[...new Set([settings.timezone, browserTimeZone()].filter((z): z is string => !!z))]
                  .filter(z => !TIMEZONE_OPTIONS.some(([v]) => v === z))
                  .map(z => <option key={z} value={z}>{z}</option>)}
                {TIMEZONE_OPTIONS.map(([v, label]) => <option key={v} value={v}>{label}</option>)}
              </select>
              <p className="text-xs text-gray-400 mt-1">Used for reminder times, and for "today", this month's totals and overdue invoices.</p>
            </div>
          </div>
        </div>

        <div className="card">
          <SectionHeader>AI Assistant</SectionHeader>
          <div className="space-y-3">
            <p className="text-xs text-gray-500">
              AI powers recipe import from links and text, and menu suggestions when planning a session.
            </p>
            <div className="flex items-center justify-between p-3 bg-warm-white rounded-xl">
              <div className="text-sm">
                <span className="font-medium">Status: </span>
                <span className={aiStatus ? 'text-green-dark font-semibold' : 'text-danger font-semibold'}>
                  {aiStatus ? '● Available' : '● Not available on this server'}
                </span>
              </div>
              <button
                onClick={handleTestAI}
                disabled={testing}
                className="px-3 py-2 bg-navy text-white rounded-xl text-xs font-medium disabled:opacity-60"
                style={{ minHeight: '36px' }}
              >
                {testing ? 'Testing...' : 'Test Connection'}
              </button>
            </div>

          </div>
        </div>

        {/* Data Management */}
        <div className="card">
          <SectionHeader>Data Management</SectionHeader>
          <div className="space-y-2">
            <a
              href="/api/settings/export"
              download
              className="flex items-center justify-between p-3 bg-warm-white rounded-xl text-sm font-medium text-navy hover:bg-blue-light transition-colors"
            >
              <span>Export All My Data (JSON)</span>
              <span>↓</span>
            </a>
            <a
              href="/api/costs/export-csv"
              download="costs.csv"
              className="flex items-center justify-between p-3 bg-warm-white rounded-xl text-sm font-medium text-navy hover:bg-blue-light transition-colors"
            >
              <span>Export Costs as CSV</span>
              <span>↓</span>
            </a>
            <button
              onClick={() => setClearConfirm(true)}
              className="w-full flex items-center justify-between p-3 bg-red-50 rounded-xl text-sm font-medium text-danger"
              style={{ minHeight: '50px' }}
            >
              <span>Clear Sample Data</span>
              <span>⚠</span>
            </button>
          </div>
        </div>

        {/* About */}
        <div className="card">
          <SectionHeader>About</SectionHeader>
          <div className="text-sm text-gray-500 space-y-1">
            <div>{APP_NAME} {APP_VERSION}: {APP_TAGLINE.toLowerCase()}</div>
            <div>
              Created by <a href={APP_REPO_URL} target="_blank" rel="noopener noreferrer" className="underline">{APP_AUTHOR}</a>.
              Source-available under the PolyForm Shield License 1.0.0.
            </div>
          </div>
        </div>

        <button onClick={handleSave} disabled={saving} className="w-full py-3.5 bg-navy text-white rounded-xl font-semibold text-base disabled:opacity-60">
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
      </div>

      <ConfirmModal
        open={clearConfirm}
        title="Clear Sample Data"
        message="This will delete The Chen Family and all sample data. This cannot be undone."
        confirmLabel="Clear Sample Data"
        onConfirm={handleClearSample}
        onCancel={() => setClearConfirm(false)}
      />
    </div>
  );
}
