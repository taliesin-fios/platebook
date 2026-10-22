// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Generated from server-rs (models.rs, routes/recipes.rs, recipe_import.rs);
// regenerate with `cargo test` in server-rs.
export type { Ingredient } from './generated/Ingredient';
export type { RecipeRow } from './generated/RecipeRow';
export type { Recipe } from './generated/Recipe';
export type { RecipeDetail } from './generated/RecipeDetail';
export type { RecipeWithIngredients } from './generated/RecipeWithIngredients';
export type { ParsedIngredient } from './generated/ParsedIngredient';
export type { ParsedRecipe } from './generated/ParsedRecipe';
export type { RecipeImportResponse } from './generated/RecipeImportResponse';

/** A recipe's session fit; the server stores free text, these two are standard. */
export type SessionFit = 'Session-Friendly' | 'Long-Cook' | (string & {});

export type ConfidenceLevel = 'high' | 'medium' | 'low';
