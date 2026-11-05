// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useAuth } from '../App';
import { fmtTimeUntilUtc } from '../utils/dateUtils';

/**
 * A strip above every page while signed in to the shared demo account: what
 * the demo is, when it resets, and a way out.
 */
export default function DemoBanner() {
  const { chef, logout } = useAuth();
  if (!chef?.demo) return null;
  const { reset_hours, next_reset_at } = chef.demo;
  const every = reset_hours === 24 ? 'every day' : `every ${reset_hours} hours`;
  const when = fmtTimeUntilUtc(next_reset_at, every);

  return (
    <div role="status" className="demo-banner flex flex-wrap items-center gap-x-3 gap-y-1 bg-green-dark text-white px-4 py-2 text-xs sm:text-sm">
      <span className="flex-1 min-w-0">
        <strong className="font-semibold">You’re exploring a demo.</strong>{' '}
        Change anything you like: it all resets {when}, and no emails are sent.
      </span>
      <button type="button" onClick={logout}
        className="shrink-0 rounded-lg bg-white/15 px-3 py-1 font-semibold hover:bg-white/25"
        style={{ minHeight: 'auto' }}>
        Leave demo
      </button>
    </div>
  );
}
