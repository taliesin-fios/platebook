// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! TypeScript-only vocabulary for the generated client types.
//!
//! Status columns are TEXT in Postgres (and the routes keep them as
//! `String`), but the client works with closed unions. These enums exist so
//! `ts-rs` can emit those unions; row fields refer to them with
//! `#[ts(as = "...")]`.

use ts_rs::TS;

/// `sessions.status`.
#[derive(TS)]
#[ts(export)]
pub enum SessionStatus {
    Pending,
    Confirmed,
    Completed,
    Cancelled,
}

/// `invoices.status`.
#[derive(TS)]
#[ts(export, rename_all = "lowercase")]
pub enum InvoiceStatus {
    Draft,
    Sent,
    Paid,
    Void,
}

/// `client_intakes.status`.
#[derive(TS)]
#[ts(export, rename_all = "snake_case")]
pub enum IntakeStatus {
    Submitted,
    DepositPending,
    DepositPaid,
    ReviewedByChef,
    Booked,
    Cancelled,
}

/// `client_intakes.payment_status`: manual statuses, then legacy Stripe ones.
#[derive(TS)]
#[ts(export, rename_all = "snake_case")]
pub enum PaymentStatus {
    NotRequested,
    DepositRequested,
    InvoiceSent,
    PaymentPending,
    PaidOffline,
    Waived,
    Unpaid,
    Pending,
    Paid,
    Refunded,
    Failed,
}
