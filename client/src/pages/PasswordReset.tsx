// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, type FormEvent } from 'react';
import { Link, useSearchParams } from 'react-router';
import AuthLayout, { Field, PrimaryButton, Alert } from '../components/AuthLayout';
import { apiCall, errorMessage } from '../hooks/useApi';
import { PASSWORD_MIN_LENGTH } from '../constants';
import type { Chef, LinkSentResponse } from '../types';

const backToSignIn = <Link to="/login" className="font-semibold text-green-dark">Back to sign in</Link>;

/** /forgot — request a reset email */
export function ForgotPassword() {
  const [email, setEmail] = useState('');
  const [busy, setBusy]   = useState(false);
  const [sent, setSent]   = useState(false);
  const [devLink, setDevLink] = useState<string | null>(null);
  const [error, setError] = useState('');

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError('');
    try {
      const data = await apiCall<LinkSentResponse>('/api/auth/forgot-password', 'POST', { email });
      setSent(true);
      if (data.dev_link) setDevLink(data.dev_link);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <AuthLayout
      title="Reset your password"
      subtitle={sent ? null : 'Enter your account email and we’ll send you a link to choose a new password.'}
      footer={backToSignIn}
    >
      {error && <Alert>{error}</Alert>}
      {sent ? (
        <>
          <Alert tone="info">If an account exists for <strong>{email}</strong>, a reset link is on its way. It expires in 30 minutes.</Alert>
          {devLink && <a href={devLink} className="text-sm text-green-dark break-all">Dev mode: open reset link →</a>}
        </>
      ) : (
        <form onSubmit={handleSubmit} noValidate>
          <Field label="Email" type="email" value={email} onChange={e => setEmail(e.target.value)} autoComplete="email" autoFocus required />
          <PrimaryButton busy={busy}>{busy ? 'Sending…' : 'Send reset link'}</PrimaryButton>
        </form>
      )}
    </AuthLayout>
  );
}

/** /reset?token= — choose a new password */
export function ResetPassword() {
  const [searchParams] = useSearchParams();
  const token = searchParams.get('token') || '';
  const [password, setPassword] = useState('');
  const [busy, setBusy]   = useState(false);
  const [error, setError] = useState(token ? '' : 'This reset link is incomplete. Request a new one.');

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (password.length < PASSWORD_MIN_LENGTH) {
      setError(`Password must be at least ${PASSWORD_MIN_LENGTH} characters.`);
      return;
    }
    setBusy(true);
    setError('');
    try {
      const me = await apiCall<Chef>('/api/auth/reset-password', 'POST', { token, password });
      window.location.href = me.onboarding_completed ? '/' : '/onboarding';
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  return (
    <AuthLayout
      title="Choose a new password"
      footer={<Link to="/forgot" className="font-semibold text-green-dark">Request a new link</Link>}
    >
      {error && <Alert>{error}</Alert>}
      <form onSubmit={handleSubmit} noValidate>
        <Field
          label="New password"
          type="password"
          hint={`At least ${PASSWORD_MIN_LENGTH} characters`}
          value={password}
          onChange={e => setPassword(e.target.value)}
          autoComplete="new-password"
          autoFocus
          required
        />
        <PrimaryButton busy={busy} disabled={!token}>{busy ? 'Saving…' : 'Save password & sign in'}</PrimaryButton>
      </form>
    </AuthLayout>
  );
}
