// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useAuth } from '../App';
import { fmtUtcTimestamp } from '../utils/dateUtils';

/**
 * Shown when the last meal-plan, invoice or booking confirmation email failed
 * (the server records `pdf_email_failed_at` / `email_failed_at` /
 * `confirmation_email_failed_at` and clears it on the next successful send),
 * so the chef sees it even after leaving the page.
 */
export default function EmailFailedBanner({ what, failedAt, onRetry, retrying, onDownload, className = 'mx-4 mt-3' }: {
  /** e.g. "meal plan", "invoice" */
  what: string;
  failedAt: string | null | undefined;
  onRetry: () => void;
  retrying: boolean;
  /** Offers the PDF instead, for the chef to send themselves. */
  onDownload?: () => void;
  /** Spacing around the banner (defaults to page margins). */
  className?: string;
}) {
  // On a server with no email at all, "Try again" can never work.
  const canSend = useAuth().chef?.email_available !== false;
  if (!failedAt) return null;
  return (
    <div role="alert" className={`${className} flex items-center gap-3 rounded-xl border border-red-200 bg-red-50 px-3.5 py-3 text-sm text-red-700`}>
      <span className="flex-1">
        ⚠ The {what} email wasn’t sent ({fmtUtcTimestamp(failedAt, "MMM d 'at' h:mm a")}).
        {!canSend && ' Email isn’t set up on this Platebook server.'}
        {onDownload && ' You can download the PDF and send it yourself.'}
      </span>
      <div className="flex shrink-0 flex-wrap justify-end gap-2">
        {onDownload && (
          <button type="button" onClick={onDownload}
            className="rounded-lg bg-white px-3 py-1.5 font-semibold text-red-700 border border-red-200"
            style={{ minHeight: 'auto' }}>
            Download PDF
          </button>
        )}
        {canSend && (
          <button type="button" onClick={onRetry} disabled={retrying}
            className="rounded-lg bg-white px-3 py-1.5 font-semibold text-red-700 border border-red-200 disabled:opacity-60"
            style={{ minHeight: 'auto' }}>
            {retrying ? 'Sending…' : 'Try again'}
          </button>
        )}
      </div>
    </div>
  );
}
