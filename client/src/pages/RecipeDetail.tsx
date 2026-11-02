// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, type ChangeEvent, type FocusEvent, type KeyboardEvent } from 'react';
import { useNavigate, useParams } from 'react-router';
import PageHeader from '../components/PageHeader';
import StarRating, { StarDisplay } from '../components/StarRating';
import { useToast } from '../components/Toast';
import { apiCall } from '../hooks/useApi';
import { STORE_SECTIONS, detectAllergens } from '../constants';
import { scaleQty, deriveQty6 } from '../utils/quantityScale';
import type { NumInput } from '../utils/numbers';
import type { RecipeDetail as RecipeDetailData } from '../types';

/** An ingredient row being edited (new rows have no ids yet). */
interface IngredientDraft {
  id?: number;
  recipe_id?: number;
  name: string;
  qty_4_serving: string | null;
  qty_6_serving: string | null;
  store_section: string | null;
  category?: string | null;
  storage_note?: string | null;
}

interface RecipeForm {
  id?: number;
  name: string;
  category: string | null;
  session_fit: string | null;
  prep_time_min: NumInput | null;
  cook_time_min: NumInput | null;
  servings_base: NumInput | null;
  hold_quality: number | null;
  reheat_quality: number | null;
  key_overlap_ingredients: string[];
  chef_notes: string | null;
  raw_instructions: string | null;
  ingredients: IngredientDraft[];
}

const CategoryBadge = ({ cat }: { cat: string | null }) => (
  <span className={`text-xs px-2.5 py-1 rounded-full font-medium ${
    cat === 'Couples'     ? 'bg-navy text-white' :
    cat === 'Families'    ? 'bg-green-dark text-white' :
    cat === 'Plant-Based' ? 'bg-teal-700 text-white' :
                            'bg-purple-700 text-white'
  }`}>{cat}</span>
);

const FitBadge = ({ fit }: { fit: string | null }) => (
  <span className={`text-xs px-2.5 py-1 rounded-full font-medium ${
    fit === 'Session-Friendly' ? 'bg-green-light text-green-dark' : 'bg-amber-50 text-amber-700'
  }`}>
    {fit === 'Session-Friendly' ? '✓ Session-Friendly' : '⏱ Long-Cook'}
  </span>
);

