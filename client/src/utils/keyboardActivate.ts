// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Keyboard access for cards and rows that are clicked as a whole. They hold
 * other buttons (swipe actions, badges), so they can't be <button>s; these
 * props put them in the tab order and make Enter and Space do what a click
 * does. Spread them next to the element's own `onClick`.
 */

/** The part of a key event this needs (a React or DOM keyboard event fits). */
interface ActivationKeyEvent {
  key: string;
  target: unknown;
  currentTarget: { click: () => void };
  preventDefault: () => void;
}

/**
 * Enter or Space on the element itself clicks it. Keys pressed on something
 * inside it (a nested button, a field) are left to that control.
 */
export function activateOnKey(e: ActivationKeyEvent): void {
  if (e.target !== e.currentTarget) return;
  if (e.key !== 'Enter' && e.key !== ' ') return;
  e.preventDefault(); // Space would otherwise scroll the page
  e.currentTarget.click();
}

/** A card that opens something when pressed. */
export const pressable = { role: 'button', tabIndex: 0, onKeyDown: activateOnKey } as const;

/** A table row that opens something: no role, so it stays a row to screen readers. */
export const pressableRow = { tabIndex: 0, onKeyDown: activateOnKey } as const;

/** A row that is ticked on and off (a grocery item, a recipe in a menu). */
export function toggleable(checked: unknown) {
  return { role: 'checkbox', 'aria-checked': Boolean(checked), tabIndex: 0, onKeyDown: activateOnKey } as const;
}
