// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, useEffect, createContext, useContext, Suspense, lazy, type ReactNode } from 'react';
import { BrowserRouter, Routes, Route, Navigate, useLocation } from 'react-router';
import { ToastProvider } from './components/Toast';
import BottomNav from './components/BottomNav';
import SideNav from './components/SideNav';
import ErrorBoundary from './components/ErrorBoundary';
import { APP_NAME } from './constants';
import type { AppSettings, Chef, Settings } from './types';
// Login and Welcome are loaded eagerly — they're the first thing an
// unauthenticated visitor sees, and a Suspense flash there would be worse
// than the small extra KB.
import Login from './pages/Login';
import Welcome from './pages/Welcome';

// Every other route is lazy-loaded so its JS (and any heavy library it pulls
// in, e.g. recharts) only downloads when that page is
// actually visited, instead of bloating the initial bundle for every user.
const Dashboard      = lazy(() => import('./pages/Dashboard'));
const Clients        = lazy(() => import('./pages/Clients'));
const ClientDetail   = lazy(() => import('./pages/ClientDetail'));
const ClientForm     = lazy(() => import('./pages/ClientForm'));
const Recipes        = lazy(() => import('./pages/Recipes'));
const RecipeDetail   = lazy(() => import('./pages/RecipeDetail'));
const RecipeImport   = lazy(() => import('./pages/RecipeImport'));
const Sessions       = lazy(() => import('./pages/Sessions'));
const SessionNew     = lazy(() => import('./pages/SessionNew'));
const SessionDetail  = lazy(() => import('./pages/SessionDetail'));
const Templates      = lazy(() => import('./pages/Templates'));
const Costs          = lazy(() => import('./pages/Costs'));
const Settings       = lazy(() => import('./pages/Settings'));
const Planner        = lazy(() => import('./pages/Planner'));
const Calendar       = lazy(() => import('./pages/Calendar'));
const IntakeForm     = lazy(() => import('./pages/IntakeForm'));
const IntakeAdmin    = lazy(() => import('./pages/IntakeAdmin'));
const Invoices       = lazy(() => import('./pages/Invoices'));
const InvoiceBuilder = lazy(() => import('./pages/InvoiceBuilder'));
const InvoicePay     = lazy(() => import('./pages/InvoicePay'));
const Signup         = lazy(() => import('./pages/Signup'));
const Onboarding     = lazy(() => import('./pages/Onboarding'));
const ForgotPassword = lazy(() => import('./pages/PasswordReset').then(m => ({ default: m.ForgotPassword })));
const ResetPassword  = lazy(() => import('./pages/PasswordReset').then(m => ({ default: m.ResetPassword })));

// ── Auth context ──────────────────────────────────────────────────────────────
interface AuthContextValue {
  chef: Chef | null;
  logout: () => void;
}

export const AuthContext = createContext<AuthContextValue>({ chef: null, logout: () => {} });
export const useAuth = () => useContext(AuthContext);

// ── Settings context — business identity available everywhere ─────────────────
export const SettingsContext = createContext<AppSettings>({ refreshSettings: () => {} });
export const useAppSettings = () => useContext(SettingsContext);

// ── Routes that work without being logged in ──────────────────────────────────
// Account pages: signed-in chefs get bounced from these to the dashboard.
const ACCOUNT_PATHS = ['/welcome', '/login', '/signup', '/forgot', '/reset'];
// Client-facing booking pages: always reachable, signed in or not.
const isBookingPath = (pathname: string) =>
  pathname === '/intake/new' ||
  /^\/book\/[a-z0-9-]+$/.test(pathname) ||
  // /intake/:id (numeric) — client edit link with ?token= (token verified server-side)
  /^\/intake\/\d+$/.test(pathname) ||
  // /invoice/:id (numeric) — client invoice link with ?token=
  /^\/invoice\/\d+$/.test(pathname);
const isAccountPath = (pathname: string) => ACCOUNT_PATHS.includes(pathname);
const isPublicPath  = (pathname: string) => isAccountPath(pathname) || isBookingPath(pathname);

// ── Auth gate — checks /api/auth/me on mount ──────────────────────────────────
// Signed out → welcome (from /) or sign-in (deep links).
// Signed in, onboarding unfinished → /onboarding.
function AuthGate({ children }: { children: ReactNode }) {
  const location = useLocation();
  const [authState, setAuthState] = useState<'loading' | 'ok' | 'none'>('loading');
  const [chef, setChef] = useState<Chef | null>(null);
  const path = location.pathname;

  useEffect(() => {
    fetch('/api/auth/me')
      .then(r => {
        if (r.ok) return r.json();
        throw new Error('not authenticated');
      })
      .then((data: Chef) => { setChef(data); setAuthState('ok'); })
      .catch(() => setAuthState('none'));
  }, []);

  const logout = async () => {
    await fetch('/api/auth/logout', { method: 'POST' }).catch(() => {});
    window.location.href = '/login';
  };

  // Client booking pages never depend on the chef's session
  if (isBookingPath(path)) {
    return (
      <AuthContext.Provider value={{ chef, logout }}>
        {children}
      </AuthContext.Provider>
    );
  }

  // Account pages render immediately for visitors; signed-in chefs go home
  if (isAccountPath(path)) {
    if (authState === 'ok') {
      return <Navigate to={chef?.onboarding_completed ? '/' : '/onboarding'} replace />;
    }
    return (
      <AuthContext.Provider value={{ chef, logout }}>
        {children}
      </AuthContext.Provider>
    );
  }

  // Loading — only show spinner on protected pages
  if (authState === 'loading') {
    return (
      <div style={{
        minHeight: '100dvh', display: 'flex', alignItems: 'center',
        justifyContent: 'center', background: '#FBF8F1',
        fontFamily: '"DM Sans", system-ui, sans-serif', color: '#9CA3AF', fontSize: '14px',
      }}>
        <div style={{ textAlign: 'center' }}>
          <div style={{ fontFamily: '"Playfair Display", serif', fontSize: '1.4rem', fontWeight: '700', color: '#3A5C32', marginBottom: '8px' }}>
            🍳
          </div>
          Loading…
        </div>
      </div>
    );
  }

  // Not authenticated — first-time visitors see the welcome page
  if (authState === 'none') {
    return <Navigate to={path === '/' ? '/welcome' : '/login'} replace state={{ from: location }} />;
  }

  // New account — finish setup before using the app
  if (!chef?.onboarding_completed && path !== '/onboarding') {
    return <Navigate to="/onboarding" replace />;
  }

  return (
    <AuthContext.Provider value={{ chef, logout }}>
      {children}
    </AuthContext.Provider>
  );
}

