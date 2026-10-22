# Platebook — UI Style & Branding Guide

## Brand Identity

| | |
|---|---|
| **Product name** | Platebook (`APP_NAME` in `client/src/constants.ts`) |
| **Tagline** | The all-in-one kitchen office for home & personal chefs (`APP_TAGLINE`) |
| **Palette name** | Fresh Harvest |
| **Aesthetic** | Warm, organic, premium — farmers-market meets fine dining |

Each chef's **business identity** (business name, logo, chef name, website, email) is per-account data from Settings / onboarding. It is shown on that chef's dashboard, booking page, emails, PDFs and invoices. The product name appears only on platform pages (welcome, sign-in, onboarding) and as a fallback when a chef hasn't set a business name.

---

## Color Palette

### Core Tokens

| Token | Hex | Tailwind class | Role |
|---|---|---|---|
| `navy` / `herb` | `#5E7F57` | `bg-navy`, `text-navy` | Primary — page headers, main buttons, FAB, active nav |
| `navy-mid` / `terracotta` | `#C56A4A` | `bg-navy-mid`, `text-navy-mid` | Secondary actions, focus ring accent |
| `green-dark` | `#3A5C32` | `bg-green-dark`, `text-green-dark` | Section headers, active sidebar links, success states |
| `green-mid` | `#5E7F57` | `bg-green-mid`, `text-green-mid` | Active tab indicators, bottom nav active |
| `green-light` / `sage` | `#EEF3EA` | `bg-green-light`, `bg-sage` | Chip/tag backgrounds, highlight areas, light button fills |
| `blue-light` | `#EEF3EA` | `bg-blue-light` | Hover backgrounds on white cards (same as sage — legacy alias) |
| `warm-white` / `ivory` | `#FBF8F1` | `bg-warm-white`, `bg-ivory` | Page background |
| `amber` / `honey` | `#D8A13C` | `bg-amber`, `text-amber` | Warnings, "Other" category accent |
| `danger` / `berry` | `#A94442` | `bg-danger`, `text-danger` | Errors, destructive actions, Cancelled status |
| `olive` | `#28302A` | `text-olive` | Primary body text |

### Status Badge Colors

Applied via `STATUS_COLORS` from `constants.ts`:

| Status | Classes |
|---|---|
| Pending | `bg-gray-100 text-gray-600` |
| Confirmed | `bg-blue-100 text-blue-700` |
| Completed | `bg-green-light text-green-dark` |
| Cancelled | `bg-red-100 text-danger` |

### Recipe Category Colors (timeline / bar chart)

| Category | Hex |
|---|---|
| Couples | `#5E7F57` (Herb Green) |
| Families | `#3A5C32` (Deep Herb) |
| Plant-Based | `#C56A4A` (Terracotta) |
| Other | `#D8A13C` (Honey) |

### Feasibility Badge Colors

| Level | Classes |
|---|---|
| GREEN | `bg-green-light text-green-dark` |
| AMBER | `bg-amber-50 text-amber-700` |
| RED | `bg-red-50 text-danger` |
| NONE | `bg-gray-100 text-gray-500` |

---

## Typography

### Fonts

| Role | Font | Weights | Source |
|---|---|---|---|
| **Display / headings** | Playfair Display | 400, 600, 700 (+ italic 400) | Google Fonts |
| **Body / UI** | DM Sans | 300, 400, 500, 600 | Google Fonts |
| **Invoice preview** | Inter / Aptos / system-ui | system fallback | — |

### Tailwind Font Classes

```
font-display   →  Playfair Display, Georgia, serif
font-sans      →  DM Sans, system-ui, sans-serif
```

### Usage Rules

- **Page titles, section headers, brand wordmark** → `font-display`
- **All UI text, labels, buttons, inputs** → `font-sans` (default)
- **Section headers** use `.section-header` class: Playfair Display, `#3A5C32`, `1.1rem`, `font-semibold`
- **Invoice PDFs** use Inter/system-ui for clean print rendering

---

## Spacing & Layout

### Page Background
`#FBF8F1` (Ivory / `warm-white`) — applied globally to `body` and `.page`

### Responsive Shell

| Breakpoint | Layout |
|---|---|
| `< 768px` (mobile) | Single column; bottom nav (60px); page scrolls under nav |
| `≥ 768px` (tablet/desktop) | Sidebar (220px fixed left) + content column; bottom nav hidden |
| `≥ 1200px` (wide) | Sidebar expands to 240px |

### Sidebar (desktop)
- Width: `220px` (`240px` at 1200px+)
- Background: `white`, `border-right: 1px solid #E5E7EB`
- Brand block: Playfair Display wordmark in `green-dark`, "Private Chef Services" subtitle in `#9CA3AF`
- Nav items: `40px` min-height, `8px` border-radius
- Active item: `bg-green-light`, `text-green-dark`, `font-semibold`
- Inactive item: `text-gray-500`, hover `bg-gray-50`

### Bottom Nav (mobile only)
- Height: `60px` + `env(safe-area-inset-bottom)`
- Active item: `green-mid` color + `2px` top border
- Inactive: `#9CA3AF`

### Fixed Bottom Action Bars
Bars that sit above the bottom nav on session/invoice pages:
```
fixed bottom-[60px] left-0 right-0 ... md:bottom-0 md:left-[220px]
```
The `md:left-[220px]` offset is **required** on every fixed bar to clear the sidebar on desktop.

---

## Core Components

