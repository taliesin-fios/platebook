// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState } from 'react';
import { apiCall, errorMessage } from '../hooks/useApi';
import type { Chef } from '../types';

/**
 * "Try the demo": signs in to the server's shared demo account
 * (`POST /api/auth/demo`, only offered when `DEMO_MODE` is on) and opens
 * the dashboard.
 */
export default function TryDemoButton({ className, children = 'Try the live demo' }: {
  className: string;
  children?: string;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  const start = async () => {
    setError('');
    setBusy(true);
    try {
      await apiCall<Chef>('/api/auth/demo', 'POST', {});
      window.location.href = '/';
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  };

  return (
    <>
      <button type="button" onClick={start} disabled={busy} className={`${className} disabled:opacity-60`}>
        {busy ? 'Opening the demo…' : children}
      </button>
      {error && <p role="alert" className="text-sm text-red-700 mt-2">{error}</p>}
    </>
  );
}
