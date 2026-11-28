// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useRef } from 'react';

/**
 * Runs an async action at most once at a time: calls made while it is still
 * running are ignored. A disabled button isn't enough on its own, because
 * several quick clicks land before React re-renders it disabled (a rehearsal
 * sent one client five copies of the same meal plan that way).
 */
export function singleFlight() {
  let busy = false;
  return async <T,>(action: () => Promise<T>): Promise<T | undefined> => {
    if (busy) return undefined;
    busy = true;
    try {
      return await action();
    } finally {
      busy = false;
    }
  };
}

/** {@link singleFlight}, kept for the component's lifetime. */
export function useSingleFlight() {
  const ref = useRef<ReturnType<typeof singleFlight> | null>(null);
  if (!ref.current) ref.current = singleFlight();
  return ref.current;
}
