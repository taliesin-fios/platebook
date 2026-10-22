// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { Link } from 'react-router';
import { APP_AUTHOR, APP_NAME, APP_REPO_URL, APP_TAGLINE } from '../constants';
import { useSignupOpen } from '../hooks/useSignupOpen';

const FEATURES = [
  { icon: '👥', title: 'Client profiles', body: 'Households, allergies, dietary styles, kitchen equipment and parking notes, all in one place.' },
  { icon: '📝', title: 'Online booking form', body: 'Share your own booking link. New inquiries land in your inbox with every preference filled in.' },
  { icon: '✨', title: 'AI recipe import', body: 'Paste a link or any recipe text and it’s parsed into ingredients, times and store sections.' },
  { icon: '🗓️', title: 'Session planning', body: 'Build menus that fit the time you have, with allergen warnings and AI menu suggestions.' },
  { icon: '🛒', title: 'Smart grocery lists', body: 'Ingredients merged and scaled across dishes, sorted by store aisle, ready to check off.' },
  { icon: '📄', title: 'Meal-plan PDFs', body: 'Branded menus with reheating instructions, emailed straight to your client.' },
  { icon: '💳', title: 'Invoices & deposits', body: 'Invoice from a session in one tap, track deposits and see what’s outstanding.' },
  { icon: '📈', title: 'Profit tracking', body: 'Revenue, grocery costs and margin per session and per client, plus CSV export for tax time.' },
];

const STEPS = [
  ['Create your account', 'Takes under a minute.'],
  ['Set up your business', 'Name, services, pricing and booking policy.'],
  ['Share your booking link', 'Clients tell you everything you need up front.'],
];

export default function Welcome() {
  const signupOpen = useSignupOpen();
  return (
    <div className="min-h-dvh bg-ivory font-sans text-olive">
      {/* Top bar */}
      <header className="max-w-5xl mx-auto flex items-center justify-between px-4 py-5">
        <div className="font-display text-xl font-bold text-green-dark">🍳 {APP_NAME}</div>
        <Link to="/login" className="text-sm font-semibold text-green-dark hover:underline">Sign in</Link>
      </header>

      {/* Hero */}
      <section className="max-w-3xl mx-auto px-4 pt-8 pb-12 text-center">
        <h1 className="font-display text-3xl sm:text-5xl font-bold leading-tight text-olive">
          {APP_TAGLINE}
        </h1>
        <p className="mt-5 text-base sm:text-lg text-gray-600 leading-relaxed">
          Whether you cook in clients’ homes, deliver weekly meal prep or run dinner parties,
          {` ${APP_NAME}`} keeps your clients, recipes, menus, grocery runs and invoices in one place,
          so you can spend your time cooking.
        </p>
        <div className="mt-8 flex flex-col sm:flex-row gap-3 justify-center">
          {signupOpen && (
            <Link to="/signup" className="px-7 py-3.5 rounded-xl bg-herb text-white font-semibold shadow-card hover:shadow-card-hover transition-shadow">
              Create your account
            </Link>
          )}
          <Link to="/login" className="px-7 py-3.5 rounded-xl bg-white text-green-dark font-semibold border border-gray-200">
            I already have an account
          </Link>
        </div>
      </section>

      {/* How it works */}
      <section className="max-w-4xl mx-auto px-4 pb-12">
        <ol className="grid gap-3 sm:grid-cols-3">
          {STEPS.map(([title, body], i) => (
            <li key={title} className="card flex gap-3 items-start">
              <span className="shrink-0 w-8 h-8 rounded-full bg-sage text-green-dark font-bold flex items-center justify-center">{i + 1}</span>
              <div>
                <div className="font-semibold text-sm">{title}</div>
                <div className="text-xs text-gray-500 mt-0.5">{body}</div>
              </div>
            </li>
          ))}
        </ol>
      </section>

      {/* Features */}
      <section className="max-w-5xl mx-auto px-4 pb-16">
        <h2 className="font-display text-2xl font-semibold text-center mb-8 text-green-dark">Everything a working chef needs</h2>
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {FEATURES.map(f => (
            <div key={f.title} className="card">
              <div className="text-2xl mb-2" aria-hidden="true">{f.icon}</div>
              <div className="font-semibold text-sm mb-1">{f.title}</div>
              <p className="text-xs text-gray-500 leading-relaxed">{f.body}</p>
            </div>
          ))}
        </div>
      </section>

      {/* Closing CTA */}
      <section className="bg-green-dark text-white">
        <div className="max-w-3xl mx-auto px-4 py-12 text-center">
          <h2 className="font-display text-2xl sm:text-3xl font-semibold">Run your chef business from one app</h2>
          <p className="mt-3 text-sage/90 text-sm">Set up in minutes. Your clients and data stay private to your account.</p>
          <Link to={signupOpen ? '/signup' : '/login'} className="inline-block mt-6 px-7 py-3.5 rounded-xl bg-white text-green-dark font-semibold">
            {signupOpen ? 'Get started' : 'Sign in'}
          </Link>
        </div>
      </section>

      <footer className="text-center text-xs text-gray-400 py-6">
        {APP_NAME} by <a href={APP_REPO_URL} target="_blank" rel="noopener noreferrer" className="underline decoration-dotted">{APP_AUTHOR}</a>
      </footer>
    </div>
  );
}
