// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * `mailto:` link for an address a client typed, or `null` when it isn't one
 * (the text is then shown as plain text, not as a dead link).
 */
export function mailtoHref(email: string | null | undefined): string | null {
  const value = (email ?? '').trim();
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value) ? `mailto:${encodeURIComponent(value).replace('%40', '@')}` : null;
}

/**
 * `tel:` link for a phone number as typed ("(503) 555-0142" → "tel:5035550142").
 * Keeps a leading "+" and drops the "(0)" of "+44 (0)20 …". `null` for
 * anything that isn't one plain number: fewer than 7 or more than 15 digits,
 * letters, an extension ("#12") or two numbers ("… / …"), where a link
 * would dial the wrong thing.
 */
export function telHref(phone: string | null | undefined): string | null {
  const value = (phone ?? '').trim();
  if (!/^\+?[\d\s().-]+$/.test(value)) return null;
  const plus = value.startsWith('+');
  const digits = (plus ? value.replace('(0)', '') : value).replace(/\D/g, '');
  if (digits.length < 7 || digits.length > 15) return null;
  return `tel:${plus ? '+' : ''}${digits}`;
}
