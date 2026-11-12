// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter, Route, Routes } from 'react-router';
import { ToastProvider } from '../components/Toast';
import { ALLERGENS_LIST } from '../constants';
import ClientForm from './ClientForm';

/** The form as it first appears, before the chef has tapped anything. */
function firstRender(path: string): string {
  return renderToStaticMarkup(
    <MemoryRouter initialEntries={[path]}>
      <ToastProvider>
        <Routes>
          <Route path="/clients/new" element={<ClientForm />} />
          <Route path="/clients/:id/edit" element={<ClientForm />} />
        </Routes>
      </ToastProvider>
    </MemoryRouter>,
  );
}

describe('ClientForm', () => {
  it.each(['/clients/new', '/clients/7/edit'])('shows the allergen choices without opening a section (%s)', path => {
    const html = firstRender(path);
    expect(html).toContain('Allergens');
    for (const allergen of ALLERGENS_LIST) {
      expect(html).toContain(allergen);
    }
  });

  it('keeps the kitchen and service sections closed until asked for', () => {
    const html = firstRender('/clients/new');
    expect(html).toContain('C — Kitchen Info');
    expect(html).not.toContain('Parking');
    expect(html).not.toContain('Grocery');
  });
});
