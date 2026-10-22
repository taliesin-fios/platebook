// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from 'react';
import { Link } from 'react-router';
import { APP_NAME } from '../constants';

/**
 * Shared shell for the signed-out pages (sign in, sign up, password reset):
 * product wordmark on top, a white card with the form, optional footer links.
 */
interface AuthLayoutProps {
  title?: ReactNode;
  subtitle?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
}

export default function AuthLayout({ title, subtitle, children, footer }: AuthLayoutProps) {
  return (
    <div className="min-h-dvh bg-ivory flex items-center justify-center px-4 py-8 font-sans">
      <div className="w-full max-w-md">
        <Link to="/welcome" className="block text-center mb-8 no-underline">
          <div className="font-display text-3xl font-bold text-green-dark leading-tight">🍳 {APP_NAME}</div>
          <div className="text-xs text-gray-400 mt-1 font-medium tracking-wide uppercase">For home &amp; personal chefs</div>
        </Link>

        <div className="bg-white rounded-2xl shadow-card p-6 sm:p-8">
          {title && <h1 className="font-display text-xl font-semibold text-olive mb-1">{title}</h1>}
          {subtitle && <p className="text-sm text-gray-500 mb-6 leading-relaxed">{subtitle}</p>}
          {children}
        </div>

        {footer && <div className="text-center text-sm text-gray-500 mt-5">{footer}</div>}
      </div>
    </div>
  );
}

/** Labelled input row used by the auth forms. */
type FieldProps = InputHTMLAttributes<HTMLInputElement> & {
  label: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
};

export function Field({ label, hint, error, ...inputProps }: FieldProps) {
  return (
    <label className="block mb-4">
      <span className="block text-sm font-medium text-gray-700 mb-1.5">{label}</span>
      <input {...inputProps} />
      {hint && !error && <span className="block text-xs text-gray-400 mt-1">{hint}</span>}
      {error && <span className="block text-xs text-berry mt-1">{error}</span>}
    </label>
  );
}

export function PrimaryButton({ children, busy, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { busy?: boolean }) {
  return (
    <button
      type="submit"
      disabled={busy || props.disabled}
      className="w-full py-3 rounded-xl bg-herb text-white font-semibold text-[15px] transition-opacity disabled:opacity-60 disabled:cursor-not-allowed"
      {...props}
    >
      {children}
    </button>
  );
}

export function Alert({ tone = 'error', children }: { tone?: 'error' | 'info'; children?: ReactNode }) {
  const styles = tone === 'error'
    ? 'bg-red-50 border-red-200 text-red-700'
    : 'bg-sage border-herb/40 text-green-dark';
  return <div className={`border rounded-lg px-3.5 py-3 mb-5 text-sm ${styles}`} role={tone === 'error' ? 'alert' : 'status'}>{children}</div>;
}
