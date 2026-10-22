// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useEffect, useState, useContext } from 'react';
import { useNavigate, useLocation } from 'react-router';
import { useAuth, SettingsContext } from '../App';
import { localTodayStr } from '../utils/dateUtils';
import { APP_NAME } from '../constants';

interface IconProps {
  filled?: boolean;
}

const HomeIcon = ({ filled }: IconProps) => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
    <polyline points="9 22 9 12 15 12 15 22" />
  </svg>
);

const ClientIcon = ({ filled }: IconProps) => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2" />
    <circle cx="9" cy="7" r="4" />
    <path d="M23 21v-2a4 4 0 00-3-3.87M16 3.13a4 4 0 010 7.75" />
  </svg>
);

const CalendarIcon = ({ filled }: IconProps) => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <rect x="3" y="4" width="18" height="18" rx="2" ry="2" />
    <line x1="16" y1="2" x2="16" y2="6" />
    <line x1="8" y1="2" x2="8" y2="6" />
    <line x1="3" y1="10" x2="21" y2="10" />
  </svg>
);

const RecipeIcon = ({ filled }: IconProps) => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2" />
    <rect x="9" y="3" width="6" height="4" rx="2" />
    <line x1="9" y1="12" x2="15" y2="12" />
    <line x1="9" y1="16" x2="13" y2="16" />
  </svg>
);

const CostIcon = ({ filled }: IconProps) => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <line x1="12" y1="1" x2="12" y2="23" />
    <path d="M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6" />
  </svg>
);

const SettingsIcon = ({ filled }: IconProps) => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
  </svg>
);

const InvoiceIcon = ({ filled }: IconProps) => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
    <polyline points="14 2 14 8 20 8" />
    <line x1="16" y1="13" x2="8" y2="13" />
    <line x1="16" y1="17" x2="8" y2="17" />
    <polyline points="10 9 9 9 8 9" />
  </svg>
);

const TABS = [
  { path: '/',         label: 'Dashboard', Icon: HomeIcon     },
  { path: '/clients',  label: 'Clients',   Icon: ClientIcon   },
  { path: '/sessions', label: 'Sessions',  Icon: CalendarIcon },
  { path: '/recipes',  label: 'Recipes',   Icon: RecipeIcon   },
  { path: '/invoices', label: 'Invoices',  Icon: InvoiceIcon  },
  { path: '/costs',    label: 'Costs',     Icon: CostIcon     },
];

export default function SideNav() {
  const navigate    = useNavigate();
  const location    = useLocation();
  const { logout }  = useAuth();
  const appSettings = useContext(SettingsContext);
  const [upcoming,  setUpcoming] = useState(0);

  // Derive logo + name from global SettingsContext (no extra fetch needed)
  const logo    = appSettings.logo_base64 || null;
  const bizName = appSettings.business_name || APP_NAME;

  useEffect(() => {
    fetch(`/api/sessions?upcoming=true&today=${localTodayStr()}`)
      .then(r => r.json())
      .then(data => setUpcoming(Array.isArray(data) ? data.length : 0))
      .catch(() => {});
  }, [location.pathname]);

  const isActive = (tabPath: string) => {
    if (tabPath === '/') return location.pathname === '/';
    return location.pathname.startsWith(tabPath);
  };

  return (
    <aside className="side-nav">
      {/* Brand */}
      <div className="side-nav-brand">
        {logo ? (
          <div className="flex items-center gap-2 mb-1">
            <img
              src={logo}
              alt="Logo"
              className="w-8 h-8 object-contain rounded-lg shrink-0"
            />
            <span className="side-nav-brand-text leading-tight">{bizName}</span>
          </div>
        ) : (
          <span className="side-nav-brand-text">{bizName}</span>
        )}
        <span className="side-nav-brand-sub">Chef Dashboard</span>
      </div>

      {/* Main nav */}
      <nav className="side-nav-links">
        {TABS.map(({ path, label, Icon }) => {
          const active     = isActive(path);
          const showBadge  = path === '/sessions' && upcoming > 0;
          return (
            <button
              key={path}
              className={`side-nav-item ${active ? 'side-nav-item-active' : 'side-nav-item-inactive'}`}
              onClick={() => navigate(path)}
            >
              <span className="relative">
                <Icon filled={active} />
                {showBadge && (
                  <span className="absolute -top-1 -right-2 bg-danger text-white text-[9px] font-bold w-3.5 h-3.5 rounded-full flex items-center justify-center">
                    {upcoming > 9 ? '9+' : upcoming}
                  </span>
                )}
              </span>
              <span className="side-nav-label">{label}</span>
            </button>
          );
        })}
      </nav>

      {/* Settings + Logout at bottom */}
      <div className="side-nav-footer">
        <button
          className={`side-nav-item ${isActive('/settings') ? 'side-nav-item-active' : 'side-nav-item-inactive'}`}
          onClick={() => navigate('/settings')}
        >
          <SettingsIcon filled={isActive('/settings')} />
          <span className="side-nav-label">Settings</span>
        </button>
        <button
          className="side-nav-item side-nav-item-inactive"
          onClick={logout}
          style={{ marginTop: '2px' }}
          title="Sign out"
        >
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/>
            <polyline points="16 17 21 12 16 7"/>
            <line x1="21" y1="12" x2="9" y2="12"/>
          </svg>
          <span className="side-nav-label">Sign Out</span>
        </button>
      </div>
    </aside>
  );
}