export default function RecipeDetail({ edit: editMode }: { edit?: boolean }) {
  const { id } = useParams();
  const navigate = useNavigate();
  const { show } = useToast();
  // /recipes/new has no :id param (its own route), so a missing id is new too.
  const isNew = !id || id === 'new';
  const [editing, setEditing] = useState(editMode || isNew);
  const [showInstructions, setShowInstructions] = useState(false);
  const [saving, setSaving] = useState(false);
  const [recipe, setRecipe] = useState<RecipeDetailData | null>(null);
  // '' while the servings input is cleared mid-edit
  const [viewServings, setViewServings] = useState<number | ''>(4);
  const [form, setForm] = useState<RecipeForm>({
    name: '', category: 'Couples', session_fit: 'Session-Friendly',
    prep_time_min: '', cook_time_min: '', servings_base: 4,
    hold_quality: 4, reheat_quality: 4,
    key_overlap_ingredients: [], chef_notes: '', raw_instructions: '',
    ingredients: [],
  });

  useEffect(() => {
    if (!isNew) {
      // C4: reset prior recipe state when navigating between recipes
      setRecipe(null);
      fetch(`/api/recipes/${id}`)
        .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
        .then((d: RecipeDetailData) => {
          setRecipe(d);
          setViewServings(d.servings_base || 4);
          setForm({
            ...d,
            key_overlap_ingredients: d.key_overlap_ingredients || [],
            ingredients: d.ingredients || [],
          });
        })
        .catch(() => {});
    }
  }, [id]);

  const set = <K extends keyof RecipeForm>(field: K, val: RecipeForm[K]) => setForm(prev => ({ ...prev, [field]: val }));

  const detectedAllergens = detectAllergens(form.ingredients);

  const handleSave = async () => {
    if (!form.name.trim()) { show('Name is required', 'error'); return; }
    const base = parseInt(String(form.servings_base)) || 4;
    if (base < 1 || base > 100) { show('Serving size must be between 1 and 100', 'error'); return; }
    setSaving(true);
    try {
      const method = isNew ? 'POST' : 'PUT';
      const url = isNew ? '/api/recipes' : `/api/recipes/${id}`;
      const saved = await apiCall<RecipeDetailData>(url, method, { ...form, servings_base: base });
      show('Recipe saved', 'success');
      setRecipe(saved);
      setViewServings(saved.servings_base || 4);
      setEditing(false);
      if (isNew) navigate(`/recipes/${saved.id}`, { replace: true });
    } catch {
      show('Could not save recipe', 'error');
    } finally {
      setSaving(false);
    }
  };

  const updateIngredient = (idx: number, field: 'name' | 'qty_4_serving' | 'qty_6_serving' | 'store_section', val: string) => {
    setForm(prev => ({
      ...prev,
      ingredients: prev.ingredients.map((ing, i) => i === idx ? { ...ing, [field]: val } : ing),
    }));
  };

  // Auto-fill qty_6_serving from qty_4_serving when user leaves the base field
  const autoFill6 = (idx: number) => {
    const ing = form.ingredients[idx];
    if (ing.qty_4_serving && !ing.qty_6_serving) {
      updateIngredient(idx, 'qty_6_serving', deriveQty6(ing.qty_4_serving));
    }
  };

  const addIngredient = () => {
    setForm(prev => ({
      ...prev,
      ingredients: [
        ...prev.ingredients,
        { name: '', qty_4_serving: '', qty_6_serving: '', store_section: 'Produce' },
      ],
    }));
  };

  const removeIngredient = (idx: number) => {
    setForm(prev => ({ ...prev, ingredients: prev.ingredients.filter((_, i) => i !== idx) }));
  };

  const addOverlap = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' || e.key === ',') {
      e.preventDefault();
      const input = e.currentTarget;
      const val = input.value.trim().replace(/,$/, '');
      if (val && !form.key_overlap_ingredients.includes(val)) {
        set('key_overlap_ingredients', [...form.key_overlap_ingredients, val]);
      }
      input.value = '';
    }
  };

  // Serving size adjuster helpers
  const adjustViewServings = (delta: number) => {
    setViewServings(prev => Math.max(1, (prev as number) + delta));
  };
  const handleViewServingsInput = (e: ChangeEvent<HTMLInputElement>) => {
    const v = parseInt(e.target.value);
    if (!isNaN(v) && v >= 1 && v <= 200) setViewServings(v);
    else if (e.target.value === '') setViewServings('');
  };
  const handleViewServingsBlur = (e: FocusEvent<HTMLInputElement>) => {
    const v = parseInt(e.target.value);
    setViewServings(!isNaN(v) && v >= 1 ? v : (recipe?.servings_base || 4));
  };

  const displayData: RecipeForm = editing ? form : (recipe || form);
  const baseServings = parseInt(String(displayData.servings_base)) || 4;
  // Arithmetic below treats a cleared input ('') as 0, exactly as the JS did
  const viewN = viewServings as number;

  if (!isNew && !recipe && !editing) {
    return (
      <div className="page">
        <PageHeader title="Recipe" back="/recipes" />
        <div className="px-4 pt-4">
          <div className="skeleton h-6 w-2/3 mb-3" />
          <div className="skeleton h-4 w-full mb-2" />
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <PageHeader
        title={isNew ? 'New Recipe' : displayData.name || 'Recipe'}
        back={editing && !isNew ? () => setEditing(false) : '/recipes'}
        action={
          !editing ? (
            <button onClick={() => setEditing(true)} className="text-white text-sm font-medium" style={{ minHeight: 'auto' }}>Edit</button>
          ) : (
            <button onClick={handleSave} disabled={saving} className="text-white text-sm font-semibold" style={{ minHeight: 'auto' }}>
              {saving ? 'Saving…' : 'Save'}
            </button>
          )
        }
      />

      <div className="px-4 pt-4 pb-28">

        {/* ── VIEW MODE HEADER ──────────────────────────────────────────── */}
        {!editing && (
          <>
            <div className="flex items-start gap-2 flex-wrap mb-3">
              <CategoryBadge cat={displayData.category} />
              <FitBadge fit={displayData.session_fit} />
            </div>
            <h1 className="font-display text-2xl font-bold text-navy mb-3">{displayData.name}</h1>
            <div className="card mb-3">
              <div className="grid grid-cols-4 text-center gap-2">
                {[
                  ['Prep',  `${displayData.prep_time_min || 0} min`],
                  ['Cook',  `${displayData.cook_time_min || 0} min`],
                  ['Total', `${((displayData.prep_time_min || 0) as number) + ((displayData.cook_time_min || 0) as number)} min`],
                  ['Serves', baseServings],
                ].map(([label, val]) => (
                  <div key={label}>
                    <div className="text-xs text-gray-500 mb-0.5">{label}</div>
                    <div className="text-sm font-semibold text-navy">{val}</div>
                  </div>
                ))}
              </div>
            </div>
            <div className="card mb-3">
              <div className="flex justify-between items-center">
                <div>
                  <div className="text-xs text-gray-500 mb-1">Hold Quality</div>
                  <StarDisplay value={displayData.hold_quality || 0} />
                </div>
                <div>
                  <div className="text-xs text-gray-500 mb-1">Reheat Quality</div>
                  <StarDisplay value={displayData.reheat_quality || 0} />
                </div>
              </div>
            </div>
          </>
        )}

        {/* ── EDIT MODE HEADER ──────────────────────────────────────────── */}
        {editing && (
          <div className="card mb-3 space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1.5">Recipe Name *</label>
              <input
                value={form.name}
                onChange={e => set('name', e.target.value)}
                placeholder="Recipe name…"
                className="font-display text-lg"
                style={{ fontFamily: '"Playfair Display", serif' }}
              />
            </div>

            <div className="flex gap-3">
              <div className="flex-1">
                <label className="block text-xs text-gray-500 mb-1">Category</label>
                <select value={form.category as string} onChange={e => set('category', e.target.value)}>
                  {['Couples', 'Families', 'Plant-Based', 'Other'].map(c => <option key={c}>{c}</option>)}
                </select>
              </div>
              <div className="flex-1">
                <label className="block text-xs text-gray-500 mb-1">Session Fit</label>
                <select value={form.session_fit as string} onChange={e => set('session_fit', e.target.value)}>
                  <option>Session-Friendly</option>
                  <option>Long-Cook</option>
                </select>
              </div>
            </div>

            <div className="grid grid-cols-3 gap-3">
              <div>
                <label className="block text-xs text-gray-500 mb-1">Prep (min)</label>
                <input type="number" min="0" value={form.prep_time_min || ''} onChange={e => set('prep_time_min', parseInt(e.target.value) || 0)} />
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1">Cook (min)</label>
                <input type="number" min="0" value={form.cook_time_min || ''} onChange={e => set('cook_time_min', parseInt(e.target.value) || 0)} />
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1">Base Serves</label>
                <input
                  type="number" min="1" max="100"
                  value={form.servings_base || ''}
                  onChange={e => set('servings_base', parseInt(e.target.value) || '')}
                  placeholder="4"
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs text-gray-500 mb-1.5">Hold Quality</label>
                <StarRating value={form.hold_quality} onChange={v => set('hold_quality', v)} />
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1.5">Reheat Quality</label>
                <StarRating value={form.reheat_quality} onChange={v => set('reheat_quality', v)} />
              </div>
            </div>

            <div>
              <label className="block text-xs text-gray-500 mb-1">Chef Notes</label>
              <textarea rows={3} value={form.chef_notes || ''} onChange={e => set('chef_notes', e.target.value)} />
            </div>
          </div>
        )}

        {/* ── OVERLAP INGREDIENTS ───────────────────────────────────────── */}
        <div className="card mb-3">
          <div className="section-header">Key Overlap Ingredients</div>
          {editing ? (
            <div>
              <div className="flex flex-wrap gap-1.5 mb-2">
                {form.key_overlap_ingredients.map((tag, i) => (
                  <span key={i} className="flex items-center gap-1 px-2.5 py-1 bg-green-light text-green-dark rounded-full text-xs font-medium">
                    {tag}
                    <button
                      onClick={() => set('key_overlap_ingredients', form.key_overlap_ingredients.filter((_, j) => j !== i))}
                      style={{ minHeight: 'auto' }}
                    >×</button>
                  </span>
                ))}
              </div>
              <input placeholder="Type ingredient + Enter to add…" onKeyDown={addOverlap} />
            </div>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {(displayData.key_overlap_ingredients || []).length === 0
                ? <span className="text-xs text-gray-400">None set</span>
                : (displayData.key_overlap_ingredients || []).map((t, i) => (
                    <span key={i} className="px-2.5 py-1 bg-green-light text-green-dark rounded-full text-xs font-medium">{t}</span>
                  ))
              }
            </div>
          )}
        </div>

        {/* ── ALLERGEN DETECTION ────────────────────────────────────────── */}
        {detectedAllergens.length > 0 && (
          <div className="mb-3 px-3 py-3 bg-amber-50 border border-amber-200 rounded-xl">
            <div className="text-sm font-semibold text-amber-800 mb-1">⚠ Allergens detected in ingredients</div>
            <div className="flex flex-wrap gap-1">
              {detectedAllergens.map(a => (
                <span key={a} className="text-xs px-2 py-0.5 bg-amber-100 text-amber-800 rounded-full">{a}</span>
              ))}
            </div>
          </div>
        )}

        {/* ── INGREDIENTS ───────────────────────────────────────────────── */}
        <div className="card mb-3">
          {/* Section header — view shows serving adjuster, edit shows base label */}
          <div className="flex items-center justify-between mb-3">
            <span className="font-display font-semibold text-green-dark text-sm">Ingredients</span>
            {!editing && (
              /* Serving size adjuster — only visible in view mode */
              <div className="flex items-center gap-1.5">
                <span className="text-xs text-gray-500 mr-0.5">Serves</span>
                <button
                  onClick={() => adjustViewServings(-1)}
                  className="w-6 h-6 rounded-full bg-gray-100 text-gray-600 text-sm font-bold flex items-center justify-center"
                  style={{ minHeight: 'auto' }}
                >−</button>
                <input
                  type="number"
                  min="1" max="200"
                  value={viewServings}
                  onChange={handleViewServingsInput}
                  onBlur={handleViewServingsBlur}
                  className="text-center font-semibold text-navy text-sm"
                  style={{ width: '40px', padding: '2px 4px', minHeight: 'auto' }}
                />
                <button
                  onClick={() => adjustViewServings(1)}
                  className="w-6 h-6 rounded-full bg-gray-100 text-gray-600 text-sm font-bold flex items-center justify-center"
                  style={{ minHeight: 'auto' }}
                >+</button>
                {viewServings !== baseServings && (
                  <button
                    onClick={() => setViewServings(baseServings)}
                    className="text-xs text-gray-400 underline ml-1"
                    style={{ minHeight: 'auto' }}
                  >reset</button>
                )}
              </div>
            )}
            {editing && (
              <span className="text-xs text-gray-400">base = {form.servings_base || 4} servings</span>
            )}
          </div>

          {/* ── VIEW MODE ingredient table ────────────────────────────── */}
          {!editing && (
            <>
              {(displayData.ingredients || []).length === 0 ? (
                <div className="text-sm text-gray-400 py-2 text-center">
                  No ingredient data. Tap <strong>Edit</strong> to add ingredients.
                </div>
              ) : (
                <>
                  {viewServings !== baseServings && (
                    <div className="mb-2 px-2 py-1.5 bg-blue-light rounded-lg text-xs text-navy-mid font-medium">
                      Scaled from {baseServings} → {viewServings} servings
                      {viewN > baseServings
                        ? ` (×${(viewN / baseServings).toFixed(2).replace(/\.?0+$/, '')})`
                        : ` (×${(viewN / baseServings).toFixed(2).replace(/\.?0+$/, '')})`}
                    </div>
                  )}
                  <div className="overflow-x-auto -mx-4 px-4">
                    <table className="w-full text-xs">
                      <thead>
                        <tr className="border-b border-gray-200">
                          <th className="text-left py-2 text-gray-500 font-medium">Ingredient</th>
                          <th className="text-right py-2 text-gray-500 font-medium">
                            Qty{viewServings !== baseServings ? ` (${viewServings})` : ''}
                          </th>
                          <th className="text-left py-2 pl-3 text-gray-500 font-medium hidden sm:table-cell">Section</th>
                        </tr>
                      </thead>
                      <tbody>
                        {(displayData.ingredients || []).map((ing, i) => {
                          const scaled = scaleQty(
                            ing.qty_4_serving,
                            baseServings,
                            viewN,
                          );
                          const missing = !ing.qty_4_serving;
                          return (
                            <tr key={i} className={`border-b border-gray-100 last:border-0 ${missing ? 'bg-amber-50' : ''}`}>
                              <td className="py-2 pr-3 font-medium text-navy">{ing.name}</td>
                              <td className={`py-2 text-right font-medium ${missing ? 'text-amber-500' : 'text-gray-700'}`}>
                                {scaled || '—'}
                              </td>
                              <td className="py-2 pl-3 text-gray-400 hidden sm:table-cell">{ing.store_section}</td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                </>
              )}
            </>
          )}

          {/* ── EDIT MODE ingredient table ────────────────────────────── */}
          {editing && (
            <div className="overflow-x-auto -mx-4 px-4">
              {form.ingredients.length > 0 && (
                <table className="w-full text-xs mb-3">
                  <thead>
                    <tr className="border-b border-gray-200">
                      <th className="text-left py-2 text-gray-500 font-medium">Ingredient</th>
                      <th className="text-center py-2 text-gray-500 font-medium">
                        Qty ({form.servings_base || 4})
                      </th>
                      <th className="text-center py-2 text-gray-500 font-medium">Qty (6)</th>
                      <th className="text-left py-2 text-gray-500 font-medium">Section</th>
                      <th className="w-6" />
                    </tr>
                  </thead>
                  <tbody>
                    {form.ingredients.map((ing, i) => (
                      <tr key={i} className="border-b border-gray-100 last:border-0 align-top">
                        <td className="py-1.5 pr-1">
                          <input
                            value={ing.name}
                            onChange={e => updateIngredient(i, 'name', e.target.value)}
                            placeholder="Name…"
                            className="text-xs p-1 w-full"
                            style={{ minHeight: '32px' }}
                          />
                        </td>
                        <td className="py-1.5 px-1">
                          <input
                            value={ing.qty_4_serving || ''}
                            onChange={e => updateIngredient(i, 'qty_4_serving', e.target.value)}
                            onBlur={() => autoFill6(i)}
                            placeholder="e.g. 2 cups"
                            className="text-xs p-1 text-center w-20"
                            style={{ minHeight: '32px' }}
                          />
                        </td>
                        <td className="py-1.5 px-1">
                          <input
                            value={ing.qty_6_serving || ''}
                            onChange={e => updateIngredient(i, 'qty_6_serving', e.target.value)}
                            placeholder="auto"
                            className="text-xs p-1 text-center w-20"
                            style={{ minHeight: '32px' }}
                          />
                        </td>
                        <td className="py-1.5 px-1">
                          <select
                            value={ing.store_section || 'Produce'}
                            onChange={e => updateIngredient(i, 'store_section', e.target.value)}
                            style={{ minHeight: '32px', padding: '2px 4px', fontSize: '11px' }}
                          >
                            {STORE_SECTIONS.map(s => <option key={s}>{s}</option>)}
                          </select>
                        </td>
                        <td className="py-1.5 text-center">
                          <button
                            onClick={() => removeIngredient(i)}
                            className="text-danger text-base leading-none"
                            style={{ minHeight: 'auto' }}
                            title="Remove ingredient"
                          >×</button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}

              {/* ── Empty state with inline first-ingredient form ────── */}
              {form.ingredients.length === 0 && (
                <div className="py-3 mb-2 text-center border border-dashed border-gray-200 rounded-xl bg-gray-50">
                  <div className="text-sm text-gray-400 mb-2">No ingredients yet</div>
                </div>
              )}

              {/* Add button — always visible in edit mode */}
              <button
                onClick={addIngredient}
                className="flex items-center gap-1.5 text-sm text-navy-mid font-medium"
                style={{ minHeight: 'auto' }}
              >
                <span className="text-base leading-none">+</span> Add Ingredient
              </button>

              {form.ingredients.length > 0 && (
                <p className="text-xs text-gray-400 mt-2">
                  Tip: tab out of the base qty field to auto-fill the 6-serving column.
                </p>
              )}
            </div>
          )}
        </div>

        {/* ── CHEF NOTES (view mode) ────────────────────────────────────── */}
        {!editing && displayData.chef_notes && (
          <div className="card mb-3">
            <div className="section-header">Chef Notes</div>
            <p className="text-sm text-gray-700">{displayData.chef_notes}</p>
          </div>
        )}

        {/* ── COOKING METHOD ────────────────────────────────────────────── */}
        {(displayData.raw_instructions || editing) && (
          <div className="card mb-3">
            <button
              className="w-full flex items-center justify-between"
              onClick={() => setShowInstructions(!showInstructions)}
              style={{ minHeight: 'auto' }}
            >
              <span className="font-display font-semibold text-green-dark text-sm">Cooking Method</span>
              <span className="text-gray-400 text-xs">
                {showInstructions ? '▲' : '▼'}{!editing && ' (for your reference)'}
              </span>
            </button>
            {showInstructions && (
              editing ? (
                <textarea
                  rows={6}
                  value={form.raw_instructions || ''}
                  onChange={e => set('raw_instructions', e.target.value)}
                  className="mt-3"
                  placeholder="Cooking instructions…"
                />
              ) : (
                <p className="mt-3 text-sm text-gray-700 whitespace-pre-line">{displayData.raw_instructions}</p>
              )
            )}
          </div>
        )}
      </div>
    </div>
  );
}
