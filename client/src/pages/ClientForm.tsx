// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, type ChangeEvent, type ReactNode } from 'react';
import { useNavigate, useParams } from 'react-router';
import PageHeader from '../components/PageHeader';
import { useToast } from '../components/Toast';
import { apiCall, errorMessage } from '../hooks/useApi';
import { ALLERGENS_LIST, ALLERGEN_ICONS, DIETARY_STYLES, PROTEINS, EQUIPMENT_OPTIONS } from '../constants';
import type { Client } from '../types';

/** Editable client fields. Loaded rows can carry nulls in the optional text columns. */
type ClientFormData = Pick<Client,
  | 'phone' | 'email' | 'address' | 'household_type' | 'disliked_foods' | 'spice_tolerance'
  | 'stovetop_type' | 'parking_notes' | 'preferred_day' | 'preferred_time' | 'grocery_handling' | 'chef_notes'
  | 'allergens' | 'dietary_styles' | 'liked_proteins' | 'equipment'
> & {
  name: string;
  adults: number | null;
  children: number | null;
  serving_size: number | null;
  oven_available: number | null;
  client_containers: number | null;
};

type SectionKey = 'A' | 'B' | 'C' | 'D';
type FieldErrors = Partial<Record<'name' | 'email' | 'phone', string>>;

const DAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
const SERVING_SIZE_OPTIONS = [2, 4, 6, 8];

// ─── Sub-components ───────────────────────────────────────────────────────────

const Section = ({ title, open, onToggle, children }: { title: ReactNode; open: boolean; onToggle: () => void; children: ReactNode }) => (
  <div className="card mb-3">
    <button
      className="w-full flex items-center justify-between text-left"
      onClick={onToggle}
      style={{ minHeight: 'auto' }}
    >
      <span className="font-display font-semibold text-green-dark">{title}</span>
      <span className={`text-gray-400 transition-transform ${open ? 'rotate-180' : ''}`}>▾</span>
    </button>
    {open && <div className="mt-4 space-y-4">{children}</div>}
  </div>
);

interface ChipGroupProps {
  options: string[];
  selected: string[];
  onChange: (next: string[]) => void;
  iconMap?: Record<string, string>;
}

const ChipGroup = ({ options, selected, onChange, iconMap }: ChipGroupProps) => (
  <div className="flex flex-wrap gap-2">
    {options.map(o => {
      const active = selected.includes(o);
      return (
        <button
          key={o}
          onClick={() => onChange(active ? selected.filter(x => x !== o) : [...selected, o])}
          className={`chip ${active ? 'chip-active' : 'chip-inactive'}`}
          style={{ minHeight: '36px' }}
        >
          {iconMap && iconMap[o] && <span className="mr-1">{iconMap[o]}</span>}
          {o}
        </button>
      );
    })}
  </div>
);

const Segmented = ({ options, value, onChange }: { options: string[]; value: string | null; onChange: (value: string) => void }) => (
  <div className="flex rounded-xl overflow-hidden border border-gray-200">
    {options.map(o => (
      <button
        key={o}
        onClick={() => onChange(o)}
        className={`flex-1 py-2.5 text-sm font-medium transition-colors ${
          value === o ? 'bg-navy text-white' : 'bg-white text-gray-600'
        }`}
        style={{ minHeight: '44px' }}
      >
        {o}
      </button>
    ))}
  </div>
);

const Label = ({ children, required }: { children: ReactNode; required?: boolean }) => (
  <label className="block text-sm font-medium text-gray-700 mb-1.5">
    {children}{required && <span className="text-danger ml-1">*</span>}
  </label>
);

// Container notice modal — shown when chef provides containers
function ContainerNoticeModal({ onClose }: { onClose: () => void }) {
  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={e => e.stopPropagation()}>
        <div className="flex items-start gap-3 mb-4">
          <span className="text-2xl">📦</span>
          <div>
            <h3 className="font-display font-bold text-green-dark text-lg leading-tight">
              Chef-Provided Containers
            </h3>
            <p className="text-xs text-gray-500 mt-0.5">What this means for your session</p>
          </div>
        </div>

        <div className="space-y-3 text-sm text-gray-700">
          <p>
            When the client does <strong>not</strong> provide their own containers, the chef will
            supply food-safe, BPA-free glass or reusable containers for all prepared dishes.
          </p>
          <div className="bg-amber-50 border border-amber-200 rounded-xl p-3">
            <p className="font-semibold text-amber-800 mb-1">💰 Additional Cost</p>
            <p className="text-amber-700 text-xs">
              A container fee of <strong>$15–$25</strong> is added to the session cost depending
              on the number of dishes. This covers containers, lids, and labels. The exact amount
              will be confirmed when booking.
            </p>
          </div>
          <p className="text-xs text-gray-500">
            To avoid this fee, the client can provide their own oven-safe, dishwasher-safe
            containers with lids before the session begins.
          </p>
        </div>

        <button
          onClick={onClose}
          className="mt-5 w-full py-3 bg-navy text-white rounded-xl font-semibold text-sm"
        >
          Got it
        </button>
      </div>
    </div>
  );
}

