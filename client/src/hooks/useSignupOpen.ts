// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useApi } from './useApi';
import type { SignupStatus } from '../types';

/**
 * The server's sign-up status (`SIGNUP_MODE`, `DEMO_MODE`). With an invite
 * token from a `/signup?invite=…` link, `open` says whether that invite still
 * works. `open` is assumed true while loading or if the check fails, so
 * sign-up links don't flicker away (the server still refuses a sign-up it
 * doesn't allow); `demo` and `invite_only` are `false` until it answers.
 */
export function useSignupStatus(invite = ''): SignupStatus & { loading: boolean } {
  const url = invite ? `/api/auth/signup?invite=${encodeURIComponent(invite)}` : '/api/auth/signup';
  const { data, loading } = useApi<SignupStatus>(url, [url]);
  return {
    open: data?.open ?? true,
    invite_only: data?.invite_only ?? false,
    demo: data?.demo ?? false,
    loading,
  };
}

/** Whether this server lets a new account be created without an invite. */
export function useSignupOpen(): boolean {
  return useSignupStatus().open;
}
