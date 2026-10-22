// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Generated from server-rs (models.rs, routes/sessions.rs, grocery.rs,
// planner.rs, templates.rs); regenerate with `cargo test` in server-rs.
import type { GeneratedGroceryItem } from './generated/GeneratedGroceryItem';
import type { SessionTemplateRow } from './generated/SessionTemplateRow';
import type { TemplateRecipe } from './generated/TemplateRecipe';
import type { Flag, Id } from './common';

export type { SessionStatus } from './generated/SessionStatus';
export type { SessionRow } from './generated/SessionRow';
export type { SessionListItem } from './generated/SessionListItem';
export type { SessionDish } from './generated/SessionDish';
export type { SessionDetail } from './generated/SessionDetail';
export type { CompletedSession } from './generated/CompletedSession';
export type { FromTemplateResponse } from './generated/FromTemplateResponse';
export type { EmailSentResponse } from './generated/EmailSentResponse';
export type { GeneratedGroceryItem } from './generated/GeneratedGroceryItem';
export type { GroceryItemRow } from './generated/GroceryItemRow';
export type { GroceryListResponse } from './generated/GroceryListResponse';
export type { PlannerItem } from './generated/PlannerItem';
export type { PlannerSection } from './generated/PlannerSection';
export type { PlannerResponse } from './generated/PlannerResponse';
export type { SessionTemplateRow } from './generated/SessionTemplateRow';
export type { TemplateRecipe } from './generated/TemplateRecipe';
export type { TemplateWithRecipes } from './generated/TemplateWithRecipes';

/**
 * A grocery list row as the session page keeps it: either a saved row
 * (`GroceryItemRow`, with an id) or one generated on the fly (no id yet).
 */
export type GroceryItem = GeneratedGroceryItem & {
  id?: Id;
  session_id?: Id | null;
  is_checked?: Flag;
};

/** A saved template; `recipes` is attached by GET/PUT, not by POST. */
export type SessionTemplate = SessionTemplateRow & { recipes?: TemplateRecipe[] };

/** POST /api/ai/suggest-menu (Claude's JSON, ids filtered to real recipes). */
export interface MenuSuggestion {
  suggested_recipe_ids: Id[];
  reasoning: string;
}