// ── Lightweight fallback shown while a lazy-loaded page's JS downloads ────────
function PageLoadingFallback() {
  return (
    <div style={{
      minHeight: '50dvh', display: 'flex', alignItems: 'center',
      justifyContent: 'center', color: '#9CA3AF', fontSize: '14px',
      fontFamily: '"DM Sans", system-ui, sans-serif',
    }}>
      Loading…
    </div>
  );
}

// ── Chef shell — sidebar + bottom nav + main content ─────────────────────────
function ChefShell({ children }: { children: ReactNode }) {
  const location = useLocation();
  const onPublicPage = isPublicPath(location.pathname) || location.pathname === '/onboarding';

  if (onPublicPage) {
    // Public pages (booking form, sign-in) and onboarding render without nav chrome
    return <>{children}</>;
  }

  return (
    <div className="app-shell">
      <SideNav />
      <div className="app-content-col">
        <main className="app-main">
          <ErrorBoundary resetKey={location.key}>{children}</ErrorBoundary>
        </main>
        <BottomNav />
      </div>
    </div>
  );
}

// ── Root ──────────────────────────────────────────────────────────────────────
export default function App() {
  const [appSettings, setAppSettings] = useState<Partial<Settings>>({});

  const loadSettings = () => {
    // The signed-in chef's own settings. On signed-out pages this 401s and
    // the context stays empty; booking pages load the chef's public profile
    // themselves.
    fetch('/api/settings')
      .then(r => r.ok ? r.json() : {})
      .then((s: Partial<Settings> | null) => {
        setAppSettings(s || {});
        document.title = s?.business_name || APP_NAME;
      })
      .catch(() => {});
  };

  useEffect(() => { loadSettings(); }, []);

  return (
    <SettingsContext.Provider value={{ ...appSettings, refreshSettings: loadSettings }}>
    <BrowserRouter>
      <ToastProvider>
        <AuthGate>
          <ChefShell>
            <Suspense fallback={<PageLoadingFallback />}>
              <Routes>
                {/* Public — no auth required */}
                <Route path="/welcome"    element={<Welcome />} />
                <Route path="/login"      element={<Login />} />
                <Route path="/signup"     element={<Signup />} />
                <Route path="/forgot"     element={<ForgotPassword />} />
                <Route path="/reset"      element={<ResetPassword />} />
                <Route path="/book/:slug" element={<IntakeForm />} />
                <Route path="/intake/new" element={<IntakeForm />} />
                <Route path="/intake/:id" element={<IntakeForm />} />
                <Route path="/invoice/:id" element={<InvoicePay />} />

                {/* First-run setup for new accounts */}
                <Route path="/onboarding" element={<Onboarding />} />

                {/* Protected — chef only */}
                <Route path="/"                   element={<Dashboard />} />
                <Route path="/clients"            element={<Clients />} />
                <Route path="/clients/new"        element={<ClientForm />} />
                <Route path="/clients/:id"        element={<ClientDetail />} />
                <Route path="/clients/:id/edit"   element={<ClientForm />} />
                <Route path="/recipes"            element={<Recipes />} />
                <Route path="/recipes/import"     element={<RecipeImport />} />
                <Route path="/recipes/new"        element={<RecipeDetail edit />} />
                <Route path="/recipes/:id"        element={<RecipeDetail />} />
                <Route path="/recipes/:id/edit"   element={<RecipeDetail edit />} />
                <Route path="/sessions"           element={<Sessions />} />
                <Route path="/sessions/new"       element={<SessionNew />} />
                <Route path="/sessions/:id"       element={<SessionDetail />} />
                <Route path="/templates"          element={<Templates />} />
                <Route path="/costs"              element={<Costs />} />
                <Route path="/settings"           element={<Settings />} />
                <Route path="/planner"            element={<Planner />} />
                <Route path="/calendar"           element={<Calendar />} />
                <Route path="/intake"             element={<IntakeAdmin />} />
                <Route path="/invoices"           element={<Invoices />} />
                <Route path="/invoices/new"       element={<InvoiceBuilder />} />
                <Route path="/invoices/:id"       element={<InvoiceBuilder />} />

                {/* Catch-all — redirect unknown paths to dashboard */}
                <Route path="*" element={<Navigate to="/" replace />} />
              </Routes>
            </Suspense>
          </ChefShell>
        </AuthGate>
      </ToastProvider>
    </BrowserRouter>
    </SettingsContext.Provider>
  );
}
