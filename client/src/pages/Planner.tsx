// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router';
import PageHeader from '../components/PageHeader';
import { useToast } from '../components/Toast';
import { localTodayStr } from '../utils/dateUtils';
import { downloadServerPdf, pdfFilename } from '../utils/serverPdf';
import { errorMessage } from '../hooks/useApi';
import type { Id, PlannerResponse, Recipe } from '../types';

const CATEGORIES = ['All', 'Couples', 'Families', 'Plant-Based', 'Other'];
const SECTION_ICONS: Record<string, string> = {
  'Produce': '🥬',
  'Meat & Seafood': '🥩',
  'Dairy & Eggs': '🥛',
  'Pantry & Dry Goods': '🫙',
  'Canned & Jarred': '🥫',
  'Herbs & Spices': '🌿',
  'Other': '🛒',
};

export default function Planner() {
  const navigate = useNavigate();
  const { show } = useToast();

  const [recipes, setRecipes] = useState<Recipe[]>([]);
  const [selected, setSelected] = useState(new Set<Id>());
  const [servingSize, setServingSize] = useState(4);
  const [servingInput, setServingInput] = useState('4');
  const [tab, setTab] = useState<'select' | 'ingredients'>('select');
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState('All');
  const [fitFilter, setFitFilter] = useState('');
  const [ingredientData, setIngredientData] = useState<PlannerResponse | null>(null);
  const [calcLoading, setCalcLoading] = useState(false);
  const [recipesLoading, setRecipesLoading] = useState(true);
  const abortRef = useRef<AbortController | null>(null);

  // Load recipe library once
  useEffect(() => {
    fetch('/api/recipes')
      .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
      .then(data => { setRecipes(Array.isArray(data) ? data : []); setRecipesLoading(false); })
      .catch(() => { setRecipes([]); setRecipesLoading(false); });
  }, []);

  // Recalculate ingredients whenever selection or serving size changes
  useEffect(() => {
    if (selected.size === 0) {
      setIngredientData(null);
      return;
    }

    // Cancel previous in-flight request
    if (abortRef.current) abortRef.current.abort();
    const controller = new AbortController();
    abortRef.current = controller;

    setCalcLoading(true);
    fetch('/api/planner/ingredients', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ recipe_ids: [...selected], serving_size: servingSize }),
      signal: controller.signal,
    })
      .then(r => r.json())
      .then(data => { setIngredientData(data); setCalcLoading(false); })
      .catch(e => { if (e.name !== 'AbortError') setCalcLoading(false); });

    return () => controller.abort();
  }, [selected, servingSize]);

  const toggleRecipe = (id: Id) => {
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const clearAll = () => {
    setSelected(new Set<Id>());
    setIngredientData(null);
    setTab('select');
  };

  const filteredRecipes = recipes.filter(r => {
    if (search && !r.name.toLowerCase().includes(search.toLowerCase())) return false;
    if (category !== 'All' && r.category !== category) return false;
    if (fitFilter === 'Session-Friendly' && r.session_fit !== 'Session-Friendly') return false;
    if (fitFilter === 'Long-Cook' && r.session_fit !== 'Long-Cook') return false;
    return true;
  });

  // ── Export CSV ─────────────────────────────────────────────────────────
  const exportCSV = () => {
    if (!ingredientData?.sections?.length) return;
    const lines = ['Name,Quantity,Store Section,Dishes'];
    for (const sec of ingredientData.sections) {
      for (const item of sec.items) {
        const dishes = item.source_dishes.join('; ').replace(/"/g, '""');
        lines.push(`"${item.name}","${item.quantity}","${sec.section}","${dishes}"`);
      }
    }
    const blob = new Blob([lines.join('\n')], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `planner-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
    show('CSV downloaded', 'success');
  };

  // ── Export PDF ─────────────────────────────────────────────────────────
  const exportPDF = async () => {
    if (!ingredientData?.sections?.length) return;
    try {
      // The browser's local date, for the "Generated" line and the filename
      const today = localTodayStr();
      const query = new URLSearchParams({
        recipe_ids: [...selected].join(','),
        serving_size: String(servingSize),
        date: today,
      });
      await downloadServerPdf(`/api/planner/pdf?${query}`, pdfFilename.planner(today));
      show('PDF downloaded', 'success');
    } catch (e) {
      show('PDF export failed: ' + errorMessage(e), 'error');
    }
  };

  const selCount = selected.size;
  const itemCount = ingredientData?.total_items ?? 0;

  return (
    <div className="page pb-20">
      <PageHeader
        title="Menu Planner"
        back="/"
        action={
          <div className="flex items-center gap-1.5">
            <span className="text-white text-xs opacity-80">Serves</span>
            <button
              onClick={() => { const v = Math.max(1, servingSize - 1); setServingSize(v); setServingInput(String(v)); }}
              className="w-6 h-6 rounded-full bg-white/20 text-white text-sm font-bold flex items-center justify-center"
              style={{ minHeight: 'auto' }}
            >−</button>
            <input
              type="number"
              min="1" max="200"
              value={servingInput}
              onChange={e => {
                setServingInput(e.target.value);
                const v = parseInt(e.target.value);
                if (!isNaN(v) && v >= 1 && v <= 200) setServingSize(v);
              }}
              onBlur={() => {
                const v = parseInt(servingInput);
                const safe = !isNaN(v) && v >= 1 ? v : 4;
                setServingSize(safe);
                setServingInput(String(safe));
              }}
              className="text-center font-semibold text-navy text-sm bg-white rounded-lg"
              style={{ width: '42px', padding: '2px 4px', minHeight: 'auto' }}
            />
            <button
              onClick={() => { const v = servingSize + 1; setServingSize(v); setServingInput(String(v)); }}
              className="w-6 h-6 rounded-full bg-white/20 text-white text-sm font-bold flex items-center justify-center"
              style={{ minHeight: 'auto' }}
            >+</button>
          </div>
        }
      />

      {/* Tab bar */}
      <div className="flex border-b border-gray-200 bg-white sticky top-0 z-10">
        <button
          className={`flex-1 py-3 text-sm font-medium transition-colors ${tab === 'select' ? 'text-navy border-b-2 border-navy' : 'text-gray-500'}`}
          onClick={() => setTab('select')}
        >
          Select Recipes
          {selCount > 0 && (
            <span className="ml-1.5 bg-navy text-white text-xs px-1.5 py-0.5 rounded-full">{selCount}</span>
          )}
        </button>
        <button
          className={`flex-1 py-3 text-sm font-medium transition-colors ${tab === 'ingredients' ? 'text-navy border-b-2 border-navy' : 'text-gray-500'}`}
          onClick={() => { if (selCount > 0) setTab('ingredients'); }}
          disabled={selCount === 0}
        >
          Ingredients
          {itemCount > 0 && (
            <span className="ml-1.5 bg-green-dark text-white text-xs px-1.5 py-0.5 rounded-full">{itemCount}</span>
          )}
        </button>
      </div>

      {/* ── SELECT TAB ──────────────────────────────────────────────────── */}
      {tab === 'select' && (
        <div>
          {/* Search + filters */}
          <div className="px-4 pt-3 pb-2 bg-white border-b border-gray-100 sticky top-[49px] z-10">
            <input
              type="text"
              placeholder="Search recipes..."
              value={search}
              onChange={e => setSearch(e.target.value)}
              className="input mb-2"
            />
            <div className="flex gap-1.5 overflow-x-auto no-scrollbar pb-1">
              {CATEGORIES.map(c => (
                <button
                  key={c}
                  onClick={() => setCategory(c)}
                  className={`shrink-0 px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
                    category === c
                      ? 'bg-navy text-white border-navy'
                      : 'bg-white text-gray-600 border-gray-200'
                  }`}
                >
                  {c}
                </button>
              ))}
            </div>
            <div className="flex gap-1.5 mt-1.5">
              {['', 'Session-Friendly', 'Long-Cook'].slice(1).map(fit => (
                <button
                  key={fit}
                  onClick={() => setFitFilter(prev => prev === fit ? '' : fit)}
                  className={`shrink-0 px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
                    fitFilter === fit
                      ? fit === 'Session-Friendly' ? 'bg-green-dark text-white border-green-dark' : 'bg-amber-500 text-white border-amber-500'
                      : 'bg-white text-gray-600 border-gray-200'
                  }`}
                >
                  {fit === 'Session-Friendly' ? '✓ Session-Friendly' : '⏱ Long-Cook'}
                </button>
              ))}
            </div>
          </div>

          {/* Selection summary bar */}
          {selCount > 0 && (
            <div className="flex items-center justify-between px-4 py-2 bg-blue-light border-b border-blue-200">
              <span className="text-sm font-medium text-navy">
                {selCount} recipe{selCount !== 1 ? 's' : ''} selected
              </span>
              <div className="flex gap-2">
                <button
                  onClick={clearAll}
                  className="text-xs text-gray-500 underline"
                >
                  Clear all
                </button>
                <button
                  onClick={() => setTab('ingredients')}
                  className="text-xs font-semibold text-navy underline"
                >
                  View ingredients →
                </button>
              </div>
            </div>
          )}

          {/* Recipe list */}
          {recipesLoading ? (
            <div className="px-4 pt-6 text-center text-gray-400 text-sm">Loading recipes...</div>
          ) : filteredRecipes.length === 0 ? (
            <div className="px-4 pt-8 text-center text-gray-400 text-sm">No recipes match your filters.</div>
          ) : (
            <div className="px-4 pt-3">
              {filteredRecipes.map(r => {
                const isSel = selected.has(r.id);
                return (
                  <div
                    key={r.id}
                    onClick={() => toggleRecipe(r.id)}
                    className={`card mb-2 cursor-pointer transition-all ${isSel ? 'ring-2 ring-navy bg-blue-light' : ''}`}
                  >
                    <div className="flex items-start gap-3">
                      {/* Checkbox */}
                      <div className={`shrink-0 w-5 h-5 rounded border-2 flex items-center justify-center mt-0.5 transition-colors ${
                        isSel ? 'bg-navy border-navy' : 'border-gray-300'
                      }`}>
                        {isSel && (
                          <svg width="11" height="9" viewBox="0 0 11 9" fill="none">
                            <path d="M1 4.5L4 7.5L10 1.5" stroke="white" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                          </svg>
                        )}
                      </div>

                      <div className="flex-1 min-w-0">
                        <div className="font-semibold text-navy text-sm leading-snug">{r.name}</div>
                        <div className="flex flex-wrap items-center gap-1.5 mt-1">
                          <span className="text-xs text-gray-500">{r.category}</span>
                          <span className="text-gray-300 text-xs">·</span>
                          <span className={`text-xs font-medium ${r.session_fit === 'Session-Friendly' ? 'text-green-dark' : 'text-amber-600'}`}>
                            {r.session_fit === 'Session-Friendly' ? '✓ Session-Friendly' : '⏱ Long-Cook'}
                          </span>
                          <span className="text-gray-300 text-xs">·</span>
                          <span className="text-xs text-gray-500">{(r.prep_time_min || 0) + (r.cook_time_min || 0)} min</span>
                        </div>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {/* ── INGREDIENTS TAB ─────────────────────────────────────────────── */}
      {tab === 'ingredients' && (
        <div>
          {/* Export bar */}
          <div className="flex items-center justify-between px-4 py-2.5 bg-white border-b border-gray-100 sticky top-[49px] z-10">
            <span className="text-sm text-gray-500">
              {selCount} recipe{selCount !== 1 ? 's' : ''} · Serves {servingSize}
            </span>
            <div className="flex gap-2">
              <button
                onClick={exportCSV}
                disabled={!ingredientData}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-gray-300 text-xs font-medium text-gray-700 bg-white disabled:opacity-40"
              >
                ↓ CSV
              </button>
              <button
                onClick={exportPDF}
                disabled={!ingredientData}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-navy text-white text-xs font-medium disabled:opacity-40"
              >
                ↓ PDF
              </button>
            </div>
          </div>

          {/* Loading state */}
          {calcLoading && (
            <div className="px-4 pt-8 text-center">
              <div className="text-gray-400 text-sm animate-pulse">Calculating ingredients...</div>
            </div>
          )}

          {/* Empty state */}
          {!calcLoading && !ingredientData && (
            <div className="px-4 pt-8 text-center text-gray-400 text-sm">
              Select recipes to generate the ingredient list.
            </div>
          )}

          {/* Ingredient sections */}
          {!calcLoading && ingredientData?.sections?.map(sec => (
            <div key={sec.section} className="mb-2">
              <div className="px-4 py-2 bg-gray-50 border-y border-gray-100 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span>{SECTION_ICONS[sec.section] || '🛒'}</span>
                  <span className="text-xs font-bold text-gray-600 uppercase tracking-wide">{sec.section}</span>
                </div>
                <span className="text-xs text-gray-400">{sec.items.length} item{sec.items.length !== 1 ? 's' : ''}</span>
              </div>

              <div className="px-4">
                {sec.items.map((item, idx) => (
                  <div
                    key={idx}
                    className={`py-2.5 border-b border-gray-50 ${item.multi_source ? 'bg-green-light/30' : ''}`}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="flex-1 min-w-0">
                        <span className="text-sm font-medium text-navy">{item.name}</span>
                        {item.multi_source && (
                          <span className="ml-1.5 text-xs bg-green-light text-green-dark px-1.5 py-0.5 rounded-full font-medium">
                            {item.source_dishes.length} dishes
                          </span>
                        )}
                      </div>
                      <span className="text-sm font-semibold text-navy shrink-0">{item.quantity}</span>
                    </div>
                    {item.source_dishes.length > 0 && (
                      <div className="text-xs text-gray-400 mt-0.5 truncate">
                        {item.source_dishes.join(' · ')}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}

          {!calcLoading && ingredientData?.sections?.length === 0 && (
            <div className="px-4 pt-8 text-center text-gray-400 text-sm">
              No ingredient data found for selected recipes.
            </div>
          )}

          {/* Bottom total */}
          {!calcLoading && itemCount > 0 && (
            <div className="px-4 py-4 text-center text-xs text-gray-400">
              {itemCount} unique ingredient{itemCount !== 1 ? 's' : ''} across {selCount} recipe{selCount !== 1 ? 's' : ''} · Serves {servingSize}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
