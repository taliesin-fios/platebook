// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { afterEach, describe, expect, it, vi } from 'vitest';
import { downloadServerPdf, fetchServerPdf, pdfFilename } from './serverPdf';

const pdfBytes = new TextEncoder().encode('%PDF-1.7 test');

function stubFetch(res: Response) {
  const fn = vi.fn(async () => res);
  vi.stubGlobal('fetch', fn);
  return fn;
}

afterEach(() => vi.unstubAllGlobals());

describe('fetchServerPdf', () => {
  it('returns the PDF the server rendered', async () => {
    const fetch = stubFetch(new Response(pdfBytes, { headers: { 'Content-Type': 'application/pdf' } }));
    const blob = await fetchServerPdf('/api/sessions/1/pdf');
    expect(fetch).toHaveBeenCalledWith('/api/sessions/1/pdf');
    expect(blob.type).toBe('application/pdf');
    expect(new Uint8Array(await blob.arrayBuffer())).toEqual(pdfBytes);
  });

  it('throws the server error (e.g. rate limited)', async () => {
    stubFetch(new Response(JSON.stringify({ error: 'Too many PDF downloads' }), { status: 429 }));
    await expect(fetchServerPdf('/api/invoices/1/pdf')).rejects.toThrow('Too many PDF downloads');
  });

  it('throws on a 404 without a JSON body', async () => {
    stubFetch(new Response('', { status: 404 }));
    await expect(fetchServerPdf('/api/sessions/1/pdf')).rejects.toThrow('PDF request failed (404)');
  });

  it('throws when the answer is not a PDF', async () => {
    stubFetch(new Response('<!doctype html>', { headers: { 'Content-Type': 'text/html' } }));
    await expect(fetchServerPdf('/api/sessions/1/pdf')).rejects.toThrow('did not return a PDF');
  });
});

describe('downloads and email attachments', () => {
  it('downloadServerPdf saves the PDF under the given name', async () => {
    stubFetch(new Response(pdfBytes, { headers: { 'Content-Type': 'application/pdf' } }));
    // No DOM in these tests: a stand-in link and object URLs.
    const link = { href: '', download: '', click: vi.fn(), remove: vi.fn() };
    vi.stubGlobal('document', { createElement: () => link, body: { appendChild: vi.fn() } });
    vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:pdf');
    await downloadServerPdf('/api/planner/pdf?recipe_ids=1', 'planner.pdf');
    expect(link.click).toHaveBeenCalledOnce();
    expect(link.download).toBe('planner.pdf');
    expect(link.href).toBe('blob:pdf');
    vi.restoreAllMocks();
  });
});

describe('pdfFilename', () => {
  it('names each document', () => {
    const s = { client_name: 'Ann  Lee', session_date: '2026-03-15' };
    expect(pdfFilename.mealPlan(s)).toBe('meal-plan-ann-lee-2026-03-15.pdf');
    expect(pdfFilename.grocery({ client_name: null, session_date: null })).toBe('grocery-client-session.pdf');
    expect(pdfFilename.planner('2026-04-02')).toBe('planner-2026-04-02.pdf');
    expect(pdfFilename.invoice('INV-0001')).toBe('invoice-INV-0001.pdf');
  });
});
