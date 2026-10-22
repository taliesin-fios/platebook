// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

import { useState, type Dispatch, type SetStateAction } from 'react';
import { useNavigate } from 'react-router';
import PageHeader from '../components/PageHeader';
import StarRating from '../components/StarRating';
import { useToast } from '../components/Toast';
import { apiCall, errorMessage } from '../hooks/useApi';
import { STORE_SECTIONS, detectAllergens, ALLERGENS_LIST } from '../constants';
import type { NumInput } from '../utils/numbers';
import type { ParsedRecipe, RecipeDetail, RecipeImportResponse } from '../types';

interface IngredientDraft {
  name: string;
  qty_4_serving: string | null;
  qty_6_serving: string | null;
  store_section: string | null;
  category?: string | null;
}

/** The recipe under review before it's saved. */
interface ImportForm {
  name: string;
  category: string;
  session_fit: string;
  prep_time_min: NumInput;
  cook_time_min: NumInput;
  servings_base: number;
  hold_quality: number;
  reheat_quality: number;
  key_overlap_ingredients: string[];
  chef_notes: string;
  raw_instructions: string;
  ingredients: IngredientDraft[];
}

/** A parse result from the server importers or the local text parser. */
type ImportResult = Omit<RecipeImportResponse, 'source' | 'parsed'> & {
  parsed: Omit<ParsedRecipe, 'ingredients'> & { ingredients: IngredientDraft[] };
  source?: string;
};

// ── Client-side text parser (no AI, no server call) ──────────────────────────

function parseFraction(str: string) {
  const m = str.match(/^(\d+)\s*\/\s*(\d+)$/);
  if (m) return parseInt(m[1]) / parseInt(m[2]);
  return null;
}

function parseQty(token: string) {
  if (!token) return '';
  const t = token.trim();
  // mixed number: "1 1/2"
  const mixed = t.match(/^(\d+)\s+(\d+\/\d+)$/);
  if (mixed) {
    const frac = parseFraction(mixed[2]);
    return frac !== null ? String(parseFloat(mixed[1]) + frac) : t;
  }
  if (/^\d+\/\d+$/.test(t)) {
    const f = parseFraction(t);
    return f !== null ? String(f) : t;
  }
  return t;
}

const UNIT_WORDS = ['cup', 'cups', 'tbsp', 'tbs', 'tablespoon', 'tablespoons',
  'tsp', 'teaspoon', 'teaspoons', 'oz', 'ounce', 'ounces', 'lb', 'lbs',
  'pound', 'pounds', 'g', 'gram', 'grams', 'kg', 'ml', 'liter', 'liters',
  'L', 'clove', 'cloves', 'slice', 'slices', 'bunch', 'head', 'can', 'cans',
  'package', 'pkg', 'sprig', 'sprigs', 'stalk', 'stalks', 'piece', 'pieces'];

const UNIT_RE = new RegExp(`^(${UNIT_WORDS.join('|')})\\.?$`, 'i');

const SECTION_KEYWORDS: Record<string, string[]> = {
  'Produce':          ['onion','garlic','tomato','pepper','zucchini','spinach','kale','carrot','celery','broccoli','lettuce','mushroom','lemon','lime','apple','avocado','shallot','leek','herb','basil','cilantro','parsley','thyme','rosemary','sage','mint','ginger','scallion','cucumber','eggplant','fennel','beet','potato','sweet potato','corn','asparagus','artichoke','bean sprout'],
  'Meat & Seafood':   ['chicken','beef','pork','lamb','turkey','duck','salmon','shrimp','tilapia','cod','tuna','halibut','scallop','crab','sausage','bacon','prosciutto','pancetta','steak','ground','mince','fillet','thigh','breast','wing','rib','loin'],
  'Dairy & Eggs':     ['milk','cream','butter','cheese','ricotta','parmesan','mozzarella','cheddar','feta','brie','gruyere','yogurt','egg','ghee','crème fraîche','mascarpone','burrata'],
  'Herbs & Spices':   ['cumin','paprika','turmeric','cinnamon','cayenne','coriander','oregano','bay leaf','chili flake','black pepper','white pepper','cardamom','clove','nutmeg','allspice','star anise','curry powder','garam masala','za\'atar'],
  'Canned & Jarred':  ['canned','tomato sauce','crushed tomato','diced tomato','coconut milk','broth','stock','paste','puree','salsa','olive oil','vegetable broth','chicken broth'],
  'Pantry & Dry Goods':['flour','sugar','rice','pasta','noodle','lentil','chickpea','quinoa','bread','panko','breadcrumb','oat','cornstarch','baking','salt','vinegar','soy sauce','fish sauce','honey','maple','mustard','ketchup','hot sauce','worcestershire','tamari','sesame oil'],
};

