// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * `mailto:` link for an address a client typed, or `null` when it isn't one
 * (the text is then shown as plain text, not as a dead link).
 */
export function mailtoHref(email: string | null | undefined): string | null {
  const value = (email ?? '').trim();
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value) ? `mailto:${value}` : null;
}

/**
 * `tel:` link for a phone number as typed ("(503) 555-0142" → "tel:5035550142").
 * Keeps a leading "+"; `null` for anything with fewer than 7 digits or with
 * letters in it.
 */
export function telHref(phone: string | null | undefined): string | null {
  const value = (phone ?? '').trim();
  if (!value || /[a-z]/i.test(value)) return null;
  const digits = value.replace(/\D/g, '');
  if (digits.length < 7) return null;
  return `tel:${value.startsWith('+') ? '+' : ''}${digits}`;
}
