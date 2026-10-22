// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Server-rendered PDFs (meal plan, grocery list, planner, invoice): the API
 * renders them with Typst (GET …/pdf, see server-rs/src/pdf).
 */

/** Fetches a server PDF; throws with the API's error message on failure. */
export async function fetchServerPdf(url: string): Promise<Blob> {
  const res = await fetch(url);
  if (!res.ok) {
    const body: { error?: string } = await res.json().catch(() => ({}));
    throw new Error(body.error || `PDF request failed (${res.status})`);
  }
  const blob = await res.blob();
  if (blob.type !== 'application/pdf') throw new Error('The server did not return a PDF');
  return blob;
}

/** Saves a blob as a file download. */
export function saveBlob(blob: Blob, filename: string) {
  const href = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = href;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  // Let the browser start the download before the URL goes away.
  setTimeout(() => URL.revokeObjectURL(href), 10_000);
}

/** Downloads the server PDF at `url` as `filename`. */
export async function downloadServerPdf(url: string, filename: string) {
  saveBlob(await fetchServerPdf(url), filename);
}

const slug = (name: string | null | undefined) =>
  name?.replace(/\s+/g, '-').toLowerCase() || 'client';

/** Download filenames. */
export const pdfFilename = {
  mealPlan: (s: { client_name?: string | null; session_date?: string | null }) =>
    `meal-plan-${slug(s.client_name)}-${s.session_date || 'session'}.pdf`,
  grocery: (s: { client_name?: string | null; session_date?: string | null }) =>
    `grocery-${slug(s.client_name)}-${s.session_date || 'session'}.pdf`,
  planner: (date: string) => `planner-${date}.pdf`,
  invoice: (invoiceNumber: string) => `invoice-${invoiceNumber}.pdf`,
};
