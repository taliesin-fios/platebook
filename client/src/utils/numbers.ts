// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/** A numeric form value: a number from the API, or the string an `<input>` holds while editing. */
export type NumInput = number | string;

/**
 * `parseFloat` for values that may already be numbers (or null).
 * `parseFloat` converts its argument with `String()` first, so this returns
 * exactly what `parseFloat(v)` would; it only exists to satisfy the types.
 */
export const parseNum = (v: unknown): number => parseFloat(String(v));
