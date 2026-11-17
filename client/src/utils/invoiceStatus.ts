// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/** Statuses an invoice can't leave: the server refuses changes with STATUS_LOCKED. */
export const FINAL_STATUSES: readonly string[] = ['paid', 'void'];

/**
 * Whether saving an invoice as `target` should ask first: it would make the
 * invoice paid or void, and it isn't that already (a locked invoice only
 * saves its internal notes, so it never asks).
 */
export function needsFinalConfirm(target: string, savedStatus: string, locked: boolean): boolean {
  return !locked && FINAL_STATUSES.includes(target) && savedStatus !== target;
}
