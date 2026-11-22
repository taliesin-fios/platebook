// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * A store section's items in shopping order: what is still to buy first, what
 * is already ticked off after it. Each group keeps the order it came in, so
 * nothing else moves when one item is ticked. Returns a new array.
 */
export function sinkTicked<T extends { is_checked?: unknown }>(items: readonly T[]): T[] {
  return [...items.filter(i => !i.is_checked), ...items.filter(i => Boolean(i.is_checked))];
}
