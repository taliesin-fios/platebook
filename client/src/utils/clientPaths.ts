// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Where the client form goes after a save: the saved client's own page (which
 * has "+ Session"), or the client list when the response carries no usable id.
 */
export function clientPathAfterSave(saved: { id?: unknown } | null | undefined): string {
  const id = saved?.id;
  return typeof id === 'number' && Number.isInteger(id) && id > 0 ? `/clients/${id}` : '/clients';
}
