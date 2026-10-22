// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import type { ReactNode } from 'react';
import { useNavigate } from 'react-router';

interface PageHeaderProps {
  title: ReactNode;
  /** Path to navigate to, or a handler, for the back arrow. */
  back?: string | (() => void);
  action?: ReactNode;
  subtitle?: ReactNode;
}

export default function PageHeader({ title, back, action, subtitle }: PageHeaderProps) {
  const navigate = useNavigate();
  return (
    <div className="page-header">
      {back && (
        <button
          onClick={() => typeof back === 'function' ? back() : navigate(back)}
          className="text-white opacity-80 hover:opacity-100 flex items-center gap-1"
          style={{ minHeight: 'auto' }}
        >
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
            <polyline points="15 18 9 12 15 6" />
          </svg>
        </button>
      )}
      <div className="flex-1">
        <div className="page-title">{title}</div>
        {subtitle && <div className="text-xs text-blue-200 font-sans mt-0.5">{subtitle}</div>}
      </div>
      {action && <div>{action}</div>}
    </div>
  );
}
