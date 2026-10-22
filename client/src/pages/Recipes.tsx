// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, useRef, type TouchEvent } from 'react';
import { useNavigate } from 'react-router';
import PageHeader from '../components/PageHeader';
import ConfirmModal from '../components/ConfirmModal';
import { SkeletonList } from '../components/Skeleton';
import { useToast } from '../components/Toast';
import { apiCall } from '../hooks/useApi';
import { StarDisplay as SD } from '../components/StarRating';
import type { Recipe } from '../types';

const CATEGORY_TABS = ['All', 'Couples', 'Families', 'Plant-Based', 'Other'];
const SESSION_FIT_TABS = ['All', 'Session-Friendly', 'Long-Cook'];

const CategoryBadge = ({ cat }: { cat: string | null }) => (
  <span className={`text-[11px] px-2 py-0.5 rounded-full font-medium ${
    cat === 'Couples' ? 'bg-navy text-white' :
    cat === 'Families' ? 'bg-green-dark text-white' :
    cat === 'Plant-Based' ? 'bg-teal-700 text-white' :
    'bg-purple-700 text-white'
  }`}>{cat}</span>
);

const FitBadge = ({ fit }: { fit: string | null }) => (
  <span className={`text-[11px] px-2 py-0.5 rounded-full font-medium ${
    fit === 'Session-Friendly' ? 'bg-green-light text-green-dark' : 'bg-amber-50 text-amber-700'
  }`}>
    {fit === 'Session-Friendly' ? '✓ Session-Friendly' : '⏱ Long-Cook'}
  </span>
);

