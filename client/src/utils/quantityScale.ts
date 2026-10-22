// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Client-side quantity string parser, formatter, and scaler.
 * Mirrors the logic in server-rs/src/routes/planner.rs so view and plan are consistent.
 */

/** Result of parsing a quantity string. `value` is null for "to taste"-style amounts. */
export interface ParsedQty {
  value: number | null;
  unit: string;
  rangeHi?: number;
  isRange?: boolean;
}

const FRAC_MAP: Array<[number, string]> = [
  [0.875, '7/8'], [0.75, '3/4'], [0.625, '5/8'],
  [0.5, '1/2'], [0.375, '3/8'], [0.25, '1/4'], [0.125, '1/8'],
];

/** Parse "1 1/2 cups" → { value: 1.5, unit: "cups" } or null if unparseable */
export function parseQtyStr(str: string | null | undefined): ParsedQty | null {
  if (!str || str === '—') return null;
  const s = str.trim();

  if (/^(to taste|as needed|optional|a few|handful|pinch)$/i.test(s)) {
    return { value: null, unit: s };
  }

  // "2-3 cups" or "1-2 cloves" — range: use lower bound, keep full string as unit suffix
  const range = s.match(/^(\d+\.?\d*)\s*-\s*(\d+\.?\d*)\s*(.*)$/);
  if (range) {
    const lo = parseFloat(range[1]);
    const hi = parseFloat(range[2]);
    return { value: lo, unit: range[3].trim(), rangeHi: hi, isRange: true };
  }

  // "1 1/2 cups"
  const mixed = s.match(/^(\d+)\s+(\d+)\/(\d+)\s*(.*)$/);
  if (mixed) {
    return {
      value: parseInt(mixed[1]) + parseInt(mixed[2]) / parseInt(mixed[3]),
      unit: mixed[4].trim(),
    };
  }

  // "1/2 cup" — guard zero denominator (C10)
  const frac = s.match(/^(\d+)\/(\d+)\s*(.*)$/);
  if (frac) {
    const denom = parseInt(frac[2]);
    if (denom === 0) return null;
    return { value: parseInt(frac[1]) / denom, unit: frac[3].trim() };
  }

  // "2.5 cups" or "3 cloves" or "3"
  const num = s.match(/^(\d+\.?\d*)\s*(.*)$/);
  if (num) {
    return { value: parseFloat(num[1]), unit: num[2].trim() };
  }

  return null;
}

/** Format a numeric value + unit back to a human-readable quantity string */
export function formatQtyNum(value: number | null | undefined, unit: string | null | undefined): string {
  if (value === null || value === undefined) return unit || '';
  if (!unit && value === 0) return '0';

  const rounded = Math.round(value * 8) / 8;
  if (rounded === 0) return unit ? `0 ${unit}` : '0';

  const whole = Math.floor(rounded);
  const fracPart = Math.round((rounded - whole) * 8) / 8;

  // If fraction part rounds to 1, carry over
  if (Math.abs(fracPart - 1) < 0.01) return formatQtyNum(whole + 1, unit);

  let fracStr = '';
  for (const [v, s] of FRAC_MAP) {
    if (Math.abs(fracPart - v) < 0.01) { fracStr = s; break; }
  }

  let numStr;
  if (whole > 0 && fracStr) numStr = `${whole} ${fracStr}`;
  else if (whole > 0)        numStr = String(whole);
  else                       numStr = fracStr || rounded.toFixed(2).replace(/\.?0+$/, '');

  return unit ? `${numStr} ${unit}` : numStr;
}

/**
 * Scale a quantity string from one serving count to another.
 * e.g. scaleQty("1 1/2 cups", 4, 6) → "2 1/4 cups"
 * Returns the original string for unparseable or "to taste" values.
 */
export function scaleQty(
  qtyStr: string | null | undefined,
  fromServings: number | null | undefined,
  toServings: number,
): string {
  if (!qtyStr || qtyStr === '—') return qtyStr || '';
  if (!fromServings || fromServings === toServings) return qtyStr;

  const parsed = parseQtyStr(qtyStr);
  if (!parsed || parsed.value === null) return qtyStr; // "to taste" etc.

  const factor = toServings / fromServings;

  // Range quantities: scale both bounds and render as "lo–hi unit"
  if (parsed.isRange && parsed.rangeHi !== undefined) {
    const scaledLo = parsed.value * factor;
    const scaledHi = parsed.rangeHi * factor;
    const loStr = formatQtyNum(scaledLo, '').trim();
    const hiStr = formatQtyNum(scaledHi, parsed.unit).trim();
    return `${loStr}–${hiStr}`;
  }

  const scaled = parsed.value * factor;
  return formatQtyNum(scaled, parsed.unit);
}

/** Auto-derive qty_6_serving from qty_4_serving (× 1.5, rounded to clean fraction) */
export function deriveQty6(qty4Str: string | null | undefined): string {
  if (!qty4Str) return '';
  const parsed = parseQtyStr(qty4Str);
  if (!parsed || parsed.value === null) return qty4Str;
  return formatQtyNum(parsed.value * 1.5, parsed.unit);
}
