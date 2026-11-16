// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { activateOnKey, pressable, pressableRow, toggleable } from './keyboardActivate';

/** A key event on `el`, or on something inside it when `inner` is given. */
function press(key: string, inner?: object, repeat = false) {
  const calls = { click: 0, prevented: 0 };
  const el = { click: () => { calls.click += 1; } };
  activateOnKey({ key, repeat, target: inner ?? el, currentTarget: el, preventDefault: () => { calls.prevented += 1; } });
  return calls;
}

describe('activateOnKey', () => {
  it('clicks the element on Enter and on Space, and stops Space from scrolling', () => {
    expect(press('Enter')).toEqual({ click: 1, prevented: 1 });
    expect(press(' ')).toEqual({ click: 1, prevented: 1 });
  });

  it('acts once while a key is held, still without scrolling', () => {
    expect(press(' ', undefined, true)).toEqual({ click: 0, prevented: 1 });
    expect(press('Enter', undefined, true)).toEqual({ click: 0, prevented: 1 });
  });

  it('ignores other keys', () => {
    expect(press('Tab')).toEqual({ click: 0, prevented: 0 });
    expect(press('a')).toEqual({ click: 0, prevented: 0 });
    expect(press('Escape')).toEqual({ click: 0, prevented: 0 });
  });

  it('leaves keys pressed on a control inside the element to that control', () => {
    expect(press('Enter', {})).toEqual({ click: 0, prevented: 0 });
    expect(press(' ', {})).toEqual({ click: 0, prevented: 0 });
  });
});

describe('the props', () => {
  it('put a card in the tab order as a button', () => {
    expect(pressable.role).toBe('button');
    expect(pressable.tabIndex).toBe(0);
    expect(pressable.onKeyDown).toBe(activateOnKey);
  });

  it('keep a table row a row', () => {
    expect('role' in pressableRow).toBe(false);
    expect(pressableRow.tabIndex).toBe(0);
  });

  it('report a ticked row as a checked checkbox', () => {
    expect(toggleable(1)).toMatchObject({ role: 'checkbox', 'aria-checked': true, tabIndex: 0 });
    expect(toggleable(0)['aria-checked']).toBe(false);
    expect(toggleable(undefined)['aria-checked']).toBe(false);
  });
});
