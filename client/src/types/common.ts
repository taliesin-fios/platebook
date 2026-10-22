// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Shapes shared across the API.
 *
 * Response types are generated from the Rust server's structs into
 * `./generated` (ts-rs; run `cargo test` in server-rs after changing a route's
 * response, and commit the result). This file holds the few aliases the
 * generated types are written in terms of, inlined there as literals.
 */

/** Row id (Postgres BIGINT identity; always a safe JS integer). */
export type Id = number;

/** Boolean columns are sent as 0 or 1 (the SQLite-era API shape, kept for compatibility). */
export type Flag = 0 | 1;

/** `YYYY-MM-DD` date-only string. Parse with `parseLocalDate`, never `parseISO`. */
export type DateString = string;

/** UTC timestamp as `YYYY-MM-DD HH:MM:SS`. Format with `fmtUtcTimestamp`. */
export type UtcTimestamp = string;

/** Body of every non-2xx JSON response. */
export interface ApiError {
  error: string;
  code?: string;
}

/** `{ success: true }` acknowledgement returned by most DELETE routes. */
export interface SuccessResponse {
  success: true;
}
