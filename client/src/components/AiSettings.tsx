// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useEffect, useState } from 'react';
import ConfirmModal from './ConfirmModal';
import { useToast } from './Toast';
import { apiCall, errorMessage } from '../hooks/useApi';
import type { AiStatus } from '../types';

/** Where to get a key, and an example model, for each provider. */
const PROVIDER_HELP: Record<string, { keys?: string; model: string }> = {
  anthropic:  { keys: 'https://console.anthropic.com/settings/keys', model: 'Leave blank for the default (Claude Sonnet)' },
  openai:     { keys: 'https://platform.openai.com/api-keys', model: 'e.g. gpt-5-mini' },
  gemini:     { keys: 'https://aistudio.google.com/apikey', model: 'e.g. gemini-2.5-flash' },
  openrouter: { keys: 'https://openrouter.ai/settings/keys', model: 'e.g. openai/gpt-5-mini' },
  mistral:    { keys: 'https://console.mistral.ai/api-keys', model: 'e.g. mistral-small-latest' },
  groq:       { keys: 'https://console.groq.com/keys', model: 'e.g. llama-3.3-70b-versatile' },
  custom:     { model: 'The model name your service uses' },
};

/** Settings → AI Assistant: each chef brings their own provider and key. */
export default function AiSettings() {
  const { show } = useToast();
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [provider, setProvider] = useState('');
  const [model, setModel] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [removeConfirm, setRemoveConfirm] = useState(false);

  const load = (s: AiStatus) => {
    setStatus(s);
    setProvider(s.provider ?? '');
    setModel(s.model ?? '');
    setBaseUrl(s.base_url ?? '');
    setApiKey('');
  };

  useEffect(() => {
    apiCall<AiStatus>('/api/ai/status').then(load).catch(() => {});
  }, []);

  if (!status) return <div className="skeleton h-24 w-full" />;

  const info = status.providers.find(p => p.id === provider);
  const help = PROVIDER_HELP[provider];
  const savedProvider = status.providers.find(p => p.id === status.provider);
  // A saved key belongs to the saved provider; switching needs a new one.
  const keyKept = !!status.key_hint && provider === status.provider && !status.key_unreadable;

  const save = async () => {
    setSaving(true);
    try {
      const saved = await apiCall<AiStatus>('/api/ai/settings', 'PUT', {
        provider, model, base_url: baseUrl, api_key: apiKey,
      });
      load(saved);
      show('AI settings saved', 'success');
    } catch (err) {
      show(errorMessage(err) || 'Could not save AI settings', 'error');
    } finally {
      setSaving(false);
    }
  };

  const test = async () => {
    setTesting(true);
    try {
      const result = await apiCall<{ working: boolean; error?: string }>('/api/ai/test', 'POST');
      if (result.working) show('AI connection works ✓', 'success');
      else show(result.error || 'The provider refused the request: check the key and model', 'error');
    } catch (err) {
      show(errorMessage(err) || 'AI is not set up', 'error');
    } finally {
      setTesting(false);
    }
  };

  const remove = async () => {
    setRemoveConfirm(false);
    try {
      load(await apiCall<AiStatus>('/api/ai/settings', 'DELETE'));
      show('AI settings removed', 'success');
    } catch (err) {
      show(errorMessage(err) || 'Could not remove AI settings', 'error');
    }
  };

  let statusText: string;
  if (status.key_unreadable) statusText = 'Your saved key needs to be entered again';
  else if (status.configured && status.source === 'account') statusText = `Using your ${savedProvider?.label ?? 'AI'} key`;
  else if (status.configured) statusText = "Using this server's Claude key";
  else statusText = 'Not set up';

  return (
    <div className="space-y-3">
      <p className="text-xs text-gray-500">
        AI powers menu suggestions when you plan a session.
        Choose a provider and add your own API key: it's stored encrypted, used only for your account,
        and the provider bills you directly.
      </p>
      <div className="flex items-center justify-between gap-2 p-3 bg-warm-white rounded-xl">
        <div className="text-sm min-w-0">
          <span className="font-medium">Status: </span>
          <span className={status.configured ? 'text-green-dark font-semibold' : 'text-danger font-semibold'}>
            ● {statusText}
          </span>
        </div>
        <button
          onClick={test}
          disabled={testing || !status.configured}
          className="shrink-0 px-3 py-2 bg-navy text-white rounded-xl text-xs font-medium disabled:opacity-60"
          style={{ minHeight: '36px' }}
        >
          {testing ? 'Testing...' : 'Test Connection'}
        </button>
      </div>

      <div>
        <label htmlFor="ai-provider" className="block text-sm font-medium text-gray-700 mb-1.5">Provider</label>
        <select id="ai-provider" value={provider} onChange={e => setProvider(e.target.value)}>
          <option value="">{status.server_key ? "Use this server's Claude key" : 'Choose a provider…'}</option>
          {status.providers.map(p => <option key={p.id} value={p.id}>{p.label}</option>)}
        </select>
      </div>

      {info && (
        <>
          <div>
            <label htmlFor="ai-model" className="block text-sm font-medium text-gray-700 mb-1.5">
              Model{info.needs_model ? '' : ' (optional)'}
            </label>
            <input id="ai-model" value={model} onChange={e => setModel(e.target.value)}
              placeholder={help?.model} autoComplete="off" spellCheck={false} />
          </div>
          {!info.default_base_url && (
            <div>
              <label htmlFor="ai-base-url" className="block text-sm font-medium text-gray-700 mb-1.5">Base URL</label>
              <input id="ai-base-url" inputMode="url" value={baseUrl} onChange={e => setBaseUrl(e.target.value)}
                placeholder="https://api.example.com/v1" autoComplete="off" spellCheck={false} />
              <p className="text-xs text-gray-400 mt-1">
                Any service with an OpenAI-compatible Chat Completions API.
                {status.allow_private_urls ? '' : ' It must be a public https:// address.'}
              </p>
            </div>
          )}
          <div>
            <label htmlFor="ai-key" className="block text-sm font-medium text-gray-700 mb-1.5">
              API Key{info.needs_key ? '' : ' (optional)'}
            </label>
            <input id="ai-key" type="password" value={apiKey} onChange={e => setApiKey(e.target.value)}
              placeholder={keyKept ? `Saved key ending ${status.key_hint}. Leave blank to keep it` : 'Paste your API key'}
              autoComplete="off" spellCheck={false} />
            {help?.keys && (
              <p className="text-xs text-gray-400 mt-1">
                Get a key from <a href={help.keys} target="_blank" rel="noopener noreferrer" className="underline">{info.label}</a>.
              </p>
            )}
          </div>
        </>
      )}

      <div className="flex flex-wrap gap-2">
        {info && (
          <button onClick={save} disabled={saving}
            className="px-4 py-2 bg-navy text-white rounded-xl text-sm font-medium disabled:opacity-60"
            style={{ minHeight: '40px' }}>
            {saving ? 'Saving...' : 'Save AI Settings'}
          </button>
        )}
        {status.provider && (
          <button onClick={() => setRemoveConfirm(true)}
            className="px-4 py-2 border border-gray-200 text-danger rounded-xl text-sm font-medium hover:bg-red-50 transition-colors"
            style={{ minHeight: '40px' }}>
            Remove My AI Settings
          </button>
        )}
      </div>

      <ConfirmModal
        open={removeConfirm}
        title="Remove AI Settings"
        message={status.server_key
          ? "This deletes your saved provider and key. You'll use this server's Claude key instead."
          : 'This deletes your saved provider and key. AI features stop until you add one again.'}
        confirmLabel="Remove"
        onConfirm={remove}
        onCancel={() => setRemoveConfirm(false)}
      />
    </div>
  );
}
