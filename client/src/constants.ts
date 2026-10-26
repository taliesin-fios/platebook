// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/** The recipe fields the feasibility estimate reads. */
export type FeasibilityDish = {
  prep_time_min?: number | null;
  cook_time_min?: number | null;
  session_fit?: string | null;
};

export type FeasibilityLevel = 'GREEN' | 'AMBER' | 'RED' | 'NONE';

export interface Feasibility {
  level: FeasibilityLevel;
  estimated_min: number;
  total_active?: number;
  passive?: number;
  buffer?: number;
  overlap_credit?: number;
}

type FeasibilityRule = (est: number, dishes: FeasibilityDish[], sessionFriendlyCount: number) => boolean;

export const SESSION_FEASIBILITY: Record<'GREEN' | 'AMBER' | 'RED', FeasibilityRule> = {
  GREEN: (est, dishes, sessionFriendlyCount) =>
    est <= 210 && sessionFriendlyCount === dishes.length && dishes.length <= 5,
  AMBER: (est, dishes, sessionFriendlyCount) =>
    (est > 210 && est <= 240) || (sessionFriendlyCount === dishes.length - 1 && dishes.length <= 5),
  RED: (est, dishes, sessionFriendlyCount) =>
    est > 240 || sessionFriendlyCount < dishes.length - 1 || dishes.length > 5,
};

export const calcFeasibility = (dishes: FeasibilityDish[] | null | undefined): Feasibility => {
  if (!dishes || dishes.length === 0) return { level: 'NONE', estimated_min: 0 };
  const total_active = dishes.reduce((s, d) => s + (d.prep_time_min || 0), 0);
  const passive = Math.max(...dishes.map(d => d.cook_time_min || 0));
  const buffer = 30;
  const overlap_credit = (dishes.length - 1) * 5;
  const estimated_min = total_active + passive + buffer - overlap_credit;
  const sfCount = dishes.filter(d => d.session_fit === 'Session-Friendly').length;

  let level: FeasibilityLevel;
  if (SESSION_FEASIBILITY.GREEN(estimated_min, dishes, sfCount)) level = 'GREEN';
  else if (SESSION_FEASIBILITY.AMBER(estimated_min, dishes, sfCount)) level = 'AMBER';
  else level = 'RED';

  return { level, estimated_min, total_active, passive, buffer, overlap_credit };
};

export const STORE_SECTIONS: string[] = [
  'Produce', 'Meat & Seafood', 'Dairy & Eggs',
  'Pantry & Dry Goods', 'Canned & Jarred', 'Herbs & Spices', 'Other'
];

export const ALLERGEN_KEYWORDS: Record<string, string[]> = {
  // A3: 'butter' used to match 'peanut butter'/'almond butter' — detection uses word boundaries now
  Dairy: ['milk', 'cream', 'butter', 'cheese', 'ricotta', 'parmesan', 'gruyere', 'mozzarella', 'yogurt', 'ghee', 'crème fraîche', 'brie', 'feta', 'burrata', 'mascarpone', 'half-and-half', 'buttermilk'],
  // A1: added bread, malt, beer, farro, spelt, bulgur, cracker, crackers
  Gluten: ['flour', 'bread', 'breadcrumbs', 'cracker', 'crackers', 'pasta', 'noodles', 'panko', 'wheat', 'barley', 'malt', 'rye', 'beer', 'ale', 'lager', 'soy sauce', 'semolina', 'couscous', 'orzo', 'farro', 'spelt', 'bulgur', 'cake', 'pastry'],
  // A2: 'egg' used to match 'eggplant' — detection uses word boundaries now
  Eggs: ['egg', 'eggs', 'egg yolk', 'egg white', 'whole egg', 'mayonnaise', 'aioli'],
  'Tree Nuts': ['almond', 'walnut', 'pecan', 'cashew', 'pistachio', 'hazelnut', 'pine nut', 'macadamia', 'chestnut'],
  Peanuts: ['peanut', 'peanut butter', 'groundnut'],
  Fish: ['anchovy', 'salmon', 'tuna', 'cod', 'halibut', 'sea bass', 'tilapia', 'fish sauce', 'worcestershire'],
  Shellfish: ['shrimp', 'crab', 'lobster', 'scallop', 'clam', 'mussel', 'oyster', 'prawn'],
  Soy: ['soy sauce', 'tofu', 'edamame', 'miso', 'tamari', 'tempeh'],
  Sesame: ['sesame', 'tahini', 'sesame oil'],
};

export const ALLERGENS_LIST = Object.keys(ALLERGEN_KEYWORDS);

export const ALLERGEN_ICONS: Record<string, string> = {
  Dairy:       '🥛',
  Gluten:      '🌾',
  Eggs:        '🥚',
  'Tree Nuts': '🌳',  // tree — represents "tree nut" category, distinct from peanut
  Peanuts:     '🥜',
  Fish:        '🐟',
  Shellfish:   '🦐',
  Soy:         '🫘',
  Sesame:      '🌿',
};

