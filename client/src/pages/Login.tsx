// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, type FormEvent } from 'react';
import { Link, useSearchParams } from 'react-router';
import AuthLayout, { Field, PrimaryButton, Alert } from '../components/AuthLayout';
import { apiCall, errorMessage } from '../hooks/useApi';
import { useSignupOpen } from '../hooks/useSignupOpen';
import type { Chef, LinkSentResponse } from '../types';

// Map server redirect error codes (from magic links) → friendly messages
const LINK_ERRORS: Record<string, string> = {
  expired:           'That sign-in link has expired. Request a new one below.',
  invalid_token:     'That sign-in link is invalid. Request a new one below.',
  missing_token:     'Incomplete sign-in link. Request a new one below.',
  link_already_used: 'That sign-in link was already used. Request a new one below.',
};

export default function Login() {
  const [searchParams] = useSearchParams();
  const signupOpen = useSignupOpen();
  const [mode, setMode]       = useState<'password' | 'link'>('password');
  const [email, setEmail]     = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy]       = useState(false);
  const [error, setError]     = useState('');
  const [linkSent, setLinkSent] = useState(false);
  const [devLink, setDevLink] = useState<string | null>(null);

  const urlError = LINK_ERRORS[searchParams.get('error') ?? ''];

  const handlePassword = async (e: FormEvent) => {
    e.preventDefault();
    setError('');
    setBusy(true);
    try {
      const me = await apiCall<Chef>('/api/auth/login', 'POST', { email, password });
      window.location.href = me.onboarding_completed ? '/' : '/onboarding';
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  const handleLink = async (e: FormEvent) => {
    e.preventDefault();
    setError('');
    setBusy(true);
    try {
      const data = await apiCall<LinkSentResponse>('/api/auth/request-link', 'POST', { email });
      setLinkSent(true);
      if (data.dev_link) setDevLink(data.dev_link); // dev-only shortcut (ALLOW_DEV_LINK)
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const switchMode = (next: 'password' | 'link') => { setMode(next); setError(''); setLinkSent(false); setDevLink(null); };

  if (linkSent) {
    return (
      <AuthLayout title="Check your inbox">
        <div className="text-center">
          <div className="text-5xl mb-4" aria-hidden="true">📬</div>
          <p className="text-sm text-gray-600 leading-relaxed mb-5">
            If an account exists for <strong className="text-olive">{email}</strong>, a sign-in link is on its way.
            It expires in 15 minutes.
          </p>
          {devLink && (
            <div className="bg-sage border border-herb rounded-lg p-3.5 mb-4 text-left">
              <div className="text-[11px] font-bold text-green-dark uppercase tracking-wide mb-1.5">Dev mode — no SMTP configured</div>
              <a href={devLink} className="text-sm text-green-dark break-all">Click here to sign in →</a>
            </div>
          )}
          <button type="button" onClick={() => switchMode('password')} className="text-sm font-medium text-herb underline">
            Back to sign in
          </button>
        </div>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout
      title="Welcome back"
      subtitle={mode === 'password' ? 'Sign in to your kitchen office.' : 'We’ll email you a one-click sign-in link. No password needed.'}
      footer={signupOpen ? <>New here? <Link to="/signup" className="font-semibold text-green-dark">Create an account</Link></> : undefined}
    >
      {urlError && <Alert>{urlError}</Alert>}
      {error && <Alert>{error}</Alert>}

      <form onSubmit={mode === 'password' ? handlePassword : handleLink} noValidate>
        <Field label="Email" type="email" value={email} onChange={e => setEmail(e.target.value)} autoComplete="email" autoFocus required />
        {mode === 'password' && (
          <Field label="Password" type="password" value={password} onChange={e => setPassword(e.target.value)} autoComplete="current-password" required />
        )}
        <PrimaryButton busy={busy}>
          {mode === 'password' ? (busy ? 'Signing in…' : 'Sign in') : (busy ? 'Sending…' : 'Email me a sign-in link')}
        </PrimaryButton>
      </form>

      <div className="flex flex-wrap justify-between gap-2 mt-5 text-sm">
        <button
          type="button"
          onClick={() => switchMode(mode === 'password' ? 'link' : 'password')}
          className="font-medium text-herb underline"
        >
          {mode === 'password' ? 'Email me a sign-in link instead' : 'Use my password instead'}
        </button>
        <Link to="/forgot" className="font-medium text-gray-500 underline">Forgot password?</Link>
      </div>
    </AuthLayout>
  );
}
