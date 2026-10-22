// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useEffect, useState } from 'react';
import { useNavigate, useLocation } from 'react-router';
import { useAuth } from '../App';
import { localTodayStr } from '../utils/dateUtils';

interface IconProps {
  filled?: boolean;
}

const HomeIcon = ({ filled }: IconProps) => (
  <svg width="22" height="22" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
    <polyline points="9 22 9 12 15 12 15 22" />
  </svg>
);

const ClientIcon = ({ filled }: IconProps) => (
  <svg width="22" height="22" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2" />
    <circle cx="9" cy="7" r="4" />
    <path d="M23 21v-2a4 4 0 00-3-3.87M16 3.13a4 4 0 010 7.75" />
  </svg>
);

const CalendarIcon = ({ filled }: IconProps) => (
  <svg width="22" height="22" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <rect x="3" y="4" width="18" height="18" rx="2" ry="2" />
    <line x1="16" y1="2" x2="16" y2="6" />
    <line x1="8" y1="2" x2="8" y2="6" />
    <line x1="3" y1="10" x2="21" y2="10" />
  </svg>
);

const RecipeIcon = ({ filled }: IconProps) => (
  <svg width="22" height="22" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2" />
    <rect x="9" y="3" width="6" height="4" rx="2" />
    <line x1="9" y1="12" x2="15" y2="12" />
    <line x1="9" y1="16" x2="13" y2="16" />
  </svg>
);

const CostIcon = ({ filled }: IconProps) => (
  <svg width="22" height="22" viewBox="0 0 24 24" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
    <line x1="12" y1="1" x2="12" y2="23" />
    <path d="M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6" />
  </svg>
);

const TABS = [
  { path: '/', label: 'Dashboard', Icon: HomeIcon },
  { path: '/clients', label: 'Clients', Icon: ClientIcon },
  { path: '/sessions', label: 'Sessions', Icon: CalendarIcon },
  { path: '/recipes', label: 'Recipes', Icon: RecipeIcon },
  { path: '/costs', label: 'Costs', Icon: CostIcon },
];

export default function BottomNav() {
  const navigate = useNavigate();
  const location = useLocation();
  const { logout } = useAuth();
  const [upcomingCount, setUpcomingCount] = useState(0);

  // C8: use AbortController so the fetch is cancelled when the component unmounts
  // or when the path changes before the previous fetch completes.
  useEffect(() => {
    const controller = new AbortController();
    fetch(`/api/sessions?upcoming=true&today=${localTodayStr()}`, { signal: controller.signal })
      .then(r => r.json())
      .then(data => setUpcomingCount(Array.isArray(data) ? data.length : 0))
      .catch(() => {});
    return () => controller.abort();
  }, [location.pathname]);

  const isActive = (tabPath: string) => {
    if (tabPath === '/') return location.pathname === '/';
    return location.pathname.startsWith(tabPath);
  };

  return (
    <nav className="bottom-nav">
      {TABS.map(({ path, label, Icon }) => {
        const active = isActive(path);
        const showBadge = path === '/sessions' && upcomingCount > 0;
        return (
          <button
            key={path}
            className={`bottom-nav-item ${active ? 'active' : 'inactive'}`}
            onClick={() => navigate(path)}
          >
            <div className="relative">
              <Icon filled={active} />
              {showBadge && (
                <span className="absolute -top-1 -right-2 bg-danger text-white text-[10px] font-bold w-4 h-4 rounded-full flex items-center justify-center">
                  {upcomingCount > 9 ? '9+' : upcomingCount}
                </span>
              )}
            </div>
            <span className="text-[11px] font-medium">{label}</span>
          </button>
        );
      })}
      {/* Sign out */}
      <button className="bottom-nav-item inactive" onClick={logout}>
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/>
          <polyline points="16 17 21 12 16 7"/>
          <line x1="21" y1="12" x2="9" y2="12"/>
        </svg>
        <span className="text-[11px] font-medium">Sign Out</span>
      </button>
    </nav>
  );
}