### `.card`
```css
background: white;
border-radius: 12px;
box-shadow: 0 2px 8px rgba(0,0,0,0.08);
padding: 16px;
```
Hover variant: `hover:shadow-card-hover` → `0 4px 16px rgba(0,0,0,0.12)`

### `.section-header`
```css
font-family: Playfair Display, serif;
color: #3A5C32;
font-size: 1.1rem;
font-weight: 600;
margin-bottom: 12px;
```

### `.chip` — filter / tag pills
```css
padding: 4px 12px;
border-radius: 999px;
font-size: 13px;
font-weight: 500;
min-height: 32px;
border: 1.5px solid transparent;
```
- Active: `bg-navy text-white border-navy`
- Inactive: `bg-white text-gray-700 border-gray-300`

### Inputs / Selects / Textareas
```css
min-height: 44px;
padding: 10px 14px;
border: 1.5px solid #D1D5DB;
border-radius: 8px;
font-family: DM Sans;
font-size: 15px;
```
Focus: `border-color: #C56A4A` + `box-shadow: 0 0 0 3px rgba(27,79,138,0.1)`

### FAB (Floating Action Button)
```css
position: fixed;
bottom: calc(80px + env(safe-area-inset-bottom));   /* mobile */
bottom: 24px;                                         /* desktop */
right: 20px;
width: 56px; height: 56px;
border-radius: 50%;
background: #5E7F57;  /* navy */
box-shadow: 0 4px 12px rgba(13,43,75,0.4);
font-size: 24px;
```
Press state: `scale(0.93)`

### Modal / Bottom Sheet
- Mobile: slides up from bottom, `border-radius: 20px 20px 0 0`
- Desktop (`≥ 640px`): centered dialog, `border-radius: 16px`, `max-width: 480px`
- Overlay: `rgba(0,0,0,0.5)`

### Skeleton Loader
```css
background: #E5E7EB;
border-radius: 6px;
animation: skeleton-pulse 1.5s ease-in-out infinite;  /* opacity 1 → 0.4 */
```

### Toast Notifications
Slides up from bottom: `translateY(100px) → translateY(0)` over `0.3s ease-out`

---

## Buttons

| Variant | Classes |
|---|---|
| Primary | `bg-navy text-white rounded-xl font-semibold py-3.5` |
| Success / email | `bg-green-dark text-white rounded-xl font-semibold py-3.5` |
| Ghost / secondary | `bg-warm-white border border-gray-200 text-navy rounded-xl` |
| Destructive | `border border-danger text-danger rounded-xl` |
| Icon button | `p-1.5 rounded-lg text-gray-400 hover:text-navy hover:bg-blue-light` |

**Tap target rule:** All `button`, `a`, and `[role="button"]` elements have `min-height: 44px` globally enforced (Apple HIG standard).

---

## Email Templates

All transactional emails share the same visual language as the app:

| Element | Value |
|---|---|
| Header background | `#5E7F57` (Herb Green) |
| Header wordmark | Playfair Display, white, `22px` |
| Header subtext | `#C8DFC4`, `13px` |
| Body background | `#FBF8F1` (Ivory) |
| Card background | `white`, `border-radius: 16px` |
| Info box | `#EEF3EA` (Pale Sage), `border-radius: 10px` |
| CTA button | `#5E7F57`, white text, `border-radius: 10px`, `14px 28px` padding |
| Footer | `#F9F7F2`, `#9CA3AF` text, `11px` |
| Footer text | `<business name> · <website>` (from the chef's settings) |
| From address | Platform `EMAIL_FROM`; **Reply-To** is the chef's business email |

---

## Invoice Preview (PDF / Print)

The invoice preview card uses a distinct style suited for client-facing documents:

| Element | Value |
|---|---|
| Font stack | Inter, Aptos, system-ui |
| Header gradient | `linear-gradient(135deg, #1B2B4B 0%, #2D6A4F 100%)` |
| Info accent | `#EEF3EA` boxes, `#3A5C32` labels |
| Filename format | `invoice-{number}.pdf` |
| Print styles | Shadow/border removed, sidebar/nav hidden, card fills page |

---

## Animations

| Name | Use | Spec |
|---|---|---|
| `skeleton-pulse` | Loading placeholders | `opacity 1↔0.4`, `1.5s ease-in-out infinite` |
| `toast-slide-up` | Toast notifications | `translateY(100px)→0`, `0.3s ease-out` |
| `sheet-up` | Bottom sheet modals | `translateY(100%)→0`, `0.3s cubic-bezier(0.34,1.56,0.64,1)` |
| `animate-spin` | Inline loading spinners | Tailwind built-in |

---

## Iconography

All icons are inline SVG — no icon library dependency. Common specs:

| Context | Size |
|---|---|
| Nav icons | `20×20` |
| Action bar / button icons | `18×18` |
| Card inline icons | `14–16×14–16` |
| Badge icons (envelope, check) | `11–12×11–12` |

Stroke weight: `strokeWidth="2"` standard, `strokeWidth="2.2"` for small icons that need visual weight.

---

## Voice & Copy Tone

- **Warm and professional** — never corporate or cold
- Chef-first: the UI speaks to Ali, not a generic "user"
- Client-facing copy (emails, intake form) is friendly and reassuring
- Error messages are practical: tell the user what to do next, not just what went wrong
  - ✓ `"No email on file — add one in the client profile"`
  - ✗ `"Error: client_email is null"`
