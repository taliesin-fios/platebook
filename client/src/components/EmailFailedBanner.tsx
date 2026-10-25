// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { fmtUtcTimestamp } from '../utils/dateUtils';

/**
 * Shown when the last meal-plan, invoice or booking confirmation email failed
 * (the server records `pdf_email_failed_at` / `email_failed_at` /
 * `confirmation_email_failed_at` and clears it on the next successful send),
 * so the chef sees it even after leaving the page.
 */
export default function EmailFailedBanner({ what, failedAt, onRetry, retrying, className = 'mx-4 mt-3' }: {
  /** e.g. "meal plan", "invoice" */
  what: string;
  failedAt: string | null | undefined;
  onRetry: () => void;
  retrying: boolean;
  /** Spacing around the banner (defaults to page margins). */
  className?: string;
}) {
  if (!failedAt) return null;
  return (
    <div role="alert" className={`${className} flex items-center gap-3 rounded-xl border border-red-200 bg-red-50 px-3.5 py-3 text-sm text-red-700`}>
      <span className="flex-1">
        ⚠ The {what} email didn’t go through ({fmtUtcTimestamp(failedAt, "MMM d 'at' h:mm a")}).
      </span>
      <button type="button" onClick={onRetry} disabled={retrying}
        className="shrink-0 rounded-lg bg-white px-3 py-1.5 font-semibold text-red-700 border border-red-200 disabled:opacity-60"
        style={{ minHeight: 'auto' }}>
        {retrying ? 'Sending…' : 'Try again'}
      </button>
    </div>
  );
}
