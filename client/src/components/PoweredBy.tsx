// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { APP_AUTHOR, APP_NAME, APP_REPO_URL } from '../constants';

/**
 * "Powered by Platebook" credit for the public, client-facing pages. Platebook
 * is source-available under the PolyForm Shield License; please keep this
 * credit (see NOTICE).
 */
export default function PoweredBy({ className = '' }: { className?: string }) {
  return (
    <p className={`text-[11px] text-center text-gray-400 ${className}`}>
      Powered by{' '}
      <a href={APP_REPO_URL} target="_blank" rel="noopener noreferrer" title={`${APP_NAME} by ${APP_AUTHOR}`} className="underline decoration-dotted hover:text-gray-500">
        {APP_NAME}
      </a>
    </p>
  );
}
