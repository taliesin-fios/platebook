// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState } from 'react';
import { useNavigate } from 'react-router';
import PageHeader from '../components/PageHeader';
import Tour from '../components/Tour';

interface Workflow {
  title: string;
  intro: string;
  steps: string[];
  action: { label: string; path: string };
}

// The everyday jobs, in the order a week usually runs. Kept short on purpose:
// the full reference is MANUAL.md in the repository.
export const WORKFLOWS: Workflow[] = [
  {
    title: 'Add a client',
    intro: 'Do this once per household, before their first cook day.',
    steps: [
      'Open Clients and choose Add Client.',
      'Enter their name and how many you’re feeding.',
      'Under Dietary Profile, tick their allergies and note what they like and dislike.',
      'Add kitchen and parking notes if you have them, then Create Client.',
    ],
    action: { label: 'Add a client', path: '/clients/new' },
  },
  {
    title: 'Plan a session',
    intro: 'A session is one cook day for one client.',
    steps: [
      'Start a new session and pick the client.',
      'Set the date and time. Make it recurring if they book you regularly.',
      'Choose the dishes. The bar at the bottom shows whether the menu fits your time, and dishes that clash with an allergy are flagged.',
      'Enter your fee and expected grocery cost, review, then Confirm Session.',
    ],
    action: { label: 'Plan a session', path: '/sessions/new' },
  },
  {
    title: 'Shop from the grocery list',
    intro: 'Every session writes its own shopping list.',
    steps: [
      'Open the session and go to its Grocery tab.',
      'Everything is added up across the dishes and grouped by store section.',
      'Tap each item as it goes in the cart. Your ticks are saved.',
      'Changed the menu? Regenerate the list before you shop, because that clears the ticks.',
    ],
    action: { label: 'Open sessions', path: '/sessions' },
  },
  {
    title: 'Send an invoice',
    intro: 'Bill for the work once the cooking is done.',
    steps: [
      'Mark the session as completed.',
      'Open Invoices and create an invoice for that client.',
      'Check the lines and the total, then email it to the client.',
      'When the money arrives, mark the invoice paid. The dashboard shows what’s still owed.',
    ],
    action: { label: 'Open invoices', path: '/invoices' },
  },
  {
    title: 'Handle a booking request',
    intro: 'New clients can ask for you through your booking link.',
    steps: [
      'Copy your booking link from the dashboard and share it.',
      'Requests arrive in your booking inbox. Open one to read their answers.',
      'Record the deposit and add any private notes.',
      'When you take them on, convert the request to a client so their details carry over.',
    ],
    action: { label: 'Open booking inbox', path: '/intake' },
  },
];

export default function Guide() {
  const navigate = useNavigate();
  const [tourOpen, setTourOpen] = useState(false);

  return (
    <div className="page">
      <PageHeader title="How it works" subtitle="The everyday jobs, step by step" back="/" />

      <div className="px-4 pt-4 space-y-4 max-w-2xl">
        <div className="card flex flex-wrap items-center gap-3">
          <p className="flex-1 min-w-[12rem] text-sm text-gray-600">
            New here, or want a reminder of what each screen is for?
          </p>
          <button
            type="button"
            onClick={() => setTourOpen(true)}
            className="px-4 py-2.5 rounded-xl bg-navy text-white font-semibold text-sm"
          >
            Take the tour
          </button>
        </div>

        {WORKFLOWS.map((w, i) => (
          <section key={w.title} className="card" aria-labelledby={`guide-${i}`}>
            <h2 id={`guide-${i}`} className="section-header" style={{ marginBottom: 4 }}>
              {i + 1}. {w.title}
            </h2>
            <p className="text-sm text-gray-500 mb-3">{w.intro}</p>
            <ol className="list-decimal pl-5 space-y-1.5 text-sm text-olive mb-4">
              {w.steps.map(s => <li key={s}>{s}</li>)}
            </ol>
            <button
              type="button"
              onClick={() => navigate(w.action.path)}
              className="px-4 py-2.5 rounded-xl bg-warm-white border border-gray-200 text-navy font-semibold text-sm"
            >
              {w.action.label} →
            </button>
          </section>
        ))}
      </div>

      <Tour open={tourOpen} onClose={() => setTourOpen(false)} />
    </div>
  );
}