export default function Recipes() {
  const navigate = useNavigate();
  const { show } = useToast();
  const [recipes, setRecipes] = useState<Recipe[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [catTab, setCatTab] = useState('All');
  const [fitTab, setFitTab] = useState('All');
  const [deleteTarget, setDeleteTarget] = useState<Recipe | null>(null);
  const [swipedId, setSwipedId] = useState<number | null>(null);
  const [fabOpen, setFabOpen] = useState(false);
  const touchStart = useRef<Record<number, number>>({});

  const load = () => {
    setLoading(true);
    fetch('/api/recipes')
      .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
      .then(d => setRecipes(Array.isArray(d) ? d : []))
      .catch(() => setRecipes([]))
      .finally(() => setLoading(false));
  };

  useEffect(() => { load(); }, []);

  const filtered = recipes.filter(r => {
    if (search && !(r.name || '').toLowerCase().includes(search.toLowerCase())) return false;
    if (catTab !== 'All' && r.category !== catTab) return false;
    if (fitTab !== 'All' && r.session_fit !== fitTab) return false;
    return true;
  });

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await apiCall(`/api/recipes/${deleteTarget.id}`, 'DELETE');
      setRecipes(prev => prev.filter(r => r.id !== deleteTarget.id));
      show('Recipe removed', 'info');
    } catch {
      show('Could not delete recipe', 'error');
    }
    setDeleteTarget(null);
    setSwipedId(null);
  };

  const handleSwipeStart = (id: number, e: TouchEvent) => { touchStart.current[id] = e.touches[0].clientX; };
  const handleSwipeEnd = (id: number, e: TouchEvent) => {
    const diff = touchStart.current[id] - e.changedTouches[0].clientX;
    if (diff > 60) setSwipedId(id);
    else if (diff < -20) setSwipedId(null);
  };

  return (
    <div className="page">
      <PageHeader title="Recipe Library" />

      {/* Search */}
      <div className="px-4 pt-3 sticky top-[57px] z-10 bg-warm-white">
        <input
          type="text"
          placeholder="Search recipes..."
          value={search}
          onChange={e => setSearch(e.target.value)}
          className="w-full mb-2"
        />

        {/* Category tabs */}
        <div className="flex gap-2 overflow-x-auto no-scrollbar pb-2">
          {CATEGORY_TABS.map(t => (
            <button
              key={t}
              onClick={() => setCatTab(t)}
              className={`shrink-0 px-3 py-1.5 rounded-full text-xs font-medium border transition-colors ${
                catTab === t ? 'bg-navy text-white border-navy' : 'bg-white text-gray-600 border-gray-200'
              }`}
              style={{ minHeight: '32px' }}
            >
              {t}
            </button>
          ))}
          <div className="w-px bg-gray-200 mx-1" />
          {SESSION_FIT_TABS.slice(1).map(t => (
            <button
              key={t}
              onClick={() => setFitTab(prev => prev === t ? 'All' : t)}
              className={`shrink-0 px-3 py-1.5 rounded-full text-xs font-medium border transition-colors ${
                fitTab === t
                  ? t === 'Session-Friendly' ? 'bg-green-dark text-white border-green-dark' : 'bg-amber-500 text-white border-amber-500'
                  : 'bg-white text-gray-600 border-gray-200'
              }`}
              style={{ minHeight: '32px' }}
            >
              {t}
            </button>
          ))}
        </div>
      </div>

      <div className="px-4">
        {loading ? <SkeletonList count={5} /> : filtered.length === 0 ? (
          <div className="card text-center py-10 mt-2">
            <div className="text-4xl mb-2">📖</div>
            <div className="text-gray-500 text-sm">No recipes found</div>
          </div>
        ) : filtered.map(r => (
          <div
            key={r.id}
            className="relative mb-3 overflow-hidden rounded-card"
            onTouchStart={e => handleSwipeStart(r.id, e)}
            onTouchEnd={e => handleSwipeEnd(r.id, e)}
          >
            <div className="absolute right-0 top-0 bottom-0 flex items-center gap-2 pr-2 bg-gray-50">
              <button
                onClick={() => { navigate(`/recipes/${r.id}/edit`); setSwipedId(null); }}
                className="px-3 py-2 bg-navy-mid text-white rounded-lg text-sm font-medium"
                style={{ minHeight: '44px' }}
              >
                Edit
              </button>
              <button
                onClick={() => setDeleteTarget(r)}
                className="px-3 py-2 bg-danger text-white rounded-lg text-sm font-medium"
                style={{ minHeight: '44px' }}
              >
                Delete
              </button>
            </div>

            <div
              className="card cursor-pointer transition-transform duration-200"
              style={{ transform: swipedId === r.id ? 'translateX(-140px)' : 'translateX(0)' }}
              onClick={() => { if (swipedId === r.id) { setSwipedId(null); return; } navigate(`/recipes/${r.id}`); }}
            >
              <div className="flex items-start justify-between mb-2">
                <div className="font-display font-semibold text-navy flex-1 pr-2">{r.name}</div>
                <CategoryBadge cat={r.category} />
              </div>
              <div className="flex items-center gap-2 flex-wrap mb-2">
                <FitBadge fit={r.session_fit} />
                <span className="text-xs text-gray-500">
                  {(r.prep_time_min || 0) + (r.cook_time_min || 0)} min total
                </span>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs text-gray-400">Hold</span>
                <SD value={r.hold_quality || 0} size={12} />
              </div>
            </div>
          </div>
        ))}
      </div>

      {/* FAB with menu */}
      {fabOpen && (
        <div className="fixed inset-0 z-30" onClick={() => setFabOpen(false)}>
          <div className="absolute bottom-[148px] right-5 flex flex-col gap-2 items-end" onClick={e => e.stopPropagation()}>
            <button
              onClick={() => { navigate('/planner'); setFabOpen(false); }}
              className="flex items-center gap-2 bg-green-dark text-white px-4 py-2.5 rounded-xl shadow-lg text-sm font-medium"
            >
              📋 Menu Planner
            </button>
            <button
              onClick={() => { navigate('/recipes/import'); setFabOpen(false); }}
              className="flex items-center gap-2 bg-navy text-white px-4 py-2.5 rounded-xl shadow-lg text-sm font-medium"
            >
              Import Recipe
            </button>
            <button
              onClick={() => { navigate('/recipes/new'); setFabOpen(false); }}
              className="flex items-center gap-2 bg-navy text-white px-4 py-2.5 rounded-xl shadow-lg text-sm font-medium"
            >
              Add Manually
            </button>
          </div>
        </div>
      )}
      <button className="fab" onClick={() => setFabOpen(prev => !prev)}>
        <span style={{ fontSize: '24px', transition: 'transform 0.2s', transform: fabOpen ? 'rotate(45deg)' : 'none' }}>+</span>
      </button>

      <ConfirmModal
        open={!!deleteTarget}
        title="Remove Recipe"
        message={`Remove "${deleteTarget?.name}" from the library?`}
        onConfirm={handleDelete}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}
