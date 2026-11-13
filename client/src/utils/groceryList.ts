// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * What to tell the chef before the grocery list is rebuilt, which clears
 * every tick. `null` when nothing is ticked: there is nothing to lose, so
 * the rebuild can go ahead without asking.
 */
export function regenerateWarning(items: ReadonlyArray<{ is_checked?: unknown }>): string | null {
  const ticked = items.filter(i => Boolean(i.is_checked)).length;
  if (ticked === 0) return null;
  return ticked === 1
    ? 'The 1 item you have ticked off will be unticked.'
    : `The ${ticked} items you have ticked off will be unticked.`;
}
