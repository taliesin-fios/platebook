// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Generated from server-rs (models.rs, routes/intake.rs, payments.rs, the
// status unions in api_types.rs); regenerate with `cargo test` in server-rs.
import type { Intake } from './generated/Intake';
import type { IntakeConverted } from './generated/IntakeConverted';
import type { IntakeStatus } from './generated/IntakeStatus';
import type { IntakeSubmitted } from './generated/IntakeSubmitted';
import type { IntakeWithSlug } from './generated/IntakeWithSlug';
import type { PaymentStatus } from './generated/PaymentStatus';

export type { Intake } from './generated/Intake';
export type { IntakeStatus } from './generated/IntakeStatus';
export type { PaymentStatus } from './generated/PaymentStatus';
export type { PaymentStatusResponse } from './generated/PaymentStatusResponse';

/** GET and PUT /api/intake/:id. */
export type IntakeDetail = IntakeWithSlug;

/** POST /api/intake (public booking form submission). */
export type IntakeSubmitResponse = IntakeSubmitted;

/** POST /api/intake/:id/convert. */
export type IntakeConvertResponse = IntakeConverted;

/**
 * An intake row with its JSON columns still as JSON text. The Node server's
 * POST /api/payments/manual/action returned this shape; the Rust server
 * returns a parsed `Intake`. Kept so the client handles both during cutover.
 */
export type IntakeRow = Omit<Intake, 'allergens' | 'dietary_styles' | 'liked_proteins'> & {
  allergens: string | null;
  dietary_styles: string | null;
  liked_proteins: string | null;
};

export interface PaymentStatusInfo {
  label: string;
  color: string;
  group: 'manual' | 'legacy';
}

export interface ManualAction {
  payment_status: PaymentStatus;
  intake_status: IntakeStatus | null;
  label: string;
}

/** GET /api/payments/mode. */
export interface PaymentsMode {
  mode: string;
  online_enabled: boolean;
  statuses: Record<PaymentStatus, PaymentStatusInfo>;
  actions: Record<string, ManualAction>;
}
