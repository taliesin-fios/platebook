// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, expect, it } from 'vitest';
import { emailSentMessage, errorCode, isRecordedEmailFailure } from './emailResult';

describe('emailSentMessage', () => {
  it('says accepted by the mail server, never delivered', () => {
    const msg = emailSentMessage('Invoice', { sent_to: 'a@example.com', delivery: 'smtp' });
    expect(msg).toBe('Invoice sent to a@example.com (accepted by the mail server)');
    expect(msg).not.toMatch(/deliver/i);
  });
  it('never calls captured mail sent', () => {
    const msg = emailSentMessage('Meal plan', { sent_to: 'a@example.com', delivery: 'captured' });
    expect(msg).toMatch(/captured/);
    expect(msg).toMatch(/not sent/);
  });
});

describe('isRecordedEmailFailure', () => {
  it('is true only for sends the server attempted and recorded', () => {
    expect(isRecordedEmailFailure('EMAIL_NOT_CONFIGURED')).toBe(true);
    expect(isRecordedEmailFailure('EMAIL_SEND_FAILED')).toBe(true);
    expect(isRecordedEmailFailure('NO_EMAIL')).toBe(false);
    expect(isRecordedEmailFailure('RATE_LIMITED')).toBe(false);
    expect(isRecordedEmailFailure(undefined)).toBe(false);
  });
  it('reads the code off an apiCall error', () => {
    expect(errorCode(Object.assign(new Error('x'), { code: 'EMAIL_SEND_FAILED' }))).toBe('EMAIL_SEND_FAILED');
    expect(errorCode(new Error('x'))).toBeUndefined();
    expect(errorCode(null)).toBeUndefined();
  });
});
