// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, type FormEvent } from 'react';
import { apiCall, errorMessage } from '../hooks/useApi';

interface Props {
  /** The signed-in account's email: typing it back is the confirmation. */
  email: string;
}

/**
 * Settings → Delete account. Two steps: a button that opens the form, then
 * the account email typed back (and the password, when the account has one)
 * before DELETE /api/auth/account runs. Afterwards the app reloads at /welcome.
 */
export default function DeleteAccount({ email }: Props) {
  const [open, setOpen] = useState(false);
  const [typed, setTyped] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [deleting, setDeleting] = useState(false);
  const matches = typed.trim().toLowerCase() === email.trim().toLowerCase();

  const close = () => { setOpen(false); setTyped(''); setPassword(''); setError(''); };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!matches || deleting) return;
    setDeleting(true);
    setError('');
    try {
      await apiCall('/api/auth/account', 'DELETE', { confirm_email: typed.trim(), password });
      window.location.assign('/welcome');
    } catch (err) {
      setError(errorMessage(err));
      setDeleting(false);
    }
  };

  if (!open) {
    return (
      <button type="button" onClick={() => setOpen(true)}
        className="w-full flex items-center justify-between p-3 bg-red-50 rounded-xl text-sm font-medium text-danger"
        style={{ minHeight: '50px' }}>
        <span>Delete My Account…</span>
        <span aria-hidden="true">⚠</span>
      </button>
    );
  }

  return (
    <form onSubmit={submit} className="p-4 rounded-xl border border-red-200 bg-red-50 space-y-3" aria-labelledby="delete-account-title">
      <div id="delete-account-title" className="text-sm font-semibold text-danger">Delete your account and all its data</div>
      <p className="text-sm text-gray-700">
        This permanently deletes your account, settings, clients, recipes, sessions, grocery lists, invoices,
        booking inquiries and payment records from this server. Your booking link and any invoice or booking
        links you sent stop working. It can’t be undone. Download <strong>Export All My Data</strong> first if you
        want a copy.
      </p>
      <p className="text-xs text-gray-500">
        Copies in this server’s backups are not removed straight away; they age out on the server owner’s backup schedule.
      </p>
      <div>
        <label htmlFor="delete-account-email" className="block text-sm font-medium text-gray-700 mb-1">
          Type your account email (<span className="font-mono">{email}</span>) to confirm
        </label>
        <input id="delete-account-email" type="email" autoComplete="off" value={typed}
          onChange={e => setTyped(e.target.value)} />
      </div>
      <div>
        <label htmlFor="delete-account-password" className="block text-sm font-medium text-gray-700 mb-1">
          Password <span className="text-gray-400 font-normal">(leave empty if you only sign in with emailed links)</span>
        </label>
        <input id="delete-account-password" type="password" autoComplete="current-password" value={password}
          onChange={e => setPassword(e.target.value)} />
      </div>
      {error && <p role="alert" className="text-sm text-danger">{error}</p>}
      <div className="flex gap-2">
        <button type="button" onClick={close} disabled={deleting}
          className="flex-1 py-3 rounded-xl border border-gray-200 bg-white text-gray-700 font-semibold text-sm">
          Cancel
        </button>
        <button type="submit" disabled={!matches || deleting}
          className="flex-1 py-3 rounded-xl bg-danger text-white font-semibold text-sm disabled:opacity-50">
          {deleting ? 'Deleting…' : 'Delete Permanently'}
        </button>
      </div>
    </form>
  );
}
