// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { mtdTrend } from './trend';

describe('mtdTrend', () => {
  it('shows a rise as a rounded percentage', () => {
    expect(mtdTrend(1120, 1000)).toEqual({ text: '▲ 12% vs last month', direction: 'up' });
  });

  it('shows a fall without a minus sign', () => {
    expect(mtdTrend(500, 2000)).toEqual({ text: '▼ 75% vs last month', direction: 'down' });
  });

  it('treats a change that rounds to 0% as flat', () => {
    expect(mtdTrend(1004, 1000)).toEqual({ text: '→ same as last month', direction: 'flat' });
  });

  it('shows nothing without last month to compare with', () => {
    expect(mtdTrend(800, 0)).toBeNull();
    expect(mtdTrend(0, 0)).toBeNull();
  });

  it('shows a full drop to $0', () => {
    expect(mtdTrend(0, 400)).toEqual({ text: '▼ 100% vs last month', direction: 'down' });
  });
});
