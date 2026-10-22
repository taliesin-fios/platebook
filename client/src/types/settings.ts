// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Generated from server-rs (accounts.rs, routes/settings.rs, public.rs,
// onboarding.rs); regenerate with `cargo test` in server-rs.
import type { SettingsResponse } from './generated/SettingsResponse';

export type { SettingsRow } from './generated/SettingsRow';
export type { SettingsResponse } from './generated/SettingsResponse';
export type { PublicProfile } from './generated/PublicProfile';
export type { ClearSampleResponse } from './generated/ClearSampleResponse';
export type { SeedLibrary } from './generated/SeedLibrary';
export type { OnboardingSeedResponse } from './generated/OnboardingSeedResponse';

/** GET /api/settings and PUT /api/settings. */
export type Settings = SettingsResponse;

/**
 * The settings context available everywhere via `useAppSettings()`. Empty
 * until GET /api/settings resolves (and on signed-out pages).
 */
export type AppSettings = Partial<Settings> & {
  refreshSettings: () => void;
};
