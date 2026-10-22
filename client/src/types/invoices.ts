// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Generated from server-rs (models.rs, routes/invoices.rs); regenerate with
// `cargo test` in server-rs.
import type { InvoiceDetail } from './generated/InvoiceDetail';

export type { InvoiceStatus } from './generated/InvoiceStatus';
export type { InvoiceRow } from './generated/InvoiceRow';
export type { InvoiceLineItem } from './generated/InvoiceLineItem';
export type { InvoiceListItem } from './generated/InvoiceListItem';
export type { InvoiceDetail } from './generated/InvoiceDetail';
export type { InvoiceDeleteResponse } from './generated/InvoiceDeleteResponse';

/** GET /api/invoices/:id, and the body of every invoice mutation. */
export type Invoice = InvoiceDetail;
