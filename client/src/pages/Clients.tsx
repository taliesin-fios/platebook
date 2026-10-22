// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, useRef, type TouchEvent } from 'react';
import { useNavigate } from 'react-router';
import PageHeader from '../components/PageHeader';
import ConfirmModal from '../components/ConfirmModal';
import { SkeletonList } from '../components/Skeleton';
import { useToast } from '../components/Toast';
import { apiCall } from '../hooks/useApi';
import type { ClientListItem } from '../types';

const ALLERGEN_ICONS: Record<string, string> = {
  Gluten: '🌾', Dairy: '🥛', Eggs: '🥚', 'Tree Nuts': '🌰',
  Peanuts: '🥜', Shellfish: '🦐', Fish: '🐟', Soy: '🫘', Sesame: '🌿',
};

export default function Clients() {
  const navigate = useNavigate();
  const { show } = useToast();
  const [clients, setClients] = useState<ClientListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<ClientListItem | null>(null);
  const [swipedId, setSwipedId] = useState<number | null>(null);
  const touchStart = useRef<Record<number, number>>({});

  const load = () => {
    setLoading(true);
    fetch('/api/clients')
      .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
      .then(d => { setClients(Array.isArray(d) ? d : []); })
      .catch(() => setClients([]))
      .finally(() => setLoading(false));
  };

  useEffect(() => { load(); }, []);

  const filtered = clients.filter(c =>
    (c.name || '').toLowerCase().includes(search.toLowerCase())
  );

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await apiCall(`/api/clients/${deleteTarget.id}`, 'DELETE');
      setClients(prev => prev.filter(c => c.id !== deleteTarget.id));
      show('Client deleted', 'info', 5000, () => {
        // undo would need restore endpoint; just inform
        load();
      });
    } catch {
      show('Could not delete client', 'error');
    }
    setDeleteTarget(null);
    setSwipedId(null);
  };

  const handleSwipeStart = (id: number, e: TouchEvent) => {
    touchStart.current[id] = e.touches[0].clientX;
  };

  const handleSwipeEnd = (id: number, e: TouchEvent) => {
    const startX = touchStart.current[id];
    if (!startX) return;
    const diff = startX - e.changedTouches[0].clientX;
    if (diff > 60) setSwipedId(id);
    else if (diff < -20) setSwipedId(null);
  };

  const HOUSEHOLD_COLORS: Record<string, string> = {
    Individual: 'bg-gray-100 text-gray-600',
    Couple: 'bg-blue-light text-navy-mid',
    'Small Family': 'bg-green-light text-green-dark',
    'Large Family': 'bg-amber-50 text-amber-700',
  };

  return (
    <div className="page">
      <PageHeader
        title="Clients"
        action={
          <button onClick={() => navigate('/clients/new')} className="text-white font-bold text-xl" style={{ minHeight: 'auto' }}>+</button>
        }
      />

      <div className="px-4 pt-3 pb-2 sticky top-[57px] z-10 bg-warm-white">
        <input
          type="text"
          placeholder="Search clients..."
          value={search}
          onChange={e => setSearch(e.target.value)}
          className="w-full"
        />
      </div>

      <div className="px-4">
        {loading ? <SkeletonList count={4} /> : filtered.length === 0 ? (
          <div className="card text-center py-10 mt-4">
            <div className="text-4xl mb-3">👤</div>
            <div className="font-semibold text-gray-700 mb-1">No clients yet</div>
            <div className="text-sm text-gray-400 mb-4">Add your first client to get started</div>
            <button
              onClick={() => navigate('/clients/new')}
              className="px-5 py-2.5 bg-navy text-white rounded-xl text-sm font-semibold"
            >
              Add Client
            </button>
          </div>
        ) : filtered.map(c => (
          <div
            key={c.id}
            className="relative mb-3 overflow-hidden rounded-card"
            onTouchStart={e => handleSwipeStart(c.id, e)}
            onTouchEnd={e => handleSwipeEnd(c.id, e)}
          >
            {/* Swipe actions */}
            <div className="absolute right-0 top-0 bottom-0 flex items-center gap-2 pr-2 bg-gray-50">
              <button
                onClick={() => { navigate(`/clients/${c.id}/edit`); setSwipedId(null); }}
                className="px-3 py-2 bg-navy-mid text-white rounded-lg text-sm font-medium"
                style={{ minHeight: '44px' }}
              >
                Edit
              </button>
              <button
                onClick={() => setDeleteTarget(c)}
                className="px-3 py-2 bg-danger text-white rounded-lg text-sm font-medium"
                style={{ minHeight: '44px' }}
              >
                Delete
              </button>
            </div>

            <div
              className="card cursor-pointer transition-transform duration-200"
              style={{ transform: swipedId === c.id ? 'translateX(-140px)' : 'translateX(0)' }}
              onClick={() => { if (swipedId === c.id) { setSwipedId(null); return; } navigate(`/clients/${c.id}`); }}
            >
              <div className="flex items-start justify-between mb-2">
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-display font-semibold text-navy">{c.name}</span>
                    {c.is_sample === 1 && (
                      <span className="text-[10px] px-1.5 py-0.5 bg-amber-100 text-amber-700 rounded font-medium">Sample</span>
                    )}
                  </div>
                  <div className="flex items-center gap-2 mt-1 flex-wrap">
                    <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${HOUSEHOLD_COLORS[c.household_type ?? ''] || 'bg-gray-100 text-gray-500'}`}>
                      {c.household_type || 'Unknown'}
                    </span>
                    <span className="text-xs text-gray-400">Serves {c.serving_size || 4}</span>
                    {c.last_session_date && (
                      <span className="text-xs text-gray-400">Last: {c.last_session_date}</span>
                    )}
                  </div>
                </div>
                <div className="flex gap-1 flex-wrap justify-end max-w-[80px]">
                  {(c.allergens || []).slice(0, 4).map(a => (
                    <span key={a} title={a} className="text-base">{ALLERGEN_ICONS[a] || '⚠️'}</span>
                  ))}
                </div>
              </div>
            </div>
          </div>
        ))}
      </div>

      <button className="fab" onClick={() => navigate('/clients/new')}>+</button>

      <ConfirmModal
        open={!!deleteTarget}
        title="Delete Client"
        message={`Remove ${deleteTarget?.name}? This cannot be undone.`}
        confirmLabel="Delete"
        onConfirm={handleDelete}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}
