// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useApi } from './useApi';
import type { SignupStatus } from '../types';

/**
 * Whether this server lets a new account be created (`SIGNUP_MODE`).
 * `true` while loading or if the check fails, so sign-up links don't flicker
 * away; the server still refuses a closed sign-up.
 */
export function useSignupOpen(): boolean {
  const { data } = useApi<SignupStatus>('/api/auth/signup');
  return data?.open ?? true;
}
