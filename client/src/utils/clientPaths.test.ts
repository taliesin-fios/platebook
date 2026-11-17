// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, it, expect } from 'vitest';
import { clientPathAfterSave } from './clientPaths';

describe('clientPathAfterSave', () => {
  it("opens the saved client's page", () => {
    expect(clientPathAfterSave({ id: 18 })).toBe('/clients/18');
  });

  it('falls back to the client list without a usable id', () => {
    expect(clientPathAfterSave(null)).toBe('/clients');
    expect(clientPathAfterSave(undefined)).toBe('/clients');
    expect(clientPathAfterSave({})).toBe('/clients');
    expect(clientPathAfterSave({ id: 0 })).toBe('/clients');
    expect(clientPathAfterSave({ id: '18' })).toBe('/clients');
    expect(clientPathAfterSave({ id: 1.5 })).toBe('/clients');
  });
});
