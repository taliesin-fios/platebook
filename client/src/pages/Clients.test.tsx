// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';
import { ToastProvider } from '../components/Toast';
import Clients from './Clients';

describe('Clients', () => {
  it('has one add button (the floating one), not a second one in the header', () => {
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <ToastProvider>
          <Clients />
        </ToastProvider>
      </MemoryRouter>,
    );
    const plusButtons = html.match(/<button[^>]*>\+<\/button>/g) ?? [];
    expect(plusButtons).toHaveLength(1);
    expect(plusButtons[0]).toContain('class="fab"');
  });
});
