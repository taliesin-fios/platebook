# Platebook — Instruction Manual

**Version 1.0.0 · Built for personal chef businesses**

---

## Table of Contents

1. [What Is Platebook?](#1-what-is-platebook)
   - [Creating Your Account & Onboarding](#creating-your-account--onboarding)
2. [Getting Started (running it yourself)](#2-getting-started-running-it-yourself)
   - [Windows 11 and macOS](#windows-11-and-macos)
   - [iPhone / Mobile Access](#iphone--mobile-access)
3. [Application Overview](#3-application-overview)
4. [Section-by-Section Guide](#4-section-by-section-guide)
   - [Dashboard](#dashboard)
   - [Clients](#clients)
   - [Recipes](#recipes)
   - [Sessions](#sessions)
   - [Templates](#templates)
   - [Costs](#costs)
   - [Settings](#settings)
5. [Core Workflows](#5-core-workflows)
   - [Planning a New Session](#planning-a-new-session)
   - [Importing a Recipe with AI](#importing-a-recipe-with-ai)
   - [Managing the Grocery List](#managing-the-grocery-list)
   - [Generating Client PDFs](#generating-client-pdfs)
   - [Booking Inquiries](#booking-inquiries)
   - [Tracking Profitability](#tracking-profitability)
6. [AI Features](#6-ai-features)
7. [Understanding Session Feasibility](#7-understanding-session-feasibility)
8. [Allergen Safety System](#8-allergen-safety-system)
9. [Customizing Platebook](#9-customizing-platebook)
   - [Branding & Business Info](#branding--business-info)
   - [Adding Your Own Recipes](#adding-your-own-recipes)
   - [Modifying the Color Theme](#modifying-the-color-theme)
   - [Changing Allergen Keywords](#changing-allergen-keywords)
   - [Adjusting Feasibility Thresholds](#adjusting-feasibility-thresholds)
   - [Editing Store Sections](#editing-store-sections)
   - [Customizing Reheat Instructions](#customizing-reheat-instructions)
   - [Modifying Session Templates](#modifying-session-templates)
   - [Database Customization](#database-customization)
10. [Data Management & Backup](#10-data-management--backup)
11. [Keyboard Shortcuts & Tips](#11-keyboard-shortcuts--tips)
12. [Troubleshooting](#12-troubleshooting)

---

## 1. What Is Platebook?

Platebook is a **business management tool for home and personal chefs**: chefs who cook in clients’ homes, run weekly meal prep, or cater dinner parties. It replaces spreadsheets, paper notes, and memory with a single mobile-first web app. Each chef has their own account, and every account's clients, recipes and invoices are private to it.

**What it does:**

| Area | What you can do |
|---|---|
| **Clients** | Store dietary profiles, allergens, kitchen info, and service preferences for every household you serve |
| **Recipes** | Maintain a searchable library of 35+ dishes with ingredients, timing, and quality ratings |
| **Sessions** | Plan weekly prep sessions step-by-step with a 5-step wizard |
| **Grocery Lists** | Auto-generate consolidated shopping lists grouped by store section |
| **Prep Timeline** | Visualize oven conflicts and cook sequencing before you arrive |
| **Costs** | Track service fees, ingredient costs, and margin per session and per client |
| **PDFs** | Generate professional client leave-behind sheets and grocery lists |
| **AI** | Use Claude AI to import recipes from text and get dish suggestions tailored to each client |
| **Booking** | Share your own booking link (`/book/your-business`); new client requests land in your Intake inbox |
| **Invoices** | Build invoices from sessions, email them, and track deposits and payments |

**Who it's for:** Solo home and personal chefs serving multiple households.

### Creating Your Account & Onboarding

1. Open the app and choose **Create your account**. Enter your business name, email and a password (at least 10 characters).
2. The **setup wizard** walks you through five short steps:
   - **Your business**: name, your name, phone, city, website and logo. These appear on your booking page, PDFs and invoices.
   - **How you work**: the services you offer, plus default session length, servings and service fee.
   - **Booking & payments**: deposit amount ($0 for none), cancellation notice, payment methods you accept, and the booking terms clients agree to. Suggested terms are written for you and update as you change the deposit.
   - **Head start**: optionally load the starter recipe library (20 recipes and 4 menu templates) and a sample client to practise with.
   - **Ready**: copy your booking link and jump to your first client, recipe or session.
3. You can choose **Finish later** at any point. Everything in the wizard can be changed in **Settings**.

**Signing in later:** use your email and password, or "Email me a sign-in link instead". If you forget your password, use **Forgot password?** on the sign-in page.

---

## 2. Getting Started (running it yourself)

This section is for running your own copy of the app (developers / self-hosting). If you're using a hosted copy, skip to [Creating Your Account & Onboarding](#creating-your-account--onboarding).

### Windows 11 and macOS

#### Prerequisites
- **Node.js 20** ([nodejs.org](https://nodejs.org), or `brew install node` on macOS)
- **Rust** ([rustup.rs](https://rustup.rs)). On Windows without Visual Studio, use the GNU toolchain and MinGW (`scoop install mingw`); see `server-rs/README.md`.
- **PostgreSQL 14 or newer** installed locally, **or** Docker Desktop (the setup step can run Postgres in a container)

#### First-Time Setup

1. Open a terminal in the project folder and install the dependencies:
   ```
   npm run install:all
   ```
2. Create the database and the server settings file (`server-rs/.env`):
   ```
   npm run db:setup
   ```
3. (Optional) For AI features, add your Anthropic API key to `server-rs/.env`:
   ```
   ANTHROPIC_API_KEY=sk-ant-your-key-here
   ```
4. Start the API server and the app together:
   ```
   npm run dev
   ```
   In Claude Code, you can instead start "Platebook — API (Rust)" and "Platebook — Frontend (Vite)" from the launch menu.
5. Open your browser at `http://localhost:5174`.

#### Stopping the Servers
Press `Ctrl+C` in the terminal.

#### Starting Automatically on Login
- **Windows:** create a `.bat` file containing `start "Platebook" cmd /k "cd /d <project folder> && npm run dev"`, then copy it into the folder that `Win+R` → `shell:startup` opens.
- **macOS:** create `~/start-platebook.sh` containing `osascript -e 'tell application "Terminal" to do script "cd /path/to/project && npm run dev"'`, make it executable (`chmod +x`), and add it to **System Settings → General → Login Items**.

#### macOS Firewall Note
If you plan to access Platebook from your iPhone, macOS may prompt you to allow the server through the firewall. Click **Allow**.

---

### iPhone / Mobile Access

Platebook is designed mobile-first — it works beautifully on iPhone Safari.

#### Requirements
- Your computer (running Platebook) and iPhone must be on the **same Wi-Fi network**
- The frontend server must be running with `host: true` in `vite.config.ts` (already configured)

#### Steps

1. Find your computer's local IP address:
   - **Windows**: Open Command Prompt → type `ipconfig` → look for "IPv4 Address" (e.g., `192.168.50.172`)
   - **macOS**: System Settings → Wi-Fi → Details → IP Address

2. On Windows, allow the port through the firewall (run once in an admin terminal):
   ```
   netsh advfirewall firewall add rule name="Vite Dev" dir=in action=allow protocol=TCP localport=5174
   ```
   On macOS, accept the firewall prompt when it appears.

3. On your iPhone, open Safari and go to:
   ```
   http://YOUR_IP_ADDRESS:5174
   ```
   Example: `http://192.168.50.172:5174`

4. **Add to Home Screen** for an app-like experience:
   - In Safari, tap the Share button (box with arrow)
   - Tap "Add to Home Screen"
   - Name it "Platebook" → tap Add
   - It will now appear as an icon on your home screen

#### iPhone Tips
- The app is optimized for iPhone screen sizes (375px width)
- Bottom navigation tabs are thumb-friendly
- Swipe left on recipe or client cards to reveal Edit and Delete buttons
- The app does not require internet — it only needs your computer's Wi-Fi

---

## 3. Application Overview

Platebook has **7 sections** accessed via the bottom navigation bar:

```
[ Dashboard ]  [ Clients ]  [ Sessions ]  [ Recipes ]  [ Costs ]
```

Settings are accessed via the gear icon on the Dashboard.

### Navigation Map
```
Dashboard ──► New Session (wizard)
           ──► New Client
           ──► Import Recipe
           ──► Templates

Clients   ──► Client Profile (view/edit)
           ──► New Client Form (4 sections)

Recipes   ──► Recipe Detail (view/edit)
           ──► New Recipe (manual)
           ──► Import Recipe (AI)

Sessions  ──► Session Detail (5 tabs: Overview, Grocery, Timeline, Costing, Delivery)
           ──► New Session (5-step wizard)

Templates ──► Use Template (launches session wizard pre-filled)

Costs     ──► Session cost table (inline edit)
           ──► Per-client profitability
           ──► 30-day revenue chart

Settings  ──► Business info, defaults, AI config, data export
```

---

## 4. Section-by-Section Guide

### Dashboard

Your home screen. Shows at a glance:

- **Sessions this week** — upcoming confirmed/pending sessions
- **Active Clients** — total households in your roster
- **Collected (MTD)** — invoices paid so far this month, with what's still outstanding and how many invoices are overdue. An arrow compares it with the same days of last month (the 1st up to today's date): **▲ 12% vs last month** means you've collected 12% more by this point than last month. There's no arrow when nothing was collected over those days last month.
- **Avg Margin** — average profitability across this month's sessions

**Quick Action buttons:**
- **+ New Session** — jump straight to the session wizard
- **👤 New Client** — open the client form
- **↓ Import Recipe** — go to AI recipe import
- **▤ Templates** — view saved menu templates

The **Next Session** card shows your next upcoming session. Below that is a list of recent sessions.

---

### Clients

A searchable list of all your client households.

**Client cards show:**
- Household name, type (Couple, Small Family, etc.)
- Serving size
- Last session date
- Allergen icons (color-coded dots)

**Actions:**
- **Tap a card** → view full client profile
- **Swipe left** on a card → reveal Edit and Delete buttons
- **+ FAB button** (bottom right) → add a new client

#### Client Form (4 Sections)

When adding or editing a client, the form is divided into collapsible sections — tap a section header to expand it:

**A — Personal Info**
- Name (required), phone, email, address
- Household type, number of adults and children
- Default serving size (4 or 6)

**B — Dietary Profile**
- **Allergens**: tap chips to select (Dairy, Gluten, Eggs, Tree Nuts, Peanuts, Fish, Shellfish, Soy, Sesame)
- **Dietary Styles**: Omnivore, Pescatarian, Vegan, Gluten-Free, Low-Carb, Mediterranean, etc.
- **Liked Proteins**: Chicken, Beef, Pork, Fish, Shrimp, Plant-Based Only, No Red Meat
- **Disliked Foods**: free text (e.g., "no cilantro, no mushrooms")
- **Spice Tolerance**: Mild, Medium, Bold, No Preference
- **Chef Notes**: anything else worth remembering

**C — Kitchen Info**
- Oven available (yes/no)
- Stovetop type (Gas, Electric, Induction)
- Available equipment (Stand Mixer, Dutch Oven, Instant Pot, etc.)
- Whether the client provides containers
- Parking notes

**D — Service Preferences**
- Preferred day and time
- Grocery handling (Chef Shops, Client Shops, Split)

---

### Recipes

Your recipe library with 35 pre-loaded dishes.

**Filtering and searching:**
- **Search bar** — filters by recipe name as you type
- **Category tabs**: All · Couples · Families · Plant-Based · Other
- **Session Fit toggles**: Session-Friendly · Long-Cook (toggle on/off)

**Recipe cards show:**
- Recipe name
- Category badge (navy = Couples, green = Families, teal = Plant-Based, purple = Other)
- Session fit badge (green ✓ = Session-Friendly, amber ⏱ = Long-Cook)
- Total time (prep + cook minutes)
- Hold quality (star rating)

**Actions:**
- **Tap a card** → view full recipe detail
- **Swipe left** → reveal Edit and Delete
- **+ FAB button** → choose Import Recipe (AI) or Add Manually

#### Recipe Detail

Each recipe shows:
- Category and session fit badges
- Prep time, cook time, total time, serves
- Hold quality and reheat quality stars
- Key overlap ingredients (green chips — ingredients shared with other dishes)
- Allergen warning banner if any ingredients trigger client allergens
- Full ingredients table with quantities for 4 and 6 servings and store section
- Chef notes
- Collapsible cooking method/instructions

---

### Sessions

The **Sessions** page shows upcoming and past sessions.

**Upcoming sessions** — sorted by date, status is Pending or Confirmed.
**Past sessions** — Completed or Cancelled.

Each session card shows: client name, date, time, status badge, dish count, service fee, margin %.

**Tap a session** to open the full detail view with 5 tabs:

#### Tab 1: Overview
- Session summary (date, time, client, serving size, status)
- Session feasibility indicator (GREEN / AMBER / RED)
- Full dish list with hold and reheat quality stars
- Chef notes
- Action buttons: Mark Complete, Generate PDFs, Edit, Delete

#### Tab 2: Grocery
- All ingredients aggregated from all dishes, scaled to your serving size
- Grouped by store section (Produce, Meat & Seafood, Dairy & Eggs, etc.)
- Tap any item to check it off as you shop
- Section-level "mark all" and "uncheck all" buttons
- **Regenerate** button if you change the dish list

#### Tab 3: Timeline
- Shows the planned start time for each dish
- Flags oven conflicts if multiple dishes need the oven at the same time
- Based on your session start time and each dish's prep/cook sequence

#### Tab 4: Costing
- Service fee, ingredient cost, gross margin, margin %
- Click any value to edit it inline
- Margin color: green (≥60%), amber (≥40%), red (<40%)

#### Tab 5: Delivery
- Reheat instructions per dish (auto-selected based on dish name)
- Storage guidance for the client
- PDF export buttons

---

### Templates

Saved menus you can reuse across sessions. If you loaded the starter library during onboarding, four templates come with it:

| Template | Dishes | Shared Ingredients |
|---|---|---|
| Weeknight Starter Trio | Lemon-Herb Roast Chicken Thighs, Chickpea and Spinach Coconut Curry, Turkey Meatballs in Marinara | garlic, pantry staples |
| Family Favorites | Beef and Vegetable Stew, Chicken and Rice Casserole, Black Bean and Sweet Potato Enchiladas | onion, cheese, stock |
| Date-Night Couples | Braised Short Ribs, Baked Salmon with Dill Yogurt Sauce, Mushroom Barley Risotto | thyme, wine, stock |
| Meatless Week | Red Lentil Soup, Roasted Vegetable Lasagna, Mediterranean Quinoa Salad | lemon, garlic, olive oil |

**To use a template:** tap "Use Template" → the session wizard opens with those dishes pre-selected.

**To create a template:** in step 3 of the session wizard, after selecting your dishes, tap "Save Current Menu as Template" and give it a name.

---

### Costs

Financial tracking and analysis across all sessions.

**Summary cards (month-to-date):**
- Revenue (sum of service fees)
- Ingredient Cost (sum of ingredient costs)
- Gross Margin ($)
- Margin % (profitability)

**30-day rolling chart:** a bar chart showing daily revenue vs cost.

**Sessions table:** every session with date, client, fee (editable), cost (editable), and margin %. Click a row to go to that session's detail.

**Export CSV:** download all session data as a spreadsheet.

**Per-client profitability:** see which clients generate the best margins.

---

### Settings

Configure Platebook to match your business.

**Business Info** — your business name, chef name, phone, business email, website and city. These appear on your booking page, PDFs and invoices. Your business email is where new-booking alerts go and what clients reply to.

**Booking & Payments** — your booking link (with a Copy button), deposit, cancellation notice, accepted payment methods, the response time clients are told to expect, and your booking terms.

**Services You Offer** — the kinds of work you do (in-home cooking, meal prep, events, classes, special diets).

**Session Defaults** — defaults for session window hours, serving size, service fee, and your **invoice number prefix** (e.g. `INV` → `INV-2026-0001`).

**Reminder Emails** — off until you turn them on. See [Reminder Emails](#reminder-emails) below.

**AI Assistant** — shows whether AI recipe import and menu suggestions are available on this server.

**Data Management:**
- **Export All My Data (JSON)** — downloads everything in your account (clients, recipes, sessions, invoices, intakes, settings)
- **Export Costs CSV** — downloads a spreadsheet of all session financials
- **Clear Sample Data** — removes The Chen Family sample client (cannot be undone)

---

## 5. Core Workflows

### Planning a New Session

The session wizard has 5 steps:

**Step 1 — Select Client**
Pick a client from the dropdown. Their allergens, dietary style, and serving size are shown below. If the client doesn't exist yet, tap "Add New Client" to create one first.

**Step 2 — Schedule**
- Date (required)
- Time (optional but useful for the timeline)
- Serving size (4 or 6)
- Session window in hours (default 3.0)
- Status (Pending or Confirmed)
- Recurring (optional): every week, or every 2, 3 or 4 weeks

**Step 3 — Menu**
This is the most important step.
- Use the **category and session fit filter buttons** to narrow the recipe list
- Use the **search box** to find a specific dish
- Check boxes to add dishes to the session
- The **bottom bar** updates in real time showing dish count, active minutes, passive minutes, estimated total time, and a GREEN / AMBER / RED feasibility indicator
- The **AI Suggest button** (if AI is configured) picks the best 3 dishes for this client automatically and shows the reasoning
- If a recipe contains a client allergen, a warning appears — you can still select it but must acknowledge it at step 5
- When happy with your menu, optionally tap "Save Current Menu as Template"

**Step 4 — Costing**
- Enter the **service fee** (what you charge the client)
- Enter your estimated **ingredient cost**
- The margin and margin % calculate automatically
- Add any chef notes for this session

**Step 5 — Review**
- See the full session summary before confirming
- If any dishes conflict with client allergens, you must check the acknowledgment box
- Tap "Confirm Session" to save

### Recurring Sessions

Most clients book on a regular schedule. Set **Recurring** when you create a session, or later from **Session Info** on the session page.

- When you tap **✓ Mark as Completed** on a recurring session, the next one is booked automatically, that many weeks later. It has the same client, time, window, serving size, fees, notes and menu, and it starts as **Pending**. A message tells you the new date.
- The new session is recurring too, so the series carries on from it. To stop the series, set **Recurring** to "Does not repeat" on the latest session.
- Each session books its follow-up only once, even if you change its status and complete it again.
- Nothing is booked for a session without a date, or for a client you've deactivated. Recipes you've deactivated are left out of the new menu.
- Recurring sessions show 🔁 in the Sessions list. A session booked this way links back to the one it came from.
- The **🔁 Repeat** button at the bottom of a session is different: it makes a one-off copy, on a date you choose, at any time.

### Reminder Emails

Platebook can email a reminder about 24 hours before each **Pending** or **Confirmed** session. Turn it on in **Settings → Reminder Emails**:

- **Remind clients:** the client gets the date, time and menu. If they reply, it comes to your business email. Clients without an email address are skipped.
- **Send me a prep sheet:** you get the client's address, parking notes, phone number, allergens (highlighted) and the menu, with a link to the session. It goes to your business email, or your login email if you haven't set one.
- **Your time zone:** session times are read in this zone, and it also sets what counts as "today" for this month's totals, overdue invoices and new invoice dates. Turning a reminder on fills it in from your device; you can also pick it yourself. A session without a time (or with a time that isn't like `14:30`) is treated as 9:00 AM.

Each reminder goes out once. If you change a session's date or time after its reminder went out, a new reminder is sent for the new time. Cancelled and completed sessions never get one. The session page shows when the reminder was sent.

---

### Importing a Recipe with AI

1. Go to **Recipes** → tap the **+ FAB** → "Import Recipe"
2. Paste the recipe text into the box. You can paste from:
   - A cooking website (copy all text from the page)
   - A screenshot you've transcribed
   - Your own memory ("chicken braised in white wine with mushrooms, shallots, cream...")
3. Tap "Parse Recipe →"
4. The AI (Claude) extracts:
   - Recipe name, category, session fit
   - Prep and cook times
   - Hold and reheat quality estimates
   - Full ingredient list with quantities, units, and store sections
   - Key overlap ingredients
   - Chef notes from the instructions
5. A **confidence banner** shows how reliable the parse was:
   - **HIGH (80–100)** — green, most fields filled correctly
   - **MEDIUM (50–79)** — amber, review times and quantities carefully
   - **LOW (0–49)** — red, significant manual editing needed
6. Edit any field directly in the form
7. Tap "Save Recipe" or "Save & Import Another"

**Tips:**
- The more complete your pasted text, the higher the confidence score
- Always verify ingredient quantities — AI estimates are good but not perfect
- If a quantity shows amber highlighting, it means the AI couldn't parse it

---

### Managing the Grocery List

1. Open a session → tap the **Grocery tab**
2. The list is automatically generated from all dishes in the session, with:
   - Quantities scaled to your serving size (4 or 6)
   - Duplicate ingredients combined (e.g., garlic from three recipes totaled together)
   - Items grouped by store section
3. As you shop, **tap each item** to check it off (gray strikethrough)
4. Use "Mark All" on a section header to check off an entire section at once
5. Tap "Uncheck All" at the top to reset the list
6. If you change the dishes in a session, tap **Regenerate** to rebuild the list
7. Tap **Download Grocery PDF** to get a printable checklist with checkboxes

**iPhone tip:** Open the grocery list before leaving for the store. The checked state persists as long as the servers are running.

---

### Generating Client PDFs

There are two PDFs you can generate for each session:

**Client Leave-Behind PDF:**
- Professional one-page summary for the client
- Includes: all dishes with reheat instructions, use-by date (4 days from session date), storage guidance, allergen notice (if applicable), your business name and session date
- Download from: Session Detail → Overview tab → "Generate Client PDF"

**Grocery List PDF:**
- Printable shopping checklist grouped by store section
- Each item has a checkbox
- Includes source dishes for each ingredient
- Download from: Session Detail → Overview tab → "Download Grocery PDF"

Both PDFs automatically use your business name from Settings.

**If an email doesn't go through:** when emailing a meal plan (✉️ Email on a session) or an invoice fails, Platebook tries once more automatically. If that fails too, the session or invoice page shows a red banner with the time and a **Try again** button, and the Sessions and Invoices lists show "⚠ Email failed" until a send succeeds. A missing client email address isn't counted as a failure: add one to the client (or invoice) and send again.

The same goes for the two emails sent when a client books through your booking page: their confirmation (with the link to review their request) and your new-booking alert. If either fails after the automatic retry, the Client Intakes page shows a red note at the top, and the inquiry shows "⚠ Confirmation email failed" or "⚠ Alert email failed". Open the inquiry to see when it happened. For the confirmation, **Try again** sends it to the client again. A failed alert to you can't be resent (you're already looking at the inquiry), but it usually means your business email in Settings needs fixing.

**Getting paid online:** with online payments on (Stripe, set up by whoever runs your Platebook server), the invoice email has a **Pay online** button. It opens a page with the invoice where your client pays by card. The invoice turns **Paid** on its own once Stripe confirms the payment. To send the link again, tap **🔗** in the invoice's action bar to copy it. The link works for 180 days, and only for that invoice. A paid or void invoice can't be paid again. If you void an invoice whose link is still open, check for a payment before you do; a refund is made in Stripe.

---

### Booking Inquiries

Requests from your booking page land on the **Client Intakes** page. Tap one to see the client's answers, record a deposit, add private notes, or convert it into a client.

- **Payment history:** the Payment section lists every payment action on the inquiry, oldest first: deposit requested, marked paid, waived, and online payments and refunds when Stripe is on.
- **Deposits:** with online payments on (Stripe, set up by whoever runs your Platebook server), clients pay the deposit themselves from their booking page: the link in their confirmation email, or right after they book. It's marked paid automatically and shows in Payment History. To send a client their page again, use **Copy client's booking link** on the inquiry. Without online payments, record deposits by hand with the Payment Actions buttons.
- **Deleting an inquiry:** use **Delete inquiry** at the bottom (for spam, duplicates or leads that went nowhere). It isn't erased: it moves to the **Deleted** filter with its payment history, and **Restore** brings it back. While it's deleted, the client's link to their request stops working and you can't edit it or take payments on it. If a client pays online for a deleted inquiry, it's restored automatically.

---

### Tracking Profitability

1. Go to **Costs**
2. Review the summary cards at the top for month-to-date totals
3. In the sessions table, **click any fee or cost value** to edit it inline
4. The margin updates immediately
5. Scroll down to see **per-client profitability** — clients sorted by margin %
6. Use "Export CSV" to bring the data into Excel or Google Sheets for further analysis

**Target margins:**
- **≥ 60%** — healthy (shown in green)
- **40–59%** — acceptable (shown in amber)
- **< 40%** — needs review (shown in red)

---

## 6. AI Features

Platebook includes two AI features powered by the Anthropic Claude API.

### Setup
AI is configured once for the whole server by whoever runs it (it is shared by all accounts, and usage is rate-limited per account):
1. Get an API key from [console.anthropic.com](https://console.anthropic.com)
2. Open `server-rs/.env` and add:
   ```
   ANTHROPIC_API_KEY=sk-ant-your-key-here
   ```
3. Restart the API server
4. In Platebook → Settings → AI Assistant, tap "Test Connection"
5. The status should change to "● Available"

### Feature 1: Recipe Import
When you paste recipe text into the Import page, Claude extracts all structured data automatically. See [Importing a Recipe with AI](#importing-a-recipe-with-ai) above.

### Feature 2: AI Menu Suggestion
In Step 3 of the session wizard, tap **"AI Suggest"**. Claude analyzes:
- The client's allergens and dietary restrictions
- Their liked proteins and disliked foods
- The selected serving size
- Your entire recipe library
- Which recipes share key ingredients (minimizing shopping overlap)
- Which recipes are Session-Friendly (fit in a 3-hour window)

It returns 3 recommended recipes with a reasoning explanation. You can accept them all, remove some, or add others.

### Without an API Key
Both AI features are disabled but the rest of the app works fully. You can plan sessions, manage clients, import recipes manually, and track costs without any AI.

---

## 7. Understanding Session Feasibility

The feasibility indicator appears during session planning and on the session detail page.

### Calculation
```
Estimated time = (sum of all dish prep times)
               + (longest single cook time)
               + 30 min buffer
               - (5 min overlap credit × number of dishes)
```

### Levels

| Level | Conditions | Meaning |
|---|---|---|
| **GREEN ✓** | Est ≤ 210 min AND all dishes Session-Friendly AND ≤ 5 dishes | Fits comfortably in a 3-hour window |
| **AMBER ◐** | Est 211–240 min, OR 1 Long-Cook dish with ≤ 5 total | Will be tight — plan carefully |
| **RED ↓** | Est > 240 min, OR 2+ Long-Cook dishes, OR > 5 dishes | Exceeds typical session window |

### Tips
- Aim for GREEN for every session
- Long-Cook dishes (tagines, lasagnas, braised meats) add significant passive time
- Use Session-Friendly recipes when building 3-dish or 4-dish menus
- The overlap credit rewards you for dishes that share oven/stovetop time

---

## 8. Allergen Safety System

### How it works

1. **Client profile** — you define which allergens a client has (Dairy, Gluten, Eggs, Tree Nuts, Peanuts, Fish, Shellfish, Soy, Sesame)
2. **Ingredient scanning** — when viewing recipes, Platebook scans every ingredient name for keywords associated with each allergen
3. **Session warnings** — during menu building, any recipe that contains a client's allergen shows a warning badge
4. **Conflict acknowledgment** — if you choose to include a conflicting recipe, you must check an acknowledgment checkbox at step 5 before the session can be saved
5. **PDF notice** — the client leave-behind PDF includes an allergen awareness section listing the avoided allergens

### Detected allergens and their keywords

| Allergen | Detected ingredients (partial list) |
|---|---|
| Dairy | milk, cream, butter, cheese, ricotta, parmesan, yogurt, ghee |
| Gluten | flour, breadcrumbs, pasta, panko, wheat, soy sauce, couscous |
| Eggs | egg, eggs, egg yolk, mayonnaise |
| Tree Nuts | almond, walnut, pecan, cashew, pistachio, pine nut |
| Peanuts | peanut, peanut butter, groundnut |
| Fish | anchovy, salmon, tuna, fish sauce, worcestershire |
| Shellfish | shrimp, crab, lobster, scallop, clam, mussel |
| Soy | soy sauce, tofu, edamame, miso, tamari |
| Sesame | sesame, tahini, sesame oil |

### Important note
The allergen system is a **planning aid**, not a medical guarantee. Always verify ingredients yourself when serving clients with severe allergies.

---

## 9. Customizing Platebook

### Branding & Business Info

The quickest customization — no code needed:

1. Go to **Settings**
2. Update Business Name, Chef Name, Phone, Email, Website, Logo
3. Tap "Save Settings"
4. Your name and business will now appear on your booking page, PDFs and invoices

**Renaming the product itself** (for whoever runs the server): change `APP_NAME` and `APP_TAGLINE` in `client/src/constants.ts`, the `<title>` in `client/index.html`, and the static `landing-page/index.html`.

---

### Adding Your Own Recipes

**Via the app (recommended):**
- Import with AI: Recipes → + → Import Recipe → paste text
- Add manually: Recipes → + → Add Manually → fill form

**For every new account (starter library):** the recipes offered during onboarding are in `server-rs/src/seed/starter_library.json` (built into the server). Edits there apply to accounts that choose the starter library after the server is rebuilt; existing accounts are unchanged, so use the app's Add Recipe form for those.

---

### Modifying the Color Theme

Open `client/src/index.css`. The color palette is defined in the `@theme` block at the top (Tailwind 4 reads it from CSS; there is no `tailwind.config.js`):

```css
@theme {
  --color-navy:        #5E7F57; /* Herb Green   (primary header / main buttons) */
  --color-navy-mid:    #C56A4A; /* Terracotta   (secondary actions) */
  --color-green-dark:  #3A5C32; /* Deep Herb    (section headers / active links) */
  /* … */
}
```

Change any hex value to adjust that color everywhere in the app (`bg-navy`, `text-navy`, …). After saving, Vite will hot-reload the browser automatically.

**Example — make the app warmer (swap navy for a deep forest green):**
```css
--color-navy:     #1A3A2A;
--color-navy-mid: #2D6A4F;
```

---

### Changing Allergen Keywords

Open `client/src/constants.ts`. Find the `ALLERGEN_KEYWORDS` object:

```javascript
export const ALLERGEN_KEYWORDS = {
  Dairy: ['milk', 'cream', 'butter', 'cheese', 'ricotta', /* ... */],
  Gluten: ['flour', 'breadcrumbs', 'pasta', /* ... */],
  // ...
};
```

Add new keywords to any category:
```javascript
Dairy: ['milk', 'cream', 'butter', 'cheese', 'ricotta', 'crème fraîche', 'heavy cream', 'half-and-half'],
```

Or add a new allergen category entirely:
```javascript
Corn: ['corn', 'cornstarch', 'corn syrup', 'masa', 'polenta', 'hominy'],
```

Then add `'Corn'` to the allergen chips list in `client/src/pages/ClientForm.tsx`.

---

### Adjusting Feasibility Thresholds

Open `client/src/constants.ts`. Find the `calcFeasibility` function:

```javascript
export function calcFeasibility(dishes) {
  const total_active = dishes.reduce((s, d) => s + (d.prep_time_min || 0), 0);
  const passive = Math.max(...dishes.map(d => d.cook_time_min || 0));
  const buffer = 30;
  const overlap_credit = (dishes.length - 1) * 5;
  const est = total_active + passive + buffer - overlap_credit;

  const longCookCount = dishes.filter(d => d.session_fit === 'Long-Cook').length;

  if (est <= 210 && longCookCount === 0 && dishes.length <= 5) return 'GREEN';
  if (est <= 240 && longCookCount <= 1 && dishes.length <= 5) return 'AMBER';
  return 'RED';
}
```

Adjustments you might want:
- **Longer session window** (e.g., 4 hours): change `210` to `240` and `240` to `270`
- **More overlap credit** (if you're efficient at parallel cooking): change `* 5` to `* 10`
- **Larger buffer**: change `const buffer = 30` to `const buffer = 45`
- **Allow more dishes**: change `<= 5` to `<= 6`

---

### Editing Store Sections

Open `client/src/constants.ts`. Find `STORE_SECTIONS`:

```javascript
export const STORE_SECTIONS = [
  'Produce',
  'Meat & Seafood',
  'Dairy & Eggs',
  'Pantry & Dry Goods',
  'Canned & Jarred',
  'Herbs & Spices',
  'Other',
];
```

Add, remove, or rename sections to match how your store is laid out. Also update the `STORE_SECTION_ORDER` array in `client/src/utils/pdfGen.ts` to keep PDF export ordering consistent.

---

### Customizing Reheat Instructions

Open `client/src/constants.ts`. Find `REHEAT_INSTRUCTIONS` and `getReheatedInstructions`:

```javascript
export const REHEAT_INSTRUCTIONS = {
  stew:     'Stovetop, medium-low, 5–8 min with lid.',
  curry:    'Stovetop, medium-low, add splash of water, 5–8 min.',
  pasta:    'Oven 325°F covered 20 min, or microwave 2–3 min.',
  meatball: 'Stovetop 5–8 min or microwave 2 min.',
  bowl:     'Warm protein separately, serve over cold grain.',
};

export function getReheatedInstructions(dishName) {
  const n = dishName.toLowerCase();
  if (n.includes('curry') || n.includes('panang') || n.includes('massaman')) return REHEAT_INSTRUCTIONS.curry;
  if (n.includes('stew') || n.includes('tagine') || n.includes('braised') || n.includes('osso')) return REHEAT_INSTRUCTIONS.stew;
  if (n.includes('pasta') || n.includes('ziti') || n.includes('lasagna') || n.includes('spaghetti')) return REHEAT_INSTRUCTIONS.pasta;
  if (n.includes('meatball')) return REHEAT_INSTRUCTIONS.meatball;
  if (n.includes('bowl')) return REHEAT_INSTRUCTIONS.bowl;
  return REHEAT_INSTRUCTIONS.stew; // default
}
```

Modify existing instructions or add new keyword matches for your specific dishes.

---

### Modifying Session Templates

Templates are stored in the database. You can manage them entirely through the app:

- **Add**: During session step 3, select your dishes → "Save Current Menu as Template"
- **Delete**: Templates page → tap "Delete" on any non-default template
- **Edit**: Templates cannot be edited directly — delete and recreate with the new dishes

The 4 starter templates come with the starter library (`server-rs/src/seed/starter_library.json`) and are added per account during onboarding.

---

### Database Customization

The data lives in PostgreSQL (the database named by `DATABASE_URL` in `server-rs/.env`; `chef_dev` locally). It holds **every account's** data; each row belongs to an account via `account_id`, and Row-Level Security keeps accounts apart.

You can open it with any Postgres client (`psql`, [pgAdmin](https://www.pgadmin.org), DBeaver). Connect as the database owner for maintenance: the app itself only sees one account's rows at a time.

**Common direct edits:**
- Bulk-update ingredient quantities for scaling changes
- Review or export session history
- Fix a typo in a client name you can't easily edit in the UI

**Schema modifications** (adding columns, tables): add a new numbered SQL file in `server-rs/migrations/`; the server applies it on start. New columns won't automatically appear in the UI: the Rust route and the frontend need changes too.

---

## 10. Data Management & Backup

### Backup
**Your account:** go to **Settings → Export All My Data (JSON)** to download everything in your account. Store it somewhere safe.

**The whole server** (operators): back up the Postgres database with `pg_dump --format=custom -f chef.dump "$DATABASE_URL"`. With `AUTO_BACKUP=true` in `server-rs/.env`, the server also writes a daily `pg_dump` into `BACKUP_DIR` (keeps the last 30). Hosted Postgres (e.g. Railway) takes its own daily backups.

### Restore
To restore the whole server from a backup:
1. Stop the server
2. `pg_restore --clean --if-exists -d "$DATABASE_URL" chef.dump`
3. Restart the server

### Export for Accounting
**Settings → Export Costs CSV** downloads a spreadsheet with all sessions, dates, clients, fees, costs, and margins. Import this into Excel, Google Sheets, or your accounting software.

### Moving to a New Computer
1. Make a `pg_dump` backup (above)
2. On the new machine, install the prerequisites and clone/copy the Platebook project folder
3. Run `npm run install:all` and `npm run db:setup`
4. Copy your `server-rs/.env` values (API key, secrets) into the new `server-rs/.env`
5. Start the server once (`npm run dev`) so it creates the schema, stop it, then restore the backup with `pg_restore`
6. Start the servers

Data from the **old SQLite version** of the app (`chef_app.db`) moves over with the migration tool in `tools/migrate-sqlite` (see its README).

---

## 11. Keyboard Shortcuts & Tips

### General
- `Escape` — close modals and confirmation dialogs
- Swipe left on any card (recipe, client) to reveal Edit / Delete
- Swipe right on a swiped card to dismiss the action buttons

### Session Wizard
- The feasibility bar at the bottom updates live as you add/remove dishes
- You can go back to any previous step using the numbered step indicators at the top
- Filtering by "Session-Friendly" in step 3 is the fastest way to build a feasible menu

### Grocery List
- Checking items off persists across browser reloads (stored in the database)
- Regenerating the list clears all checkmarks — do this before shopping, not during

### iPhone
- Add to Home Screen for an app icon (see Getting Started → iPhone)
- Portrait orientation is optimal
- Landscape works but some filter rows may scroll horizontally

---

## 12. Troubleshooting

### App won't load (`localhost:5174` blank or error)
- Make sure both servers are running: the API server on port 3002 and the frontend on port 5174 (`npm run dev` starts both)
- Check for errors in the terminal — common causes are a missing `node_modules` folder (run `npm run install:all`) or Postgres not running (run `npm run db:setup`)

### Can't connect from iPhone
- Confirm your phone and computer are on the same Wi-Fi network
- Verify the IP address with `ipconfig` (Windows) or System Settings → Wi-Fi (macOS)
- On Windows, check that the firewall rule was added for port 5174
- Restart the Vite frontend server after changing `vite.config.ts`

### AI features not working
- Verify `ANTHROPIC_API_KEY` is set in `server-rs/.env`
- Restart the API server after changing `.env`
- Go to Settings → AI Configuration → "Test Connection"
- Check that your API key has sufficient credits at console.anthropic.com

### Recipes page crashes or shows blank
- Open your browser's developer console (F12) and look for the error
- The most common cause is a bad import in a custom component — compare against the original source files

### Grocery list not generating
- Make sure the session has at least one dish selected
- Tap "Regenerate" to force a fresh build
- Check the API server terminal for errors (`/api/sessions/:id/grocery/generate`)

### Database errors on startup
- The server needs Postgres running and `DATABASE_URL` in `server-rs/.env` pointing at it: run `npm run db:setup`, which creates the database (or starts it in Docker). The schema is applied automatically on start.
- If the data is damaged, restore the latest `pg_dump` backup (see Data Management & Backup).
- **Warning:** dropping the database loses all your data. Always keep a backup.

### Port already in use
If the server says the address is already in use:
- Another process is using port 3002 or 5174
- On Windows: `netstat -ano | findstr :3002` then `taskkill /PID <pid> /F`
- On macOS: `lsof -ti:3002 | xargs kill`

---

*Platebook · Version 1.1.0*
