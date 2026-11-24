// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import type { EmailSentResponse } from '../types';

/**
 * Error codes for a send the server attempted and recorded as failed
 * (`*_email_failed_at`). Others (rate limit, no address, demo) aren't failures
 * of the email itself, so they don't show the "didn't go through" banner.
 */
const RECORDED_FAILURES = new Set(['EMAIL_NOT_CONFIGURED', 'EMAIL_SEND_FAILED', 'INVALID_EMAIL']);

export function isRecordedEmailFailure(code: unknown): boolean {
  return typeof code === 'string' && RECORDED_FAILURES.has(code);
}

/** The `code` an `apiCall` error carries, if any. */
export function errorCode(err: unknown): string | undefined {
  const code = (err as { code?: unknown } | null)?.code;
  return typeof code === 'string' ? code : undefined;
}

/**
 * The toast after a successful send. "Sent" means the mail server accepted
 * it, which is all Platebook can know; captured mail (dev/test) never says sent.
 */
export function emailSentMessage(what: string, res: Pick<EmailSentResponse, 'sent_to' | 'delivery'>): string {
  if (res.delivery === 'captured') {
    return `${what} captured by the local test outbox — not sent to ${res.sent_to}`;
  }
  return `${what} sent to ${res.sent_to} (accepted by the mail server)`;
}
