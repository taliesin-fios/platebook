// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, expect, it, vi } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';
import Welcome from './Welcome';

const status = vi.hoisted(() => ({ open: false, demo: false, loading: false }));
vi.mock('../hooks/useSignupOpen', () => ({ useSignupStatus: () => status }));
vi.mock('../components/TryDemoButton', () => ({ default: () => <button>Try the live demo</button> }));

function markup() { return renderToStaticMarkup(<MemoryRouter><Welcome /></MemoryRouter>); }

describe('Welcome marketing and next steps', () => {
  it('shows demo only when enabled and clearly routes to self-hosting and feedback', () => {
    status.open = false; status.demo = true;
    const html = markup();
    expect(html).toContain('Try the live demo');
    expect(html).toContain('shared demo uses sample data, may be reset at any time, and is not for private information.');
    expect(html).toContain('Self-host Platebook');
    expect(html).toMatch(/<a[^>]*href="https:\/\/github.com\/taliesin-fios\/platebook"[^>]*>Source code<\/a>/);
    expect(html).toMatch(/<a[^>]*href="https:\/\/github.com\/taliesin-fios\/platebook\/blob\/main\/docs\/SELF_HOSTING.md"[^>]*>(?:Self-hosting guide|Self-host Platebook)<\/a>/);
    expect(html).toMatch(/<a[^>]*href="https:\/\/github.com\/taliesin-fios\/platebook\/discussions"[^>]*>Give feedback<\/a>/);
    expect(html).not.toContain('href="/signup"');
  });
  it('offers account creation on this server only when registration is open', () => {
    status.open = true; status.demo = false;
    const html = markup();
    expect(html).toContain('Create an account on this server');
    expect(html).toContain('href="/signup"');
    expect(html).toContain('Sign in');
    expect(html).not.toContain('Try the live demo');
  });
  it('offers the real signup route when registration is open with or without demo', () => {
    for (const demo of [false, true]) {
      status.open = true; status.demo = demo;
      const html = markup();
      expect(html.match(/href="\/signup"/g)).toHaveLength(2);
      expect(html).toContain('Create an account on this server');
    }
  });
  it('does not send closed-registration visitors to login as a signup action', () => {
    status.open = false; status.demo = false;
    const html = markup();
    expect(html).not.toContain('href="/signup"');
    expect(html).not.toContain('Get started');
    expect(html).toContain('Source code');
    expect(html).toContain('PolyForm Shield 1.0.0');
  });
});
