// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, expect, it } from 'vitest';

import html from '../index.html?raw';

const metaContent = (attribute: 'name' | 'property', value: string): string | undefined => {
  const escapedValue = value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = html.match(new RegExp(`<meta\\s+${attribute}="${escapedValue}"\\s+content="([^"]*)"\\s*/?>`, 'i'));
  return match?.[1]?.replaceAll('&amp;', '&');
};

describe('static page metadata', () => {
  it('describes Platebook accurately for search and social previews', () => {
    const description = metaContent('name', 'description');
    expect(description).toBeDefined();
    expect(description).toContain('Platebook');
    expect(description).toContain('self-hostable');
    expect(description).toContain('source-available');
    expect(description).toContain('home and personal chefs');
    expect(description).toContain('public demo');
    expect(description).not.toMatch(/hosted signup service/i);
  });

  it('provides Open Graph and Twitter metadata in the initial HTML', () => {
    expect(html).toMatch(/<meta\s+property="og:type"\s+content="website"\s*\/>/i);
    expect(metaContent('property', 'og:site_name')).toBe('Platebook');
    expect(metaContent('property', 'og:title')).toBe('Platebook — for home & personal chefs');
    expect(metaContent('name', 'twitter:card')).toBe('summary');
    expect(metaContent('name', 'twitter:title')).toBe('Platebook — for home & personal chefs');

    const description = metaContent('name', 'description');
    expect(metaContent('property', 'og:description')).toBe(description);
    expect(metaContent('name', 'twitter:description')).toBe(description);
  });
});