function guessSection(name: string) {
  const n = name.toLowerCase();
  for (const [section, kws] of Object.entries(SECTION_KEYWORDS)) {
    if (kws.some(k => n.includes(k))) return section;
  }
  return 'Other';
}

// C19: complete unicode-fraction normalisation map
const UNICODE_FRACS: Record<string, string> = {
  '½':'1/2','⅓':'1/3','⅔':'2/3','¼':'1/4','¾':'3/4',
  '⅛':'1/8','⅜':'3/8','⅝':'5/8','⅞':'7/8',
  '⅙':'1/6','⅚':'5/6','⅕':'1/5','⅖':'2/5','⅗':'3/5','⅘':'4/5',
};

function normaliseUnicode(str: string) {
  // Insert a space between a digit and an immediately-following unicode fraction
  // so "1½" becomes "1 ½" before tokenising.
  let s = str;
  for (const uc of Object.keys(UNICODE_FRACS)) {
    s = s.replace(new RegExp(`(\\d)(${uc})`, 'g'), `$1 $2`);
    s = s.replace(new RegExp(uc, 'g'), UNICODE_FRACS[uc]);
  }
  return s;
}

function parseIngredientLine(line: string): IngredientDraft | null {
  line = line.replace(/^[-•*·]\s*/, '').trim();
  if (!line) return null;
  // C19: normalise unicode fractions before tokenising
  line = normaliseUnicode(line);
  const tokens = line.split(/\s+/);
  let idx = 0;
  const qtyParts: string[] = [];

  // grab numeric / fractional tokens
  while (idx < tokens.length && /^[\d/.]+$/.test(tokens[idx])) {
    qtyParts.push(tokens[idx++]);
  }
  const rawQty = qtyParts.join(' ');
  const qty = parseQty(rawQty);

  // skip unit
  if (idx < tokens.length && UNIT_RE.test(tokens[idx])) idx++;

  // rest is name (strip prep notes after comma)
  const namePart = tokens.slice(idx).join(' ').split(',')[0].trim();
  if (!namePart) return null;

  const qty4 = qty ? `${qty}` : '';
  // scale to 6: multiply numeric qty by 1.5
  let qty6 = '';
  const numQty = parseFloat(qty4);
  if (!isNaN(numQty)) {
    const q6 = numQty * 1.5;
    qty6 = q6 % 1 === 0 ? String(q6) : q6.toFixed(2).replace(/\.?0+$/, '');
  }

  return { name: namePart, qty_4_serving: qty4, qty_6_serving: qty6, store_section: guessSection(namePart) };
}