// A2/A3: use word-boundary matching so 'egg' doesn't match 'eggplant',
// and 'butter' doesn't match 'peanut butter' or 'almond butter'.
const ALLERGEN_EXCLUSIONS: Record<string, RegExp[]> = {
  Dairy: [/peanut\s+butter/i, /almond\s+butter/i, /cashew\s+butter/i, /sunflower\s+butter/i, /nut\s+butter/i, /cocoa\s+butter/i, /apple\s+butter/i, /shea\s+butter/i, /body\s+butter/i],
  Eggs:  [/eggplant/i, /eggnog/i],
};

export const detectAllergens = (ingredients: Array<{ name?: string | null }> = []): string[] => {
  const detected = new Set<string>();
  const lowerIngNames = ingredients.map(i => (i.name || '').toLowerCase());
  for (const [allergen, keywords] of Object.entries(ALLERGEN_KEYWORDS)) {
    const exclusions = ALLERGEN_EXCLUSIONS[allergen] || [];
    for (const kw of keywords) {
      const re = new RegExp(`\\b${kw.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`, 'i');
      if (lowerIngNames.some(n => {
        if (exclusions.some(ex => ex.test(n))) return false;
        return re.test(n);
      })) {
        detected.add(allergen);
        break;
      }
    }
  }
  return [...detected];
};

/** Time zones offered in Settings (the browser's own zone is added if missing). */
export const TIMEZONE_OPTIONS: ReadonlyArray<readonly [string, string]> = [
  ['America/New_York', 'Eastern (New York)'],
  ['America/Chicago', 'Central (Chicago)'],
  ['America/Denver', 'Mountain (Denver)'],
  ['America/Phoenix', 'Mountain, no DST (Phoenix)'],
  ['America/Los_Angeles', 'Pacific (Los Angeles)'],
  ['America/Anchorage', 'Alaska (Anchorage)'],
  ['Pacific/Honolulu', 'Hawaii (Honolulu)'],
  ['America/Toronto', 'Eastern (Toronto)'],
  ['America/Vancouver', 'Pacific (Vancouver)'],
  ['Europe/London', 'London'],
  ['UTC', 'UTC'],
];

/** The viewer's IANA time zone, e.g. "America/Chicago" ('' if unknown). */
export const browserTimeZone = (): string => {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || '';
  } catch {
    return '';
  }
};

/** Choices for a session's `repeat_every_weeks` (null = one-off). */
export const REPEAT_OPTIONS: ReadonlyArray<readonly [number | null, string]> = [
  [null, 'Does not repeat'],
  [1, 'Every week'],
  [2, 'Every 2 weeks'],
  [3, 'Every 3 weeks'],
  [4, 'Every 4 weeks'],
];

export const repeatLabel = (weeks: number | null | undefined): string =>
  REPEAT_OPTIONS.find(([w]) => w === (weeks ?? null))?.[1] ?? `Every ${weeks} weeks`;

export const MARGIN_THRESHOLDS = {
  healthy: 60,
  acceptable: 40,
};

export const marginColor = (pct: number): string => {
  if (pct >= MARGIN_THRESHOLDS.healthy) return 'text-green-dark';
  if (pct >= MARGIN_THRESHOLDS.acceptable) return 'text-amber-600';
  return 'text-danger';
};

export const marginLabel = (pct: number): string => {
  if (pct >= MARGIN_THRESHOLDS.healthy) return '✓ Healthy';
  if (pct >= MARGIN_THRESHOLDS.acceptable) return '◐ Acceptable';
  return '↓ Review costs';
};

export const marginBg = (pct: number): string => {
  if (pct >= MARGIN_THRESHOLDS.healthy) return 'bg-green-light text-green-dark';
  if (pct >= MARGIN_THRESHOLDS.acceptable) return 'bg-amber-50 text-amber-700';
  return 'bg-red-50 text-danger';
};

export const DIETARY_STYLES = [
  'Omnivore', 'Pescatarian', 'Vegetarian', 'Vegan',
  'Gluten-Free', 'Dairy-Free', 'Paleo', 'Low-Carb',
  'Mediterranean', 'Nut-Free Kitchen Required'
];

export const PROTEINS = [
  'Chicken', 'Beef', 'Pork', 'Lamb', 'Fish',
  'Shellfish', 'Plant-Based Only', 'No Red Meat'
];

export const EQUIPMENT_OPTIONS = [
  'Stand Mixer', 'Food Processor', 'Instant Pot',
  'Dutch Oven', 'Large Sheet Pans', 'Cast Iron Skillet'
];

