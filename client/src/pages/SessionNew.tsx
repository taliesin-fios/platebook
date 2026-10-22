// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { format } from 'date-fns';
import { parseLocalDate } from '../utils/dateUtils';
import PageHeader from '../components/PageHeader';
import { useToast } from '../components/Toast';
import { apiCall, errorMessage } from '../hooks/useApi';
import { calcFeasibility, FEASIBILITY_COLORS, ALLERGEN_KEYWORDS, marginColor, REPEAT_OPTIONS, repeatLabel } from '../constants';
import { StarDisplay } from '../components/StarRating';
import type {
  ClientListItem, FromTemplateResponse, Id, MenuSuggestion, Recipe, SessionDetail, SessionListItem, SessionStatus, Settings,
} from '../types';

/** The new session being built across the wizard steps. */
interface SessionDraft {
  /** The <select> value: a client id, '' or 'new'. */
  client_id: string;
  session_date: string;
  session_time: string;
  serving_size: number;
  session_window_hours: number;
  status: SessionStatus;
  service_fee: number;
  ingredient_cost: number;
  chef_notes: string;
  recipe_ids: Id[];
  repeat_every_weeks: number | null;
}

const STEPS = ['Client', 'Schedule', 'Menu', 'Costing', 'Review'];

const CategoryBadge = ({ cat, small }: { cat: string | null; small?: boolean }) => (
  <span className={`${small ? 'text-[10px] px-1.5 py-0.5' : 'text-xs px-2 py-0.5'} rounded-full font-medium ${
    cat === 'Couples' ? 'bg-navy text-white' :
    cat === 'Families' ? 'bg-green-dark text-white' :
    cat === 'Plant-Based' ? 'bg-teal-700 text-white' : 'bg-purple-700 text-white'
  }`}>{cat}</span>
);

const checkAllergenConflict = (
  recipe: { name?: string | null; ingredients?: Array<{ name?: string | null }> },
  clientAllergens: string[] = [],
) => {
  if (!clientAllergens.length) return [];
  const conflicts: string[] = [];
  const ingredientNames = (recipe.ingredients || []).map(i => i.name?.toLowerCase() || '');
  const nameStr = (ingredientNames.join(' ') + ' ' + (recipe.name || '').toLowerCase());

  for (const allergen of clientAllergens) {
    const keywords = ALLERGEN_KEYWORDS[allergen] || [];
    if (keywords.some(kw => nameStr.includes(kw.toLowerCase()))) {
      conflicts.push(allergen);
    }
  }
  return conflicts;
};

