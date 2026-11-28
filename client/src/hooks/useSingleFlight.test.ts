// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, expect, it } from 'vitest';
import { singleFlight } from './useSingleFlight';

describe('singleFlight', () => {
  it('ignores calls while the first is running, then allows the next', async () => {
    const guard = singleFlight();
    let runs = 0;
    let release!: () => void;
    const slow = () => new Promise<void>((r) => { runs += 1; release = r; });
    const first = guard(slow);
    await guard(slow); // ignored: returns at once
    await guard(slow);
    expect(runs).toBe(1);
    release();
    await first;
    const again = guard(slow);
    release();
    await again;
    expect(runs).toBe(2);
  });

  it('frees itself after a failure', async () => {
    const guard = singleFlight();
    await expect(guard(async () => { throw new Error('boom'); })).rejects.toThrow('boom');
    expect(await guard(async () => 'ok')).toBe('ok');
  });
});