export const REHEAT_INSTRUCTIONS: Record<string, string> = {
  default: 'Stovetop, medium-low heat, stir occasionally, 5–8 min',
  stew: 'Stovetop, medium-low heat, stir occasionally, 5–8 min',
  curry: 'Stovetop, medium-low, splash of water if needed, 5–8 min',
  pasta: 'Oven 325°F covered 20 min, or microwave covered 2–3 min',
  meatballs: 'Stovetop, medium-low, 5–8 min, or microwave 2 min',
  bowl: 'Warm meatballs separately, serve over cold grain',
};

export const getReheatedInstructions = (recipeName = ''): string => {
  const name = recipeName.toLowerCase();
  if (name.includes('curry') || name.includes('massaman') || name.includes('panang')) return REHEAT_INSTRUCTIONS.curry;
  if (name.includes('ziti') || name.includes('lasagna') || name.includes('casserole') || name.includes('gratin')) return REHEAT_INSTRUCTIONS.pasta;
  if (name.includes('meatball') || name.includes('meatballs')) return REHEAT_INSTRUCTIONS.meatballs;
  if (name.includes('bowl')) return REHEAT_INSTRUCTIONS.bowl;
  return REHEAT_INSTRUCTIONS.stew;
};

/**
 * Returns the reheat category for color-coding in the client PDF.
 * Categories: 'stovetop' | 'oven' | 'cold'
 */
export const getReheatedCategory = (recipeName = ''): 'stovetop' | 'oven' | 'cold' => {
  const name = recipeName.toLowerCase();
  // Cold / no-heat dishes
  if (name.includes('salad') || name.includes('sashimi') || name.includes('ceviche') ||
      name.includes('gazpacho') || name.includes('crudo') || name.includes('tartare') ||
      name.includes('carpaccio')) return 'cold';
  // Oven dishes
  if (name.includes('lasagna') || name.includes('ziti') || name.includes('casserole') ||
      name.includes('gratin') || name.includes('bake') || name.includes('roast') ||
      name.includes('frittata') || name.includes('quiche') || name.includes('enchilada') ||
      name.includes('pot pie')) return 'oven';
  // Everything else defaults to stovetop
  return 'stovetop';
};

export const CATEGORY_COLORS: Record<string, string> = {
  Couples: 'bg-navy text-white',
  Families: 'bg-green-dark text-white',
  'Plant-Based': 'bg-terracotta text-white',
  Other: 'bg-honey text-olive',
};

export const CATEGORY_BAR_COLORS: Record<string, string> = {
  Couples: '#5E7F57',
  Families: '#3A5C32',
  'Plant-Based': '#C56A4A',
  Other: '#D8A13C',
};

export const STATUS_COLORS: Record<string, string> = {
  Pending: 'bg-gray-100 text-gray-600',
  Confirmed: 'bg-blue-100 text-blue-700',
  Completed: 'bg-green-light text-green-dark',
  Cancelled: 'bg-red-100 text-danger',
};

export const FEASIBILITY_COLORS: Record<string, string> = {
  GREEN: 'bg-green-light text-green-dark',
  AMBER: 'bg-amber-50 text-amber-700',
  RED: 'bg-red-50 text-danger',
  NONE: 'bg-gray-100 text-gray-500',
};

// ── Product identity ───────────────────────────────────────────────────────────
// The platform's own name, shown on the welcome / sign-in pages and as a
// fallback wherever a chef hasn't set a business name yet. Rename here.
export const APP_NAME = 'Platebook';
export const APP_TAGLINE = 'The all-in-one kitchen office for home & personal chefs';
/** Platebook's author and home page, credited in the app (see NOTICE). */
export const APP_VERSION = '0.2.0';
export const APP_AUTHOR = 'taliesin-fios';
export const APP_REPO_URL = 'https://github.com/taliesin-fios/platebook';

// Ways a chef can describe their business during onboarding (multi-select)
export const SERVICE_TYPES = [
  { id: 'in_home',   label: 'Cooking in clients’ homes', icon: '🏡' },
  { id: 'meal_prep', label: 'Weekly meal prep',              icon: '🥡' },
  { id: 'events',    label: 'Dinner parties & events',       icon: '🍷' },
  { id: 'classes',   label: 'Cooking classes',               icon: '👩‍🍳' },
  { id: 'dietary',   label: 'Special-diet & medical meals',  icon: '🥗' },
];

export const PASSWORD_MIN_LENGTH = 10;

/** Public booking link a chef shares with prospective clients. */
export const bookingUrl = (slug: string | null | undefined, origin: string = window.location.origin): string =>
  slug ? `${origin}/book/${slug}` : `${origin}/intake/new`;

/** Dollars (string/number) ⇄ integer cents for the deposit field. */
export const dollarsToCents = (v: unknown): number => {
  const n = Number(v);
  return Number.isFinite(n) && n > 0 ? Math.round(n * 100) : 0;
};
export const centsToDollars = (c: unknown): string => (Number(c) > 0 ? (Number(c) / 100).toFixed(2).replace(/\.00$/, '') : '0');
