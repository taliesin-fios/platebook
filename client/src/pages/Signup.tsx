// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, type ChangeEvent, type FormEvent } from 'react';
import { Link } from 'react-router';
import AuthLayout, { Field, PrimaryButton, Alert } from '../components/AuthLayout';
import { apiCall, errorMessage } from '../hooks/useApi';
import { useSignupOpen } from '../hooks/useSignupOpen';
import { PASSWORD_MIN_LENGTH } from '../constants';

export default function Signup() {
  const [form, setForm] = useState({ business_name: '', email: '', password: '' });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const signupOpen = useSignupOpen();
  const set = (k: keyof typeof form) => (e: ChangeEvent<HTMLInputElement>) => setForm(prev => ({ ...prev, [k]: e.target.value }));

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError('');
    if (form.password.length < PASSWORD_MIN_LENGTH) {
      setError(`Password must be at least ${PASSWORD_MIN_LENGTH} characters.`);
      return;
    }
    setBusy(true);
    try {
      await apiCall('/api/auth/signup', 'POST', form);
      // Full reload so the auth gate picks up the new session cookie
      window.location.href = '/onboarding';
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  if (!signupOpen) {
    return (
      <AuthLayout
        title="Sign-up is closed"
        subtitle="This server isn't taking new accounts. If you should have one, ask its owner."
        footer={<>Already have an account? <Link to="/login" className="font-semibold text-green-dark">Sign in</Link></>}
      />
    );
  }

  return (
    <AuthLayout
      title="Create your account"
      subtitle="Set up your chef business in a few minutes. You can change everything later."
      footer={<>Already have an account? <Link to="/login" className="font-semibold text-green-dark">Sign in</Link></>}
    >
      {error && <Alert>{error}</Alert>}
      <form onSubmit={handleSubmit} noValidate>
        <Field
          label="Business name"
          hint="What clients will see, e.g. “Sunday Suppers” or your own name"
          value={form.business_name}
          onChange={set('business_name')}
          type="text"
          maxLength={120}
          autoComplete="organization"
          autoFocus
          required
        />
        <Field label="Email" type="email" value={form.email} onChange={set('email')} autoComplete="email" required />
        <Field
          label="Password"
          type="password"
          hint={`At least ${PASSWORD_MIN_LENGTH} characters`}
          value={form.password}
          onChange={set('password')}
          autoComplete="new-password"
          minLength={PASSWORD_MIN_LENGTH}
          required
        />
        <PrimaryButton busy={busy}>{busy ? 'Creating account…' : 'Create account'}</PrimaryButton>
      </form>
    </AuthLayout>
  );
}