// ─── Main component ───────────────────────────────────────────────────────────

export default function ClientForm() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { show } = useToast();
  const isEdit = !!id;

  const [sections, setSections]             = useState<Record<SectionKey, boolean>>({ A: true, B: false, C: false, D: false });
  const [saving, setSaving]                 = useState(false);
  const [errors, setErrors]                 = useState<FieldErrors>({});
  const [showContainerNotice, setShowContainerNotice] = useState(false);
  const [customServing, setCustomServing]   = useState(false);

  const [form, setForm] = useState<ClientFormData>({
    name: '', phone: '', email: '', address: '',
    household_type: 'Couple', adults: 2, children: 0, serving_size: 4,
    allergens: [], dietary_styles: [], liked_proteins: [],
    disliked_foods: '', spice_tolerance: 'No Preference',
    oven_available: 1, stovetop_type: 'Gas', equipment: [],
    client_containers: 0, parking_notes: '',
    preferred_day: '', preferred_time: '', grocery_handling: 'Chef shops',
    chef_notes: '',
  });

  useEffect(() => {
    if (isEdit) {
      fetch(`/api/clients/${id}`)
        .then(r => r.text()).then(t => t ? JSON.parse(t) : {})
        .then((d: Partial<Client>) => {
          if (d.id) {
            setForm(prev => ({ ...prev, ...d }));
            // If stored serving size isn't a preset, show custom input
            if (d.serving_size && !SERVING_SIZE_OPTIONS.includes(d.serving_size)) {
              setCustomServing(true);
            }
          }
        });
    }
  }, [id]);

  const set = <K extends keyof ClientFormData>(field: K, val: ClientFormData[K]) => setForm(prev => ({ ...prev, [field]: val }));

  // ── Validation ──────────────────────────────────────────────────────────────
  const validate = () => {
    const e: FieldErrors = {};
    if (!form.name.trim()) {
      e.name = 'Full name is required';
    }
    if (form.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(form.email.trim())) {
      e.email = 'Enter a valid email address';
    }
    if (form.phone && !/^[\d\s\-().+]{7,}$/.test(form.phone.trim())) {
      e.phone = 'Enter a valid phone number';
    }
    setErrors(e);
    return Object.keys(e).length === 0;
  };

  // ── Submit ───────────────────────────────────────────────────────────────────
  const handleSubmit = async () => {
    if (!validate()) {
      show('Please fix the highlighted fields', 'error');
      return;
    }
    setSaving(true);
    try {
      const method = isEdit ? 'PUT' : 'POST';
      const url    = isEdit ? `/api/clients/${id}` : '/api/clients';
      await apiCall(url, method, form);
      show(isEdit ? 'Client updated' : 'Client saved', 'success');
      navigate('/clients');
    } catch (err) {
      const msg = (errorMessage(err) || '').toLowerCase();
      if (msg.includes('unique') || msg.includes('duplicate') || msg.includes('already exists')) {
        show('A client with this email already exists', 'error');
      } else if (msg.includes('name') && msg.includes('required')) {
        show('Client name is required', 'error');
      } else if (msg.includes('network') || msg.includes('failed to fetch') || msg.includes('load')) {
        show('Network error — check your connection and try again', 'error');
      } else if (msg.includes('500') || msg.includes('server')) {
        show('Server error — please try again in a moment', 'error');
      } else {
        show(errorMessage(err) || 'Could not save client — please try again', 'error');
      }
      console.error('[ClientForm] save error:', err);
    } finally {
      setSaving(false);
    }
  };

  const toggle = (key: SectionKey) => setSections(prev => ({ ...prev, [key]: !prev[key] }));

  // ── Serving size helpers ─────────────────────────────────────────────────────
  const handleServingSizeChange = (e: ChangeEvent<HTMLSelectElement>) => {
    const val = e.target.value;
    if (val === 'custom') {
      setCustomServing(true);
    } else {
      setCustomServing(false);
      set('serving_size', parseInt(val));
    }
  };

  const selectedPreset = SERVING_SIZE_OPTIONS.includes(form.serving_size as number) ? String(form.serving_size) : 'custom';

  // ── Container toggle ─────────────────────────────────────────────────────────
  const handleContainerChange = (v: string) => {
    const isNo = v === 'No';
    set('client_containers', isNo ? 0 : 1);
    if (isNo) setShowContainerNotice(true);
  };

  return (
    <div className="page">
      <PageHeader
        title={isEdit ? 'Edit Client' : 'New Client'}
        back="/clients"
      />

      {showContainerNotice && (
        <ContainerNoticeModal onClose={() => setShowContainerNotice(false)} />
      )}

      <div className="px-4 pt-4 pb-24">

        {/* ── Section A: Personal Info ────────────────────────────── */}
        <Section title="A — Personal Info" open={sections.A} onToggle={() => toggle('A')}>
          <div>
            <Label required>Full Name</Label>
            <input
              value={form.name}
              onChange={e => set('name', e.target.value)}
              placeholder="e.g., The Chen Family"
              className={errors.name ? 'border-danger' : ''}
            />
            {errors.name && <p className="text-danger text-xs mt-1">{errors.name}</p>}
          </div>
          <div>
            <Label>Phone</Label>
            <input
              type="tel"
              value={form.phone || ''}
              onChange={e => set('phone', e.target.value)}
              placeholder="(925) 555-0100"
              className={errors.phone ? 'border-danger' : ''}
            />
            {errors.phone && <p className="text-danger text-xs mt-1">{errors.phone}</p>}
          </div>
          <div>
            <Label>Email</Label>
            <input
              type="email"
              value={form.email || ''}
              onChange={e => set('email', e.target.value)}
              placeholder="client@email.com"
              className={errors.email ? 'border-danger' : ''}
            />
            {errors.email && <p className="text-danger text-xs mt-1">{errors.email}</p>}
          </div>
          <div>
            <Label>Address (for travel reference only)</Label>
            <input
              value={form.address || ''}
              onChange={e => set('address', e.target.value)}
              placeholder="123 Main St, City, State"
            />
          </div>
          <div>
            <Label>Household Type</Label>
            <Segmented
              options={['Individual', 'Couple', 'Small Family', 'Large Family']}
              value={form.household_type}
              onChange={v => set('household_type', v)}
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <Label>Adults</Label>
              <input type="number" min="1" max="20" value={form.adults as number}
                onChange={e => set('adults', parseInt(e.target.value))} />
            </div>
            <div>
              <Label>Children</Label>
              <input type="number" min="0" max="20" value={form.children as number}
                onChange={e => set('children', parseInt(e.target.value))} />
            </div>
          </div>
          <div>
            <Label>Preferred Serving Size</Label>
            <select value={selectedPreset} onChange={handleServingSizeChange}>
              {SERVING_SIZE_OPTIONS.map(n => (
                <option key={n} value={String(n)}>{n} servings</option>
              ))}
              <option value="custom">Custom…</option>
            </select>
            {customServing && (
              <div className="mt-2">
                <input
                  type="number"
                  min="1"
                  max="200"
                  value={form.serving_size as number}
                  onChange={e => set('serving_size', parseInt(e.target.value) || 4)}
                  placeholder="Enter serving count"
                />
                <p className="text-xs text-gray-400 mt-1">Enter any number 1–200</p>
              </div>
            )}
          </div>
        </Section>

        {/* ── Section B: Dietary Profile ──────────────────────────── */}
        <Section title="B — Dietary Profile" open={sections.B} onToggle={() => toggle('B')}>
          <div>
            <Label>Allergens</Label>
            <ChipGroup options={ALLERGENS_LIST} selected={form.allergens}
              onChange={v => set('allergens', v)} iconMap={ALLERGEN_ICONS} />
          </div>
          <div>
            <Label>Dietary Style</Label>
            <ChipGroup options={DIETARY_STYLES} selected={form.dietary_styles}
              onChange={v => set('dietary_styles', v)} />
          </div>
          <div>
            <Label>Proteins They Like</Label>
            <ChipGroup options={PROTEINS} selected={form.liked_proteins}
              onChange={v => set('liked_proteins', v)} />
          </div>
          <div>
            <Label>Foods to Avoid (free text)</Label>
            <textarea rows={2} value={form.disliked_foods || ''}
              onChange={e => set('disliked_foods', e.target.value)}
              placeholder="e.g., mushrooms, cilantro, olives" />
          </div>
          <div>
            <Label>Spice Tolerance</Label>
            <Segmented
              options={['Mild', 'Medium', 'Bold', 'No Preference']}
              value={form.spice_tolerance}
              onChange={v => set('spice_tolerance', v)}
            />
          </div>
          <div>
            <Label>Additional Dietary Notes</Label>
            <textarea rows={2} value={form.chef_notes || ''}
              onChange={e => set('chef_notes', e.target.value)} />
          </div>
        </Section>

        {/* ── Section C: Kitchen Info ─────────────────────────────── */}
        <Section title="C — Kitchen Info" open={sections.C} onToggle={() => toggle('C')}>
          <div>
            <Label>Oven Available</Label>
            <Segmented options={['Yes', 'No']}
              value={form.oven_available ? 'Yes' : 'No'}
              onChange={v => set('oven_available', v === 'Yes' ? 1 : 0)} />
          </div>
          <div>
            <Label>Stovetop Type</Label>
            <Segmented options={['Gas', 'Electric', 'Induction']}
              value={form.stovetop_type || 'Gas'}
              onChange={v => set('stovetop_type', v)} />
          </div>
          <div>
            <Label>Equipment Available</Label>
            <ChipGroup options={EQUIPMENT_OPTIONS} selected={form.equipment}
              onChange={v => set('equipment', v)} />
          </div>
          <div>
            <Label>Client Provides Containers</Label>
            <Segmented
              options={['Yes', 'No']}
              value={form.client_containers == null ? null : form.client_containers ? 'Yes' : 'No'}
              onChange={handleContainerChange}
            />
            {form.client_containers === 0 && (
              <div className="mt-2 flex items-center gap-2 text-xs text-amber-700 bg-amber-50 border border-amber-200 rounded-xl px-3 py-2">
                <span>📦</span>
                <span>Chef will supply containers — <button
                  className="underline font-medium"
                  style={{ minHeight: 'auto' }}
                  onClick={() => setShowContainerNotice(true)}
                >see details & cost</button></span>
              </div>
            )}
          </div>
          <div>
            <Label>Parking Notes</Label>
            <input
              value={form.parking_notes || ''}
              onChange={e => set('parking_notes', e.target.value)}
              placeholder="e.g., street parking, driveway" />
          </div>
        </Section>

        {/* ── Section D: Service Preferences ─────────────────────── */}
        <Section title="D — Service Preferences" open={sections.D} onToggle={() => toggle('D')}>
          <div>
            <Label>Preferred Session Day</Label>
            <select value={form.preferred_day || ''} onChange={e => set('preferred_day', e.target.value)}>
              <option value="">Select day</option>
              {DAYS.map(d => <option key={d} value={d}>{d}</option>)}
            </select>
          </div>
          <div>
            <Label>Preferred Session Time</Label>
            <input type="time" value={form.preferred_time || ''}
              onChange={e => set('preferred_time', e.target.value)} />
          </div>
          <div>
            <Label>Grocery Purchasing</Label>
            <Segmented
              options={['Chef shops', 'Client shops', 'Split']}
              value={form.grocery_handling}
              onChange={v => set('grocery_handling', v)}
            />
            {form.grocery_handling === 'Chef shops' && (
              <p className="text-xs text-gray-500 mt-1.5">
                ✓ Chef handles all shopping — an extra shopping fee may apply.
              </p>
            )}
          </div>
          <div>
            <Label>Chef Notes</Label>
            <textarea rows={4} value={form.chef_notes || ''}
              onChange={e => set('chef_notes', e.target.value)}
              placeholder="Important notes about this client..." />
          </div>
        </Section>

        <button
          onClick={handleSubmit}
          disabled={saving}
          className="w-full py-3.5 bg-navy text-white rounded-xl font-semibold text-base disabled:opacity-60"
        >
          {saving ? 'Saving...' : isEdit ? 'Save Changes' : 'Create Client'}
        </button>
      </div>
    </div>
  );
}