export default function SessionNew() {
  const navigate = useNavigate();
  const { show } = useToast();
  const [params] = useSearchParams();
  const [step, setStep] = useState(0);
  const [clients, setClients] = useState<ClientListItem[]>([]);
  const [recipes, setRecipes] = useState<Recipe[]>([]);
  const [settings, setSettings] = useState<Partial<Settings>>({});
  const [aiStatus, setAiStatus] = useState(false);
  const [aiLoading, setAiLoading] = useState(false);
  const [aiReasoning, setAiReasoning] = useState('');
  const [saving, setSaving] = useState(false);
  const [allergenAcknowledged, setAllergenAcknowledged] = useState(false);
  const [prevSession, setPrevSession] = useState<SessionListItem | null>(null);
  const [loadingPrev, setLoadingPrev] = useState(false);

  const [session, setSession] = useState<SessionDraft>({
    client_id: params.get('client_id') || '',
    session_date: params.get('date') || '',
    session_time: '',
    serving_size: 4,
    session_window_hours: 3.0,
    status: 'Pending',
    service_fee: 0,
    ingredient_cost: 0,
    chef_notes: '',
    recipe_ids: [],
    repeat_every_weeks: null,
  });

  const [filters, setFilters] = useState({
    search: '', category: 'All', fit: 'All', hideConflicts: true,
  });

  useEffect(() => {
    Promise.all([
      fetch('/api/clients').then(r => r.ok ? r.json() : []),
      fetch('/api/recipes').then(r => r.ok ? r.json() : []),
      fetch('/api/settings').then(r => r.ok ? r.json() : {}),
      fetch('/api/ai/status').then(r => r.ok ? r.json() : {}),
    ]).then(([cl, rc, st, ai]: [unknown, unknown, Partial<Settings> | null, { configured?: boolean } | null]) => {
      setClients(Array.isArray(cl) ? cl : []);
      setRecipes(Array.isArray(rc) ? rc : []);
      setSettings(st || {});
      setAiStatus(ai?.configured || false);
      setSession(prev => ({
        ...prev,
        serving_size: st?.default_serving_size || 4,
        session_window_hours: st?.default_session_window || 3.0,
        service_fee: st?.default_service_fee || 0,
      }));
    });

    // Handle template pre-selection
    const templateId = params.get('template');
    if (templateId) {
      fetch(`/api/sessions/from-template/${templateId}`)
        .then(r => r.ok ? r.json() : {})
        .then((d: Partial<FromTemplateResponse>) => {
          const ids = d.recipe_ids;
          if (ids) setSession(prev => ({ ...prev, recipe_ids: ids }));
        });
    }
  }, []);

  // Fetch most-recent session for the selected client
  useEffect(() => {
    if (!session.client_id || session.client_id === 'new') {
      setPrevSession(null);
      return;
    }
    fetch(`/api/sessions?client_id=${session.client_id}`)
      .then(r => r.ok ? r.json() : [])
      .then(data => {
        const arr: SessionListItem[] = Array.isArray(data) ? data : [];
        setPrevSession(arr[0] || null); // sessions ordered DESC, first = most recent
      })
      .catch(() => setPrevSession(null));
  }, [session.client_id]);

  const selectedClient = clients.find(c => c.id === parseInt(session.client_id));
  const clientAllergens = selectedClient?.allergens || [];

  const selectedRecipes = recipes.filter(r => session.recipe_ids.includes(r.id));
  const feasibility = calcFeasibility(selectedRecipes);

  const toggleRecipe = (id: Id) => {
    setSession(prev => ({
      ...prev,
      recipe_ids: prev.recipe_ids.includes(id)
        ? prev.recipe_ids.filter(x => x !== id)
        : [...prev.recipe_ids, id],
    }));
  };

  const filteredRecipes = recipes.filter(r => {
    if (filters.search && !r.name.toLowerCase().includes(filters.search.toLowerCase())) return false;
    if (filters.category !== 'All' && r.category !== filters.category) return false;
    if (filters.fit !== 'All' && r.session_fit !== filters.fit) return false;
    if (filters.hideConflicts) {
      const conflicts = checkAllergenConflict(r, clientAllergens);
      if (conflicts.length > 0) return false;
    }
    return true;
  });

  const conflictCount = clientAllergens.length
    ? recipes.filter(r => checkAllergenConflict(r, clientAllergens).length > 0).length
    : 0;

  const margin = session.service_fee - session.ingredient_cost;
  // C20: when no fee entered yet, show neutral (not red) margin
  const marginPct = session.service_fee > 0 ? ((margin / session.service_fee) * 100).toFixed(1) : null;

  const handleAiSuggest = async () => {
    if (!session.client_id) { show('Select a client first', 'error'); return; }
    setAiLoading(true);
    try {
      const result = await apiCall<MenuSuggestion>('/api/ai/suggest-menu', 'POST', {
        client_id: session.client_id,
        serving_size: session.serving_size,
        session_window_hours: session.session_window_hours,
        dish_count: 3,
      });
      setSession(prev => ({ ...prev, recipe_ids: result.suggested_recipe_ids || [] }));
      setAiReasoning(result.reasoning || '');
    } catch (err) {
      if (errorMessage(err).includes('API_KEY_MISSING')) {
        show('AI not configured — add API key in Settings', 'error');
      } else {
        show('AI took too long — try again or select manually', 'error');
      }
    } finally {
      setAiLoading(false);
    }
  };

  const handleSaveTemplate = async () => {
    const name = window.prompt('Template name:');
    if (!name) return;
    try {
      await apiCall('/api/templates', 'POST', {
        name,
        description: `${selectedRecipes.map(r => r.name).join(', ')}`,
        recipe_ids: session.recipe_ids,
      });
      show('Template saved', 'success');
    } catch {
      show('Could not save template', 'error');
    }
  };

  const handleConfirm = async () => {
    if (session.recipe_ids.length === 0) { show('Add at least 1 dish', 'error'); return; }
    if (!session.client_id) { show('Select a client', 'error'); return; }

    // Check for allergen conflicts in selection
    const hasConflicts = selectedRecipes.some(r => checkAllergenConflict(r, clientAllergens).length > 0);
    if (hasConflicts && !allergenAcknowledged) {
      show('Please acknowledge the allergen conflict below', 'error');
      return;
    }

    setSaving(true);
    try {
      // C14: send client_id as a number, not a string from the <select>
      const payload = { ...session, client_id: parseInt(session.client_id, 10) };
      const saved = await apiCall<SessionDetail>('/api/sessions', 'POST', payload);
      show('Session created', 'success');
      navigate(`/sessions/${saved.id}`);
    } catch {
      show('Could not save session', 'error');
    } finally {
      setSaving(false);
    }
  };

  const hasAllergenConflicts = selectedRecipes.some(r => checkAllergenConflict(r, clientAllergens).length > 0);

  const canProceed = () => {
    if (step === 0) return !!session.client_id;
    if (step === 1) return !!session.session_date;
    if (step === 2) return session.recipe_ids.length > 0;
    return true;
  };

  return (
    <div className="page">
      <PageHeader title="New Session" back="/sessions" />

      {/* Progress bar */}
      <div className="flex bg-white border-b border-gray-100 sticky top-[57px] z-10">
        {STEPS.map((s, i) => (
          <button
            key={s}
            onClick={() => i < step && setStep(i)}
            className={`flex-1 py-2.5 text-[11px] font-medium transition-colors border-b-2 ${
              i === step ? 'text-navy border-navy' :
              i < step ? 'text-green-mid border-green-mid' :
              'text-gray-400 border-transparent'
            }`}
            style={{ minHeight: '44px' }}
          >
            {i < step ? '✓' : i + 1} {s}
          </button>
        ))}
      </div>

      <div className="px-4 pt-4 pb-36">
        {/* STEP 1 — Client */}
        {step === 0 && (
          <div className="space-y-4">
            <div className="card">
              <div className="section-header">Select Client</div>
              <select
                value={session.client_id}
                onChange={e => setSession(prev => ({ ...prev, client_id: e.target.value }))}
              >
                <option value="">— Choose a client —</option>
                {clients.map(c => (
                  <option key={c.id} value={c.id}>{c.name}</option>
                ))}
                <option value="new">+ Add New Client</option>
              </select>
              {session.client_id === 'new' && (
                <button onClick={() => navigate('/clients/new')} className="mt-2 text-navy-mid text-sm font-medium">
                  → Go to New Client form
                </button>
              )}
              {selectedClient && (
                <div className="mt-3 p-3 bg-blue-light rounded-xl text-sm">
                  <div className="font-semibold text-navy">{selectedClient.household_type}</div>
                  <div className="text-gray-600">Serves {selectedClient.serving_size} · {selectedClient.adults} adults{selectedClient.children! > 0 ? `, ${selectedClient.children} children` : ''}</div>
                  {clientAllergens.length > 0 && (
                    <div className="mt-1 text-danger text-xs font-medium">⚠ Allergens: {clientAllergens.join(', ')}</div>
                  )}
                </div>
              )}
            </div>

            {/* Previous menu suggestion */}
            {prevSession && session.recipe_ids.length === 0 && (
              <div className="card border border-green-mid">
                <div className="flex items-start justify-between mb-1">
                  <div className="text-sm font-semibold text-green-dark">📋 Repeat last menu?</div>
                  <button
                    onClick={() => setPrevSession(null)}
                    className="text-gray-400 text-lg leading-none"
                    style={{ minHeight: 'auto' }}
                  >×</button>
                </div>
                <p className="text-xs text-gray-500 mb-3">
                  {selectedClient?.name}'s last session on{' '}
                  <span className="font-medium text-navy">
                    {(() => { try { return format(parseLocalDate(prevSession.session_date), 'MMM d, yyyy'); } catch { return prevSession.session_date; } })()}
                  </span>
                  {' '}had <span className="font-medium text-navy">{prevSession.dish_count || 0} dishes</span>.
                </p>
                <button
                  disabled={loadingPrev}
                  onClick={async () => {
                    setLoadingPrev(true);
                    try {
                      const s: Partial<SessionDetail> = await fetch(`/api/sessions/${prevSession.id}`).then(r => r.json());
                      const ids = (s.dishes || []).map(d => d.id);
                      if (ids.length) {
                        setSession(prev => ({ ...prev, recipe_ids: ids }));
                        show(`Loaded ${ids.length} dishes from last session`, 'success');
                      } else {
                        show('No dishes found in that session', 'error');
                      }
                    } catch {
                      show('Could not load previous menu', 'error');
                    } finally {
                      setLoadingPrev(false);
                    }
                  }}
                  className="w-full py-2 bg-green-dark text-white rounded-xl text-sm font-semibold disabled:opacity-50"
                  style={{ minHeight: '40px' }}
                >
                  {loadingPrev ? 'Loading...' : 'Load Previous Menu'}
                </button>
              </div>
            )}
          </div>
        )}

        {/* STEP 2 — Schedule */}
        {step === 1 && (
          <div className="space-y-4">
            <div className="card space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1.5">Date *</label>
                <input type="date" value={session.session_date} onChange={e => setSession(prev => ({ ...prev, session_date: e.target.value }))} />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1.5">Time</label>
                <input type="time" value={session.session_time} onChange={e => setSession(prev => ({ ...prev, session_time: e.target.value }))} />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1.5">Serving Size</label>
                <div className="flex rounded-xl overflow-hidden border border-gray-200">
                  {[4, 6].map(n => (
                    <button key={n} onClick={() => setSession(prev => ({ ...prev, serving_size: n }))}
                      className={`flex-1 py-3 text-sm font-medium ${session.serving_size === n ? 'bg-navy text-white' : 'bg-white text-gray-600'}`}
                      style={{ minHeight: '44px' }}
                    >
                      {n} servings
                    </button>
                  ))}
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1.5">Session Window (hours)</label>
                <input type="number" step="0.5" min="1" max="8" value={session.session_window_hours}
                  onChange={e => setSession(prev => ({ ...prev, session_window_hours: parseFloat(e.target.value) }))} />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1.5">Status</label>
                <div className="flex rounded-xl overflow-hidden border border-gray-200">
                  {(['Pending', 'Confirmed'] as const).map(s => (
                    <button key={s} onClick={() => setSession(prev => ({ ...prev, status: s }))}
                      className={`flex-1 py-3 text-sm font-medium ${session.status === s ? 'bg-navy text-white' : 'bg-white text-gray-600'}`}
                      style={{ minHeight: '44px' }}
                    >
                      {s}
                    </button>
                  ))}
                </div>
              </div>
              <div>
                <label htmlFor="repeat" className="block text-sm font-medium text-gray-700 mb-1.5">Recurring</label>
                <select id="repeat" value={session.repeat_every_weeks ?? ''}
                  onChange={e => setSession(prev => ({ ...prev, repeat_every_weeks: e.target.value ? Number(e.target.value) : null }))}>
                  {REPEAT_OPTIONS.map(([w, label]) => <option key={label} value={w ?? ''}>{label}</option>)}
                </select>
                {session.repeat_every_weeks && (
                  <p className="text-xs text-gray-500 mt-1.5">When you mark this session complete, the next one is booked automatically.</p>
                )}
              </div>
            </div>
          </div>
        )}

        {/* STEP 3 — Menu */}
        {step === 2 && (
          <div>
            {/* Client allergen header */}
            {selectedClient && (
              <div className="mb-3 p-3 bg-blue-light rounded-xl">
                <div className="text-sm font-semibold text-navy">{selectedClient.name}</div>
                {clientAllergens.length > 0 && (
                  <div className="text-xs text-danger mt-0.5">⚠ Allergens: {clientAllergens.join(', ')}</div>
                )}
              </div>
            )}

            {/* Filters */}
            <div className="flex gap-2 overflow-x-auto no-scrollbar mb-3 pb-1">
              {['All', 'Couples', 'Families', 'Plant-Based'].map(c => (
                <button key={c}
                  onClick={() => setFilters(prev => ({ ...prev, category: c }))}
                  className={`shrink-0 px-3 py-1.5 rounded-full text-xs font-medium border ${
                    filters.category === c ? 'bg-navy text-white border-navy' : 'bg-white text-gray-600 border-gray-200'
                  }`}
                  style={{ minHeight: '32px' }}
                >{c}</button>
              ))}
              {['Session-Friendly', 'Long-Cook'].map(f => (
                <button key={f}
                  onClick={() => setFilters(prev => ({ ...prev, fit: prev.fit === f ? 'All' : f }))}
                  className={`shrink-0 px-3 py-1.5 rounded-full text-xs font-medium border ${
                    filters.fit === f
                      ? f === 'Session-Friendly' ? 'bg-green-dark text-white border-green-dark' : 'bg-amber-500 text-white border-amber-500'
                      : 'bg-white text-gray-600 border-gray-200'
                  }`}
                  style={{ minHeight: '32px' }}
                >{f}</button>
              ))}
            </div>

            <input
              type="text"
              placeholder="Search recipes..."
              value={filters.search}
              onChange={e => setFilters(prev => ({ ...prev, search: e.target.value }))}
              className="mb-3"
            />

            {/* Allergen filter toggle */}
            {clientAllergens.length > 0 && (
              <div className="flex items-center gap-2 mb-3 p-2 bg-gray-50 rounded-xl">
                <button
                  onClick={() => setFilters(prev => ({ ...prev, hideConflicts: !prev.hideConflicts }))}
                  className={`w-10 h-5 rounded-full transition-colors relative shrink-0 ${filters.hideConflicts ? 'bg-green-mid' : 'bg-gray-300'}`}
                  style={{ minHeight: 'auto', padding: 0 }}
                >
                  <span className={`absolute top-0.5 w-4 h-4 bg-white rounded-full shadow transition-transform ${filters.hideConflicts ? 'translate-x-5' : 'translate-x-0.5'}`} />
                </button>
                <span className="text-xs text-gray-600">
                  {filters.hideConflicts ? `Hiding ${conflictCount} allergen conflicts` : 'Showing all (conflicts flagged)'}
                </span>
              </div>
            )}

            {/* AI Suggest */}
            {aiStatus ? (
              <button
                onClick={handleAiSuggest}
                disabled={aiLoading}
                className="w-full py-2.5 mb-3 bg-green-light text-green-dark border border-green-mid rounded-xl text-sm font-semibold"
              >
                {aiLoading ? '✨ Finding best dishes...' : '✨ AI Suggest Menu'}
              </button>
            ) : (
              <button
                onClick={() => navigate('/settings')}
                className="w-full py-2.5 mb-3 bg-gray-50 text-gray-500 border border-gray-200 rounded-xl text-sm"
              >
                ✨ Configure AI in Settings
              </button>
            )}

            {aiReasoning && (
              <div className="mb-3 p-3 bg-green-light border border-green-mid rounded-xl">
                <div className="text-xs font-semibold text-green-dark mb-1">AI Suggestion:</div>
                <p className="text-xs text-green-dark">{aiReasoning}</p>
                <button
                  onClick={() => { setAiReasoning(''); }}
                  className="text-xs text-green-dark underline mt-1"
                  style={{ minHeight: 'auto' }}
                >Clear suggestion</button>
              </div>
            )}

            {/* Recipe list */}
            <div className="space-y-2">
              {filteredRecipes.map(r => {
                const selected = session.recipe_ids.includes(r.id);
                const conflicts = !filters.hideConflicts ? checkAllergenConflict(r, clientAllergens) : [];
                return (
                  <div
                    key={r.id}
                    onClick={() => toggleRecipe(r.id)}
                    className={`card cursor-pointer transition-all ${
                      selected ? 'border-2 border-green-mid bg-green-light' :
                      conflicts.length > 0 ? 'border border-danger bg-red-50' :
                      'border border-transparent'
                    }`}
                  >
                    <div className="flex items-start gap-3">
                      <div className={`w-5 h-5 rounded-full border-2 shrink-0 mt-0.5 flex items-center justify-center ${
                        selected ? 'bg-green-mid border-green-mid text-white' : 'border-gray-300'
                      }`}>
                        {selected && <span className="text-xs font-bold">✓</span>}
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-start justify-between gap-1">
                          <span className="font-display font-semibold text-navy text-sm">{r.name}</span>
                          <CategoryBadge cat={r.category} small />
                        </div>
                        <div className="flex items-center gap-2 mt-1 flex-wrap">
                          <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${r.session_fit === 'Session-Friendly' ? 'bg-green-light text-green-dark' : 'bg-amber-50 text-amber-700'}`}>
                            {r.session_fit === 'Session-Friendly' ? '✓' : '⏱'} {r.session_fit}
                          </span>
                          <span className="text-[11px] text-gray-500">{(r.prep_time_min || 0) + (r.cook_time_min || 0)} min</span>
                          <StarDisplay value={r.hold_quality || 0} size={11} />
                        </div>
                        {conflicts.length > 0 && (
                          <div className="text-xs text-danger mt-1">⚠ Contains: {conflicts.join(', ')}</div>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
              {filteredRecipes.length === 0 && (
                <div className="text-center py-6 text-gray-400 text-sm">No recipes match these filters</div>
              )}
            </div>

            {/* Save as template */}
            {session.recipe_ids.length > 0 && (
              <button
                onClick={handleSaveTemplate}
                className="mt-3 w-full py-2.5 border border-gray-200 text-gray-600 rounded-xl text-sm font-medium"
              >
                Save Current Menu as Template
              </button>
            )}
          </div>
        )}

        {/* STEP 4 — Costing */}
        {step === 3 && (
          <div className="space-y-4">
            <div className="card space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1.5">Service Fee</label>
                <div className="relative">
                  <span className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400">$</span>
                  <input
                    type="number"
                    min="0"
                    step="10"
                    value={session.service_fee}
                    onChange={e => setSession(prev => ({ ...prev, service_fee: parseFloat(e.target.value) || 0 }))}
                    className="pl-7"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1.5">Estimated Ingredient Cost</label>
                <div className="relative">
                  <span className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400">$</span>
                  <input
                    type="number"
                    min="0"
                    step="5"
                    value={session.ingredient_cost}
                    onChange={e => setSession(prev => ({ ...prev, ingredient_cost: parseFloat(e.target.value) || 0 }))}
                    className="pl-7"
                  />
                </div>
              </div>

              <div className="p-3 bg-warm-white rounded-xl space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-gray-500">Gross Margin</span>
                  <span className="font-semibold">${margin.toFixed(2)}</span>
                </div>
                <div className="flex justify-between text-sm">
                  <span className="text-gray-500">Margin %</span>
                  <span className={`font-bold ${marginPct === null ? 'text-gray-400' : marginColor(Number(marginPct))}`}>
                    {marginPct === null ? '—' : `${marginPct}%`}
                  </span>
                </div>
              </div>
            </div>

            <div className="card">
              <label className="block text-sm font-medium text-gray-700 mb-1.5">Chef Notes</label>
              <textarea rows={4} value={session.chef_notes || ''} onChange={e => setSession(prev => ({ ...prev, chef_notes: e.target.value }))} placeholder="Notes for this session..." />
            </div>
          </div>
        )}

        {/* STEP 5 — Review */}
        {step === 4 && (
          <div className="space-y-4">
            <div className="card">
              <div className="section-header">Session Summary</div>
              <div className="space-y-2">
                <div className="flex justify-between"><span className="text-sm text-gray-500">Client</span><span className="text-sm font-medium">{selectedClient?.name || '—'}</span></div>
                <div className="flex justify-between"><span className="text-sm text-gray-500">Date</span><span className="text-sm font-medium">{session.session_date || '—'}</span></div>
                <div className="flex justify-between"><span className="text-sm text-gray-500">Time</span><span className="text-sm font-medium">{session.session_time || '—'}</span></div>
                <div className="flex justify-between"><span className="text-sm text-gray-500">Serving Size</span><span className="text-sm font-medium">{session.serving_size}</span></div>
                <div className="flex justify-between"><span className="text-sm text-gray-500">Status</span><span className="text-sm font-medium">{session.status}</span></div>
                <div className="flex justify-between"><span className="text-sm text-gray-500">Recurring</span><span className="text-sm font-medium">{repeatLabel(session.repeat_every_weeks)}</span></div>
                <div className="flex justify-between"><span className="text-sm text-gray-500">Service Fee</span><span className="text-sm font-medium">${session.service_fee}</span></div>
                <div className="flex justify-between"><span className="text-sm text-gray-500">Ingredient Cost</span><span className="text-sm font-medium">${session.ingredient_cost}</span></div>
                <div className="flex justify-between"><span className="text-sm text-gray-500">Margin</span>
                  <span className={`text-sm font-bold ${marginPct === null ? 'text-gray-400' : marginColor(Number(marginPct))}`}>{marginPct === null ? '—' : `${marginPct}%`}</span>
                </div>
              </div>
            </div>

            <div className="card">
              <div className="section-header">Menu ({selectedRecipes.length} dishes)</div>
              {selectedRecipes.map(r => (
                <div key={r.id} className="py-2 border-b border-gray-100 last:border-0">
                  <div className="text-sm font-medium text-navy">{r.name}</div>
                  <div className="text-xs text-gray-500">{r.category} · {(r.prep_time_min || 0) + (r.cook_time_min || 0)} min</div>
                </div>
              ))}
            </div>

            {hasAllergenConflicts && (
              <div className="p-3 bg-red-50 border border-danger rounded-xl">
                <div className="text-sm font-semibold text-danger mb-2">⚠ Allergen Conflict Detected</div>
                {selectedRecipes.filter(r => checkAllergenConflict(r, clientAllergens).length > 0).map(r => (
                  <div key={r.id} className="text-xs text-danger mb-1">
                    {r.name}: contains {checkAllergenConflict(r, clientAllergens).join(', ')}
                  </div>
                ))}
                <label className="flex items-start gap-2 mt-3 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={allergenAcknowledged}
                    onChange={e => setAllergenAcknowledged(e.target.checked)}
                    style={{ width: '18px', height: '18px', minHeight: 'auto', marginTop: '1px' }}
                  />
                  <span className="text-xs text-danger">I acknowledge this allergen conflict and confirm the chef has verified safety</span>
                </label>
              </div>
            )}

            {session.recipe_ids.length === 0 && (
              <div className="p-3 bg-red-50 border border-danger rounded-xl text-sm text-danger font-medium">
                Add at least 1 dish before confirming
              </div>
            )}
          </div>
        )}
      </div>

      {/* Bottom sticky panel */}
      <div className="fixed fixed-action-bar left-0 right-0 bg-white border-t border-gray-200 shadow-lg z-20 md:left-[220px]">
        {step === 2 && session.recipe_ids.length > 0 && (
          <div className="px-4 py-2 bg-warm-white border-b border-gray-100">
            <div className="flex justify-between text-xs text-gray-600">
              <span>Dishes: <b>{session.recipe_ids.length}</b></span>
              <span>Active: <b>{selectedRecipes.reduce((s, r) => s + (r.prep_time_min || 0), 0)} min</b></span>
              <span>Passive: <b>{selectedRecipes.length ? Math.max(...selectedRecipes.map(r => r.cook_time_min || 0)) : 0} min</b></span>
            </div>
            <div className="flex items-center justify-between mt-1">
              <span className="text-xs font-semibold text-navy">Total: ~{Math.round(feasibility.estimated_min)} min</span>
              <span className={`text-xs px-2 py-0.5 rounded-full font-bold ${FEASIBILITY_COLORS[feasibility.level] || 'bg-gray-100'}`}>
                {feasibility.level}
              </span>
            </div>
          </div>
        )}
        <div className="flex gap-3 px-4 py-3">
          {step > 0 && (
            <button onClick={() => setStep(s => s - 1)} className="shrink-0 px-4 py-3 border border-gray-200 text-gray-600 rounded-xl text-sm font-medium">
              ← Back
            </button>
          )}
          {step < 4 ? (
            <button
              onClick={() => { if (canProceed()) setStep(s => s + 1); else show('Complete this step first', 'error'); }}
              className="flex-1 py-3 bg-navy text-white rounded-xl text-sm font-semibold"
            >
              {step === 2 ? `Next → (${session.recipe_ids.length} dishes)` : 'Next →'}
            </button>
          ) : (
            <button
              onClick={handleConfirm}
              disabled={saving || session.recipe_ids.length === 0 || (hasAllergenConflicts && !allergenAcknowledged)}
              className="flex-1 py-3 bg-navy text-white rounded-xl text-sm font-semibold disabled:opacity-50"
            >
              {saving ? 'Saving...' : 'Confirm Session'}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
