// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';
import Tour from './Tour';
import Guide, { WORKFLOWS } from '../pages/Guide';
import { TOUR_STEPS } from '../utils/tour';

const noop = () => {};
const buttons = (html: string) => [...html.matchAll(/<button[^>]*>(.*?)<\/button>/g)].map(m => m[1].replace(/<[^>]+>/g, '').trim());

describe('Tour', () => {
  it('renders nothing while closed', () => {
    expect(renderToStaticMarkup(<Tour open={false} onClose={noop} />)).toBe('');
  });

  it('opens on the first step as a labelled dialog with Skip and Next', () => {
    const html = renderToStaticMarkup(<Tour open onClose={noop} onOpenGuide={noop} />);
    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-modal="true"');
    expect(html).toContain('aria-labelledby="tour-title"');
    expect(html).toContain(`Step 1 of ${TOUR_STEPS.length}`);
    expect(html).toContain('Dashboard: your week at a glance');
    expect(buttons(html)).toEqual(['Skip tour', 'Next']);
  });

  it('offers Back in the middle', () => {
    const html = renderToStaticMarkup(<Tour open onClose={noop} onOpenGuide={noop} initialStep={2} />);
    expect(html).toContain('Step 3 of 5');
    expect(buttons(html)).toEqual(['Skip tour', 'Back', 'Next']);
  });

  it('ends with the guide and Done', () => {
    const last = TOUR_STEPS.length - 1;
    const html = renderToStaticMarkup(<Tour open onClose={noop} onOpenGuide={noop} initialStep={last} />);
    expect(buttons(html)).toEqual(['Skip tour', 'Back', 'Open the guide', 'Done']);
    // On the guide page itself there is nowhere else to send them.
    const onGuide = renderToStaticMarkup(<Tour open onClose={noop} initialStep={last} />);
    expect(buttons(onGuide)).toEqual(['Skip tour', 'Back', 'Done']);
  });

  it('gives every control a text label', () => {
    for (let i = 0; i < TOUR_STEPS.length; i++) {
      const html = renderToStaticMarkup(<Tour open onClose={noop} onOpenGuide={noop} initialStep={i} />);
      for (const label of buttons(html)) expect(label).not.toBe('');
    }
  });
});

describe('Guide', () => {
  const html = renderToStaticMarkup(<MemoryRouter><Guide /></MemoryRouter>);

  it('covers the five everyday jobs in order', () => {
    expect(WORKFLOWS.map(w => w.title)).toEqual([
      'Add a client', 'Plan a session', 'Shop from the grocery list', 'Send an invoice', 'Handle a booking request',
    ]);
    for (const [i, w] of WORKFLOWS.entries()) expect(html).toContain(`${i + 1}. ${w.title}`);
  });

  it('can replay the tour, which starts closed', () => {
    expect(buttons(html)).toContain('Take the tour');
    expect(html).not.toContain('role="dialog"');
  });

  it('sends each job to a real screen', () => {
    expect(WORKFLOWS.map(w => w.action.path)).toEqual(['/clients/new', '/sessions/new', '/sessions', '/invoices', '/intake']);
  });
});
