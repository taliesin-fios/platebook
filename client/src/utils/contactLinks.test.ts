// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { mailtoHref, telHref } from './contactLinks';

describe('mailtoHref', () => {
  it('links a plain address', () => {
    expect(mailtoHref(' jordan.lee@example.com ')).toBe('mailto:jordan.lee@example.com');
  });

  it('gives no link for text that is not an address', () => {
    expect(mailtoHref('')).toBeNull();
    expect(mailtoHref(null)).toBeNull();
    expect(mailtoHref('call me instead')).toBeNull();
    expect(mailtoHref('a@b')).toBeNull();
  });

  it('escapes characters that would change the link', () => {
    expect(mailtoHref('a#b@example.com')).toBe('mailto:a%23b@example.com');
    expect(mailtoHref('x?subject=hi@example.com')).toBe('mailto:x%3Fsubject%3Dhi@example.com');
    expect(mailtoHref('50%off@example.com')).toBe('mailto:50%25off@example.com');
  });
});

describe('telHref', () => {
  it('keeps only the digits of a number as typed', () => {
    expect(telHref('(503) 555-0142')).toBe('tel:5035550142');
    expect(telHref('503.555.0142')).toBe('tel:5035550142');
  });

  it('keeps a leading plus', () => {
    expect(telHref('+44 20 7946 0958')).toBe('tel:+442079460958');
    expect(telHref('+44 (0)20 7946 0958')).toBe('tel:+442079460958');
  });

  it('gives no link for missing, short or wordy values', () => {
    expect(telHref(null)).toBeNull();
    expect(telHref('')).toBeNull();
    expect(telHref('12345')).toBeNull();
    expect(telHref('ask my husband 5035550142')).toBeNull();
  });

  it('gives no link when the text holds more than one plain number', () => {
    expect(telHref('(503) 555-0142 #12')).toBeNull();
    expect(telHref('503-555-0142 / 503-555-0199')).toBeNull();
    expect(telHref('5035550142 5035550199')).toBeNull();
  });
});