function parseTextRecipe(text: string): ImportResult {
  const lines = text.split('\n').map(l => l.trim()).filter(Boolean);

  // Name: first non-blank line (if it looks like a title, not an ingredient)
  let name = '';
  let nameIdx = 0;
  for (let i = 0; i < Math.min(5, lines.length); i++) {
    const l = lines[i];
    if (!/^\d/.test(l) && !l.includes(':') && l.length < 80) {
      name = l.replace(/^#+\s*/, '');
      nameIdx = i;
      break;
    }
  }

  // Times
  let prepMin = 0, cookMin = 0;
  const timeRe = /(prep|preparation)[:\s]+(\d+)\s*(?:min|minute)/i;
  const cookRe = /(cook|bake|roast)[:\s]+(\d+)\s*(?:min|minute)/i;
  const tmMatch = text.match(timeRe); if (tmMatch) prepMin = parseInt(tmMatch[2]);
  const ckMatch = text.match(cookRe);  if (ckMatch) cookMin = parseInt(ckMatch[2]);

  // Servings
  let servings = 4;
  const srvMatch = text.match(/serves?\s*:?\s*(\d+)/i) || text.match(/(\d+)\s*servings?/i);
  if (srvMatch) servings = parseInt(srvMatch[1]);

  // Ingredients: look for a section header then parse lines
  const ingHeaderRe = /^(ingredients?|ingredient list)[:\s]*$/i;
  const instrHeaderRe = /^(instructions?|directions?|method|steps?|preparation)[:\s]*$/i;

  let ingStart = -1, ingEnd = lines.length;
  for (let i = 0; i < lines.length; i++) {
    if (ingStart === -1 && ingHeaderRe.test(lines[i])) { ingStart = i + 1; continue; }
    if (ingStart !== -1 && instrHeaderRe.test(lines[i])) { ingEnd = i; break; }
  }

  // fallback: use all lines that start with a bullet or number+unit pattern
  const ingCandidates = ingStart !== -1
    ? lines.slice(ingStart, ingEnd)
    : lines.filter(l => /^[-•*·\d]/.test(l) && l.length < 120);

  const ingredients = ingCandidates
    .map(parseIngredientLine)
    .filter((ing): ing is IngredientDraft => Boolean(ing));

  // Instructions: text after the instruction header
  let instrLines: string[] = [];
  let instrStart = -1;
  for (let i = 0; i < lines.length; i++) {
    if (instrHeaderRe.test(lines[i])) { instrStart = i + 1; break; }
  }
  if (instrStart !== -1) instrLines = lines.slice(instrStart);

  const confidence = ingredients.length >= 3
    ? ingredients.length >= 6 ? 80 : 60
    : 35;
  const level = confidence >= 70 ? 'high' as const : confidence >= 50 ? 'medium' as const : 'low' as const;

  return {
    parsed: {
      name, category: 'Couples', session_fit: 'Session-Friendly',
      prep_time_min: prepMin, cook_time_min: cookMin, servings_base: servings,
      hold_quality: 4, reheat_quality: 4,
      key_overlap_ingredients: [], chef_notes: '',
      raw_instructions: instrLines.join('\n'),
      ingredients,
    },
    confidence,
    confidence_level: level,
  };
}

// ── Shared sub-components ─────────────────────────────────────────────────────

interface ConfidenceBannerProps {
  level: string | null;
  score: number | null;
  ingredientCount: number;
  source: string | null;
}

const ConfidenceBanner = ({ level, score, ingredientCount, source }: ConfidenceBannerProps) => {
  const configs: Record<string, { bg: string; text: string; icon: string; msg: string }> = {
    high:   { bg: 'bg-green-light border-green-mid',   text: 'text-green-dark', icon: '✓', msg: `Parsed — ${ingredientCount} ingredients found. Review and save.` },
    medium: { bg: 'bg-amber-50 border-amber-300',      text: 'text-amber-800',  icon: '◐', msg: 'Parsed with some gaps — review highlighted fields.' },
    low:    { bg: 'bg-red-50 border-red-200',           text: 'text-danger',     icon: '⚠', msg: 'Low confidence — fill in missing fields before saving.' },
  };
  const c = configs[level ?? ''] || configs.medium;
  return (
    <div className={`px-4 py-3 rounded-xl border ${c.bg} mb-4`}>
      <div className={`font-semibold text-sm ${c.text}`}>{c.icon} {c.msg}</div>
      <div className="text-xs text-gray-500 mt-0.5">
        Confidence: {score}/100
        {source && <span className="ml-2 opacity-70">· via {source === 'json_ld' ? 'structured data' : source === 'text_parser' ? 'text parser' : source}</span>}
      </div>
    </div>
  );
};

interface ReviewFormProps {
  form: ImportForm;
  setForm: Dispatch<SetStateAction<ImportForm>>;
  confidence: number | null;
  confidenceLevel: string | null;
  source: string | null;
  onSave: () => void;
  onSaveAnother: () => void;
  onDiscard: () => void;
  saving: boolean;
}

function ReviewForm({ form, setForm, confidence, confidenceLevel, source, onSave, onSaveAnother, onDiscard, saving }: ReviewFormProps) {
  const [overlapInput, setOverlapInput] = useState('');
  const set = <K extends keyof ImportForm>(field: K, val: ImportForm[K]) => setForm(prev => ({ ...prev, [field]: val }));
  const detectedAllergens = detectAllergens(form.ingredients);

  const updateIngredient = (idx: number, field: 'name' | 'qty_4_serving' | 'qty_6_serving' | 'store_section', val: string) => {
    setForm(prev => ({
      ...prev,
      ingredients: prev.ingredients.map((ing, i) => i === idx ? { ...ing, [field]: val } : ing),
    }));
  };

  const addIngredient = () => {
    setForm(prev => ({
      ...prev,
      ingredients: [...prev.ingredients, { name: '', qty_4_serving: '', qty_6_serving: '', store_section: 'Produce' }],
    }));
  };

  const removeIngredient = (idx: number) => {
    setForm(prev => ({ ...prev, ingredients: prev.ingredients.filter((_, i) => i !== idx) }));
  };

  return (
    <div>
      <ConfidenceBanner level={confidenceLevel} score={confidence} ingredientCount={form.ingredients.length} source={source} />

      {/* Name */}
      <div className="card mb-3">
        <label className="block text-xs text-gray-500 mb-1.5">Recipe Name</label>
        <input
          value={form.name}
          onChange={e => set('name', e.target.value)}
          placeholder="Recipe name..."
          style={{ fontFamily: '"Playfair Display", serif', fontSize: '18px', fontWeight: 600 }}
        />
      </div>

      {/* Category + Session Fit + Times */}
      <div className="card mb-3 space-y-3">
        <div className="flex gap-3">
          <div className="flex-1">
            <label className="block text-xs text-gray-500 mb-1">Category</label>
            <select value={form.category} onChange={e => set('category', e.target.value)}>
              {['Couples','Families','Plant-Based','Other'].map(c => <option key={c}>{c}</option>)}
            </select>
          </div>
          <div className="flex-1">
            <label className="block text-xs text-gray-500 mb-1">Session Fit</label>
            <select value={form.session_fit} onChange={e => set('session_fit', e.target.value)}>
              <option>Session-Friendly</option>
              <option>Long-Cook</option>
            </select>
          </div>
        </div>
        {form.session_fit === 'Long-Cook' && (
          <div className="text-xs text-amber-600 bg-amber-50 px-3 py-2 rounded-lg">⏱ Not recommended for standard 3-hour sessions</div>
        )}
        <div className="grid grid-cols-3 gap-2">
          <div>
            <label className="block text-xs text-gray-500 mb-1">Prep (min)</label>
            <input type="number" value={form.prep_time_min || ''} onChange={e => set('prep_time_min', parseInt(e.target.value) || 0)} />
          </div>
          <div>
            <label className="block text-xs text-gray-500 mb-1">Cook (min)</label>
            <input type="number" value={form.cook_time_min || ''} onChange={e => set('cook_time_min', parseInt(e.target.value) || 0)} />
          </div>
          <div>
            <label className="block text-xs text-gray-500 mb-1">Total</label>
            <div className="py-2.5 px-3 bg-gray-50 rounded-lg text-sm text-gray-600">
              {(parseInt(String(form.prep_time_min)) || 0) + (parseInt(String(form.cook_time_min)) || 0)} min
            </div>
          </div>
        </div>
      </div>

      {/* Quality ratings */}
      <div className="card mb-3">
        <div className="flex justify-between">
          <div>
            <div className="text-xs text-gray-500 mb-2">Hold Quality <span className="text-gray-400">(holds 4 days)</span></div>
            <StarRating value={form.hold_quality} onChange={v => set('hold_quality', v)} />
          </div>
          <div>
            <div className="text-xs text-gray-500 mb-2">Reheat Quality <span className="text-gray-400">(reheats well)</span></div>
            <StarRating value={form.reheat_quality} onChange={v => set('reheat_quality', v)} />
          </div>
        </div>
      </div>

      {/* Overlap ingredients */}
      <div className="card mb-3">
        <div className="section-header">Key Overlap Ingredients</div>
        <div className="flex flex-wrap gap-1.5 mb-2">
          {form.key_overlap_ingredients.map((tag, i) => (
            <span key={i} className="flex items-center gap-1 px-2.5 py-1 bg-green-light text-green-dark rounded-full text-xs font-medium">
              {tag}
              <button onClick={() => set('key_overlap_ingredients', form.key_overlap_ingredients.filter((_, j) => j !== i))} style={{ minHeight: 'auto' }}>×</button>
            </span>
          ))}
        </div>
        <input
          value={overlapInput}
          onChange={e => setOverlapInput(e.target.value)}
          onKeyDown={e => {
            if ((e.key === 'Enter' || e.key === ',') && overlapInput.trim()) {
              e.preventDefault();
              const val = overlapInput.trim().replace(/,$/, '');
              if (!form.key_overlap_ingredients.includes(val)) set('key_overlap_ingredients', [...form.key_overlap_ingredients, val]);
              setOverlapInput('');
            }
          }}
          placeholder="Type + Enter to add..."
        />
        <p className="text-xs text-gray-400 mt-1">Core pantry ingredients shared across dishes</p>
      </div>

      {/* Chef Notes */}
      <div className="card mb-3">
        <div className="section-header">Chef Notes</div>
        <textarea rows={3} value={form.chef_notes || ''} onChange={e => set('chef_notes', e.target.value)} />
      </div>

      {/* Allergen detection */}
      {detectedAllergens.length > 0 && (
        <div className="mb-3 px-4 py-3 bg-amber-50 border border-amber-200 rounded-xl">
          <div className="text-sm font-semibold text-amber-800 mb-1">⚠ Allergens detected: {detectedAllergens.join(', ')}</div>
          <p className="text-xs text-amber-700">Confirm these are correct — they affect client allergen filtering.</p>
        </div>
      )}

      {/* Ingredients */}
      <div className="card mb-3">
        <div className="section-header">Ingredients</div>
        <div className="overflow-x-auto -mx-2 px-2">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-gray-200">
                <th className="text-left py-2 text-gray-500 font-medium">Ingredient</th>
                <th className="text-center py-2 text-gray-500 font-medium">4 srv</th>
                <th className="text-center py-2 text-gray-500 font-medium">6 srv</th>
                <th className="w-6" />
              </tr>
            </thead>
            <tbody>
              {form.ingredients.map((ing, i) => {
                const missing = !ing.qty_4_serving || !ing.qty_6_serving;
                return (
                  <tr key={i} className={`border-b border-gray-100 last:border-0 ${missing ? 'border-l-2 border-l-amber-400' : ''}`}>
                    <td className="py-1.5 pr-1">
                      <input value={ing.name} onChange={e => updateIngredient(i, 'name', e.target.value)} style={{ minHeight: '32px', fontSize: '12px', padding: '4px 6px' }} />
                    </td>
                    <td className="py-1.5 px-1">
                      <input value={ing.qty_4_serving || ''} onChange={e => updateIngredient(i, 'qty_4_serving', e.target.value)} style={{ minHeight: '32px', fontSize: '12px', padding: '4px 6px', width: '70px', textAlign: 'center' }} />
                    </td>
                    <td className="py-1.5 px-1">
                      <input value={ing.qty_6_serving || ''} onChange={e => updateIngredient(i, 'qty_6_serving', e.target.value)} style={{ minHeight: '32px', fontSize: '12px', padding: '4px 6px', width: '70px', textAlign: 'center' }} />
                    </td>
                    <td className="py-1.5">
                      <button onClick={() => removeIngredient(i)} className="text-danger" style={{ minHeight: 'auto' }}>×</button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <button onClick={addIngredient} className="mt-3 text-sm text-navy-mid font-medium" style={{ minHeight: 'auto' }}>
            + Add Ingredient
          </button>
        </div>
      </div>

      {/* Cooking method */}
      <div className="card mb-4">
        <div className="text-xs text-gray-500 mb-1.5">Cooking Method <span className="text-gray-400">— for your reference, not shown to clients</span></div>
        <textarea rows={5} value={form.raw_instructions || ''} onChange={e => set('raw_instructions', e.target.value)} placeholder="Cooking instructions..." />
      </div>

      {/* Save actions */}
      <div className="space-y-2">
        <button onClick={onSave} disabled={saving} className="w-full py-3.5 bg-navy text-white rounded-xl font-semibold disabled:opacity-60">
          {saving ? 'Saving...' : 'Save Recipe'}
        </button>
        <button onClick={onSaveAnother} disabled={saving} className="w-full py-3 border border-navy text-navy rounded-xl font-medium text-sm disabled:opacity-60">
          Save & Import Another
        </button>
        <button onClick={onDiscard} className="w-full py-2.5 text-gray-500 text-sm" style={{ minHeight: 'auto' }}>
          Discard
        </button>
      </div>
    </div>
  );
}

// ── Tabs ──────────────────────────────────────────────────────────────────────

const EMPTY_FORM: ImportForm = {
  name: '', category: 'Couples', session_fit: 'Session-Friendly',
  prep_time_min: '', cook_time_min: '', servings_base: 4,
  hold_quality: 4, reheat_quality: 4,
  key_overlap_ingredients: [], chef_notes: '', raw_instructions: '',
  ingredients: [],
};

function UrlTab({ onParsed }: { onParsed: (result: ImportResult) => void }) {
  const { show } = useToast();
  const [url, setUrl]         = useState('');
  const [loading, setLoading] = useState(false);

  const handleFetch = async () => {
    const trimmed = url.trim();
    if (!trimmed) return;
    setLoading(true);
    try {
      const result = await apiCall<RecipeImportResponse>('/api/recipes/import/url', 'POST', { url: trimmed });
      onParsed(result);
    } catch (err) {
      show(errorMessage(err) || 'Could not fetch recipe — make sure the URL is a recipe page.', 'error', 5000);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="card">
      <p className="text-sm text-gray-500 mb-3">
        Paste a link to any recipe page. Works with most major food sites (AllRecipes, NYT Cooking, Serious Eats, Food Network, Epicurious, etc.)
      </p>
      <input
        type="url"
        value={url}
        onChange={e => setUrl(e.target.value)}
        onKeyDown={e => e.key === 'Enter' && handleFetch()}
        placeholder="https://www.allrecipes.com/recipe/..."
        disabled={loading}
        className="mb-3"
      />
      <button
        onClick={handleFetch}
        disabled={!url.trim() || loading}
        className="w-full py-3 bg-navy text-white rounded-xl font-semibold disabled:opacity-40"
      >
        {loading ? (
          <span className="flex items-center justify-center gap-2">
            <span className="animate-spin">⟳</span> Fetching recipe...
          </span>
        ) : 'Import from URL →'}
      </button>
      <p className="text-xs text-gray-400 mt-2 text-center">No AI — reads the site's structured recipe data directly</p>
    </div>
  );
}

function TextTab({ onParsed }: { onParsed: (result: ImportResult) => void }) {
  const { show } = useToast();
  const [text, setText]   = useState('');

  const PLACEHOLDER = `Paste or type your recipe here — any format works.

Examples:
· A full recipe copied from a food site
· Ingredients list + instructions
· "3 lbs chicken thighs, white wine, mushrooms, garlic — braise 1 hour, serves 4"`;

  const handleParse = () => {
    if (text.trim().length < 20) {
      show('Paste more text to parse', 'error');
      return;
    }
    const result = parseTextRecipe(text);
    if (!result.parsed.ingredients.length) {
      show('Could not find ingredients — check the format and try again', 'error', 4000);
      return;
    }
    onParsed({ ...result, source: 'text_parser' });
  };

  return (
    <div className="card">
      <p className="text-sm text-gray-500 mb-3">
        Paste recipe text from a website, cookbook, or type from memory. No AI — parsed locally in your browser.
      </p>
      <div className="relative">
        <textarea
          rows={8}
          value={text}
          onChange={e => setText(e.target.value)}
          placeholder={PLACEHOLDER}
          className="resize-none pr-8"
        />
        {text && (
          <button
            onClick={() => setText('')}
            className="absolute top-2 right-2 text-gray-400 hover:text-gray-600 text-lg font-bold"
            style={{ minHeight: 'auto' }}
          >×</button>
        )}
        <div className="text-right text-xs text-gray-400 mt-1">{text.length} characters</div>
      </div>
      <button
        onClick={handleParse}
        disabled={text.length < 20}
        className="w-full py-3 mt-3 bg-navy text-white rounded-xl font-semibold disabled:opacity-40"
      >
        Parse Recipe →
      </button>
      <p className="text-xs text-gray-400 mt-2 text-center">Works best with a standard ingredient list followed by instructions</p>
    </div>
  );
}

function ManualTab() {
  const navigate = useNavigate();
  return (
    <div className="card text-center py-8">
      <div className="text-4xl mb-3">📝</div>
      <p className="font-semibold text-navy mb-1">Enter recipe manually</p>
      <p className="text-sm text-gray-500 mb-5">Build a recipe from scratch with full control over every field.</p>
      <button
        onClick={() => navigate('/recipes/new')}
        className="px-6 py-3 bg-navy text-white rounded-xl font-semibold"
      >
        Create New Recipe →
      </button>
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────

const TABS = [
  { id: 'url',    label: '🔗 From URL' },
  { id: 'text',   label: '📋 Paste Text' },
  { id: 'manual', label: '✏️ Manual' },
];

export default function RecipeImport() {
  const navigate = useNavigate();
  const { show }  = useToast();
  const [tab, setTab]             = useState('url');
  const [parsed, setParsed]       = useState<ImportResult['parsed'] | null>(null);
  const [confidence, setConf]     = useState<number | null>(null);
  const [confLevel, setConfLevel] = useState<string | null>(null);
  const [source, setSource]       = useState<string | null>(null);
  const [form, setForm]           = useState<ImportForm>(EMPTY_FORM);
  const [saving, setSaving]       = useState(false);

  const handleParsed = (result: ImportResult) => {
    setParsed(result.parsed);
    setConf(result.confidence);
    setConfLevel(result.confidence_level);
    setSource(result.source || null);
    setForm({
      name:                    result.parsed.name               || '',
      category:                result.parsed.category           || 'Couples',
      session_fit:             result.parsed.session_fit        || 'Session-Friendly',
      prep_time_min:           result.parsed.prep_time_min      || '',
      cook_time_min:           result.parsed.cook_time_min      || '',
      servings_base:           result.parsed.servings_base      || 4,
      hold_quality:            result.parsed.hold_quality       || 4,
      reheat_quality:          result.parsed.reheat_quality     || 4,
      key_overlap_ingredients: result.parsed.key_overlap_ingredients || [],
      chef_notes:              result.parsed.chef_notes         || '',
      raw_instructions:        result.parsed.raw_instructions   || '',
      ingredients:             result.parsed.ingredients        || [],
    });
  };

  const handleSave = async (addAnother = false) => {
    if (!form.name.trim()) { show('Recipe name is required', 'error'); return; }
    setSaving(true);
    try {
      const saved = await apiCall<RecipeDetail>('/api/recipes', 'POST', { ...form, import_source: source || 'import' });
      show('Recipe saved', 'success');
      if (addAnother) {
        setParsed(null);
        setForm(EMPTY_FORM);
      } else {
        navigate(`/recipes/${saved.id}`);
      }
    } catch {
      show('Could not save recipe', 'error');
    } finally {
      setSaving(false);
    }
  };

  const handleDiscard = () => {
    if (window.confirm('Discard this recipe?')) {
      setParsed(null);
      setForm(EMPTY_FORM);
    }
  };

  return (
    <div className="page">
      <PageHeader title="Import a Recipe" back="/recipes" />

      <div className="px-4 pt-4 pb-28">
        {!parsed ? (
          <>
            {/* Tab bar */}
            <div className="flex rounded-xl bg-gray-100 p-1 mb-4 gap-1">
              {TABS.map(t => (
                <button
                  key={t.id}
                  onClick={() => setTab(t.id)}
                  className={`flex-1 py-2 text-xs font-semibold rounded-lg transition-colors ${
                    tab === t.id ? 'bg-white text-navy shadow-xs' : 'text-gray-500'
                  }`}
                  style={{ minHeight: '36px' }}
                >
                  {t.label}
                </button>
              ))}
            </div>

            {tab === 'url'    && <UrlTab  onParsed={handleParsed} />}
            {tab === 'text'   && <TextTab onParsed={handleParsed} />}
            {tab === 'manual' && <ManualTab />}
          </>
        ) : (
          <ReviewForm
            form={form}
            setForm={setForm}
            confidence={confidence}
            confidenceLevel={confLevel}
            source={source}
            onSave={() => handleSave(false)}
            onSaveAnother={() => handleSave(true)}
            onDiscard={handleDiscard}
            saving={saving}
          />
        )}
      </div>
    </div>
  );
}
