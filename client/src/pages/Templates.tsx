// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router';
import PageHeader from '../components/PageHeader';
import ConfirmModal from '../components/ConfirmModal';
import { SkeletonList } from '../components/Skeleton';
import { useToast } from '../components/Toast';
import { apiCall } from '../hooks/useApi';
import type { Id, Recipe, SessionTemplate } from '../types';

const CATEGORY_COLORS: Record<string, string> = {
  'Couples':     'bg-navy text-white',
  'Families':    'bg-green-dark text-white',
  'Plant-Based': 'bg-teal-700 text-white',
  'Other':       'bg-purple-700 text-white',
};

const CategoryBadge = ({ cat }: { cat: string | null }) => (
  <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${CATEGORY_COLORS[cat ?? ''] || CATEGORY_COLORS.Other}`}>
    {cat || 'Other'}
  </span>
);

// ── Edit Modal ────────────────────────────────────────────────────────────────
interface TemplateEditModalProps {
  template: SessionTemplate;
  onSave: (updated: SessionTemplate) => void;
  onClose: () => void;
}

function TemplateEditModal({ template, onSave, onClose }: TemplateEditModalProps) {
  const [form, setForm] = useState<{ name: string; description: string; recipe_ids: Id[] }>({
    name:        template?.name        || '',
    description: template?.description || '',
    recipe_ids:  template?.recipe_ids  || [],
  });
  const [allRecipes, setAllRecipes] = useState<Recipe[]>([]);
  const [saving, setSaving] = useState(false);
  const { show } = useToast();

  useEffect(() => {
    fetch('/api/recipes')
      .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
      .then(d => setAllRecipes(Array.isArray(d) ? (d as Recipe[]).filter(r => r.is_active) : []))
      .catch(() => setAllRecipes([]));
  }, []);

  const toggleRecipe = (recipeId: Id) => {
    setForm(f => ({
      ...f,
      recipe_ids: f.recipe_ids.includes(recipeId)
        ? f.recipe_ids.filter(id => id !== recipeId)
        : [...f.recipe_ids, recipeId],
    }));
  };

  const handleSave = async () => {
    if (!form.name.trim()) { show('Template name is required', 'error'); return; }
    setSaving(true);
    try {
      const updated = await apiCall<SessionTemplate>(`/api/templates/${template.id}`, 'PUT', form);
      onSave(updated);
      show('Template saved', 'success');
    } catch {
      show('Could not save template', 'error');
    } finally {
      setSaving(false);
    }
  };

  // Group recipes by category
  const byCategory = allRecipes.reduce<Record<string, Recipe[]>>((acc, r) => {
    const cat = r.category || 'Other';
    if (!acc[cat]) acc[cat] = [];
    acc[cat].push(r);
    return acc;
  }, {});

  return (
    <div className="overlay" onClick={onClose}>
      <div
        className="modal"
        style={{ maxHeight: '88dvh', display: 'flex', flexDirection: 'column' }}
        onClick={e => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between mb-4 shrink-0">
          <h3 className="font-display font-bold text-green-dark text-lg">Edit Template</h3>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 text-xl leading-none"
            style={{ minHeight: 'auto' }}
          >×</button>
        </div>

        {/* Scrollable body */}
        <div className="overflow-y-auto flex-1 space-y-4 pb-2 pr-0.5">
          {/* Name */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1.5">
              Template Name <span className="text-danger">*</span>
            </label>
            <input
              type="text"
              value={form.name}
              onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
              placeholder="e.g. Classic Family Week"
              autoFocus
            />
          </div>

          {/* Description */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1.5">
              Description <span className="text-xs text-gray-400 font-normal">(optional)</span>
            </label>
            <textarea
              value={form.description}
              onChange={e => setForm(f => ({ ...f, description: e.target.value }))}
              placeholder="Quick note about when to use this template…"
              rows={2}
              style={{ resize: 'none' }}
            />
          </div>

          {/* Recipe picker */}
          <div>
            <div className="text-sm font-medium text-gray-700 mb-2">
              Recipes
              {form.recipe_ids.length > 0 && (
                <span className="ml-2 text-xs text-green-dark font-semibold">
                  {form.recipe_ids.length} selected
                </span>
              )}
            </div>

            {allRecipes.length === 0 ? (
              <div className="text-sm text-gray-400 py-4 text-center">Loading recipes…</div>
            ) : (
              <div className="space-y-3">
                {Object.entries(byCategory).map(([cat, recipes]) => (
                  <div key={cat}>
                    <div className="text-xs text-gray-400 font-semibold uppercase tracking-wide mb-1.5 flex items-center gap-2">
                      <CategoryBadge cat={cat} />
                    </div>
                    <div className="space-y-1">
                      {recipes.map(r => {
                        const checked = form.recipe_ids.includes(r.id);
                        return (
                          <label
                            key={r.id}
                            className={`flex items-center gap-3 px-3 py-2.5 rounded-xl cursor-pointer transition-colors ${
                              checked ? 'bg-green-light border border-green-mid/30' : 'bg-warm-white hover:bg-blue-light'
                            }`}
                          >
                            <input
                              type="checkbox"
                              checked={checked}
                              onChange={() => toggleRecipe(r.id)}
                              className="accent-green-dark w-4 h-4 shrink-0"
                              style={{ minHeight: 'auto', width: '16px', height: '16px', padding: 0, border: 'none' }}
                            />
                            <div className="min-w-0">
                              <div className="text-sm font-medium text-navy truncate">{r.name}</div>
                              {r.session_fit && (
                                <div className="text-xs text-gray-400">{r.session_fit}</div>
                              )}
                            </div>
                          </label>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex gap-2 mt-4 pt-4 border-t border-gray-100 shrink-0">
          <button
            onClick={onClose}
            className="flex-1 py-3 border border-gray-200 rounded-xl text-sm font-medium text-gray-600"
            style={{ minHeight: '44px' }}
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={saving}
            className="flex-1 py-3 bg-navy text-white rounded-xl text-sm font-semibold disabled:opacity-60"
            style={{ minHeight: '44px' }}
          >
            {saving ? 'Saving…' : 'Save Template'}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────
export default function Templates() {
  const navigate     = useNavigate();
  const { show }     = useToast();
  const [templates,    setTemplates]    = useState<SessionTemplate[]>([]);
  const [loading,      setLoading]      = useState(true);
  const [deleteTarget, setDeleteTarget] = useState<SessionTemplate | null>(null);
  const [editTarget,   setEditTarget]   = useState<SessionTemplate | null>(null);

  const load = useCallback(() => {
    setLoading(true);
    fetch('/api/templates')
      .then(r => r.json())
      .then(d => setTemplates(Array.isArray(d) ? d : []))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await apiCall(`/api/templates/${deleteTarget.id}`, 'DELETE');
      setTemplates(prev => prev.filter(t => t.id !== deleteTarget.id));
      show('Template deleted', 'info');
    } catch {
      show('Could not delete template', 'error');
    }
    setDeleteTarget(null);
  };

  const handleSaveEdit = (updated: SessionTemplate) => {
    setTemplates(prev => prev.map(t => t.id === updated.id ? updated : t));
    setEditTarget(null);
  };

  return (
    <div className="page">
      <PageHeader title="Session Templates" back="/" />

      <div className="px-4 pt-4 pb-24">
        {loading ? <SkeletonList count={3} /> : templates.length === 0 ? (
          <div className="card text-center py-10">
            <div className="text-4xl mb-3">📋</div>
            <div className="text-gray-500 text-sm">No templates yet</div>
            <p className="text-xs text-gray-400 mt-1">Save a menu as template from the session builder</p>
          </div>
        ) : templates.map(t => (
          <div key={t.id} className="card mb-3">
            <div className="flex items-start justify-between mb-2">
              <div className="flex-1 pr-2 min-w-0">
                <div className="font-display font-semibold text-navy">{t.name}</div>
                {t.is_default ? (
                  <span className="text-[10px] bg-blue-light text-navy-mid px-1.5 py-0.5 rounded font-medium">Default</span>
                ) : null}
              </div>

              {/* Action buttons */}
              <div className="flex items-center gap-1 shrink-0">
                {/* Edit */}
                <button
                  onClick={() => setEditTarget(t)}
                  className="p-2 text-gray-400 hover:text-navy rounded-lg hover:bg-blue-light transition-colors"
                  style={{ minHeight: 'auto' }}
                  title="Edit template"
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7" />
                    <path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z" />
                  </svg>
                </button>

                {/* Delete (not for default templates) */}
                {!t.is_default && (
                  <button
                    onClick={() => setDeleteTarget(t)}
                    className="p-2 text-gray-400 hover:text-danger rounded-lg hover:bg-red-50 transition-colors"
                    style={{ minHeight: 'auto' }}
                    title="Delete template"
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <polyline points="3 6 5 6 21 6" />
                      <path d="M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6" />
                      <path d="M10 11v6M14 11v6" />
                      <path d="M9 6V4h6v2" />
                    </svg>
                  </button>
                )}
              </div>
            </div>

            {t.description && (
              <p className="text-xs text-gray-500 mb-3">{t.description}</p>
            )}

            {/* Recipe chips */}
            <div className="flex flex-wrap gap-1.5 mb-3">
              {(t.recipes || []).map(r => (
                <div key={r.id} className="flex items-center gap-1.5 bg-warm-white px-2 py-1 rounded-lg">
                  <CategoryBadge cat={r.category} />
                  <span className="text-xs text-gray-700">{r.name}</span>
                </div>
              ))}
              {(t.recipe_ids || []).length > (t.recipes || []).length && (
                <span className="text-xs text-gray-400 px-2 py-1">
                  +{t.recipe_ids.length - (t.recipes || []).length} more
                </span>
              )}
              {(t.recipe_ids || []).length === 0 && (
                <span className="text-xs text-gray-400 italic">No recipes — tap Edit to add some</span>
              )}
            </div>

            <button
              onClick={() => navigate(`/sessions/new?template=${t.id}`)}
              className="w-full py-2.5 bg-navy text-white rounded-xl text-sm font-semibold"
            >
              Use Template →
            </button>
          </div>
        ))}
      </div>

      {/* Edit modal */}
      {editTarget && (
        <TemplateEditModal
          template={editTarget}
          onSave={handleSaveEdit}
          onClose={() => setEditTarget(null)}
        />
      )}

      <ConfirmModal
        open={!!deleteTarget}
        title="Delete Template"
        message={`Remove "${deleteTarget?.name}"?`}
        onConfirm={handleDelete}
        onCancel={() => setDeleteTarget(null)}
      />
    </div>
  );
}
