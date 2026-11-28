# Chef tester, leg 2: Robin Hale (Juniper Table Kitchen), inquiries → clients → week plan → groceries

Date: 2026-11-24 (fictional week of interest: Mon 2026-12-07). App: http://localhost:4310 (production-mode instance), driven with Playwright Chromium, 1280x800, deviceScaleFactor 2 (one 375px check).
Screenshots are in `screenshots/`. Exported files: `okafor-grocery.pdf` (with `okafor-grocery-pdf.txt` from `pdftotext -layout`) and `okafor-grocery-copied.txt` (clipboard text from "Copy / Share").
No emails were sent in this leg: the mail sink held 4 messages at the start and still held 4 at the end. I did not click "Try again", "Request Deposit", "Email", or anything to do with invoices.

## Key results (checked after reload)

- **3 inquiries found, 3 clients created, 0 duplicates.** I double-clicked Convert, and also converted the same inquiry from a second, stale tab. Each time the server returned the existing client (201 the first time, 200 after that).
- **Final week**, checked after reload in the Sessions list, the Calendar, each session page, and with a read-only API check (`GET /api/sessions`):

| Household | Date | Time | Servings | Dishes | Recurrence | Status | Warnings / notes |
|---|---|---|---|---|---|---|---|
| Okafor | Mon 2026-12-07 | 10:30 (their edited time) | 4 | Garlic Lemon Chicken Rice Bowls, Chickpea & Spinach Curry | Every week (`repeat_every_weeks: 1`) | Confirmed | Satay was added, triggered "⚠ Allergen Conflict Detected" and a required acknowledgement, then was removed. The calendar shows **only Dec 7**. Later weeks are created only when the session is marked complete. |
| Lindqvist | **Thu 2026-12-10** (moved from Wed 12-09) | 13:00 | **4** (asked for 2, but the wizard offers only 4 or 6) | Roasted Vegetable Tray Bake, Chickpea & Spinach Curry | none | Confirmed | Rescheduling had to be done by creating a new session and deleting the Wednesday one. There is no way to edit a session's date. |
| Castellano | Fri 2026-12-11 | 15:00 | 6 | Spicy Black Bean Chili | none | Confirmed | The curry is hidden from Castellano because of the false "Dairy" flag (leg-1 D2). |
| Castellano (repeat) | Fri 2026-12-18 | 15:00 | 6 | Spicy Black Bean Chili | none (made with "Repeat +1 week") | **Pending** (the clone does not keep "Confirmed") | There is no control to change the status to Confirmed. |

## Chronological log

| # | Goal | What I tried | What the screen showed | Outcome | Screenshot / verification |
|---|---|---|---|---|---|
| 1 | Find inquiries | Signed in. Looked at the dashboard and the side nav. | Dashboard: "Active Clients 0", "No upcoming sessions", booking-link card. **Nothing says 3 inquiries are waiting.** The nav (Dashboard, Clients, Sessions, Recipes, Invoices, Costs, Guide, Settings) has **no Inquiries entry.** | friction / missing (F1) | 01-dashboard-no-inquiry-signal.png |
| 2 | Find inquiries | Opened Clients. | "No clients yet / Add your first client". No mention of inquiries. | friction | 02-clients-page-inquiries.png |
| 3 | Find inquiries | As Robin, opened my business inbox (mail sink). Two alerts were waiting: "New client inquiry from Okafor Household…" and "…Castellano Household…". Each gives only email (and phone for Okafor) plus "Review the inquiry: http://localhost:4310/intake". Clicked that link. | `/intake` "Inquiries · 3 total · 📥 3 new submissions awaiting review", with a banner "⚠ Booking emails didn't go through for 1 inquiry". The Lindqvist card shows "⚠ Confirmation email failed" and "⚠ Alert email failed". (I later found that the Guide page also has an "Open inquiries →" link.) | pass (found only through the email) | 03-inquiries-list-from-email-link.png |
| 4 | How would I have heard? | Compared the three cards. | Okafor and Castellano reached me by alert email. **Lindqvist produced no alert at all.** The app says so in the detail view: "Platebook couldn't email you about this inquiry (Nov 24 at 3:28 PM): the mail server was unavailable or isn't set up. Check this page for new inquiries until email works." That message is clear. But the only place to "check this page" is a page with no nav link. The customer was told on purpose that "No confirmation email will be sent", yet the chef sees "Confirmation email failed" with a **Try again** button, which on this instance would now email Sven. I did not click it. | pass / misleading (M1) | 05-inquiry-lindqvist-detail.png |
| 5 | Review Okafor | Opened inquiry #1. | Phone; Small Family 2+2; 4 servings; Allergens Peanuts, Tree Nuts; Nut-Free Kitchen Required; Likes Chicken; Avoids "Mushrooms. No peanuts or tree nuts at all (serious child allergy)."; Spice Mild; Cuisines "Rice bowls…"; **Time 10:30** (the edit carried over); Scheduling note still says "every Monday morning around 10:00"; Client note ends "Sesame is fine." (the edit carried over). **No marker or timestamp shows that the customer edited the request after submitting**, and the list date stays "Nov 24, 2026". | pass / friction (F2) | 04-inquiry-okafor-detail.png |
| 6 | Review Lindqvist | Opened #2. | Couple 2/0, 2 servings, Gluten, Vegetarian + Gluten-Free, "Plant-Based Only", strict-celiac avoid text, Wednesday 13:00. | pass | 05-inquiry-lindqvist-detail.png |
| 7 | Review Castellano | Opened #3. | Large Family 4/2, 6 servings, Dairy, Dairy-Free, Bold, Friday 15:00. The scheduling note "…this Friday AND a repeat session the following Friday as well (two sessions)" and the client note "Please confirm both Fridays" appear only as free text. Nothing turns the request into two sessions. | pass / missing (X1) | 06-inquiry-castellano-detail.png |
| 8 | Convert Okafor (twice) | Double-clicked "✓ Convert to Client Record". | A native browser `confirm()` asks "Convert … to a client record?" (the first time, my script dismissed it and nothing happened). With both confirms accepted: **one** `POST /api/intake/1/convert` (201), then the app went to `/clients/1` with the toast "Client record created". | pass | 07-okafor-after-convert-doubleclick.png |
| 9 | Convert again | Reopened inquiry #1. | The Convert button is replaced by "Open client record / Already a client." | pass | 08-okafor-inquiry-after-convert.png |
| 10 | Convert from a stale tab | Opened Lindqvist in two tabs and converted in tab A (201), then in tab B. | Tab B: `POST …/2/convert` returned **200 with the same client id 2**, opened `/clients/2`, and showed the toast "Client record created" (slightly misleading, because nothing new was created). The client list holds one Lindqvist. | pass (M2 minor) | 09-lindqvist-convert-from-second-tab.png, 10-clients-list-no-duplicates.png |
| 11 | Convert Castellano | Converted it. | Client 3 created. **Every inquiry's status then changed to "Booked"**, although no session existed yet. | misleading (M3) | 11-castellano-client-created.png, 54-inquiries-final-all-booked.png |
| 12 | Check what carried over | Read the three client profiles. | Carried over: name, email, phone, household, serving size, allergens, dietary style, liked proteins, avoid text, spice, preferred day and time. **Lost: "Cuisines"** (Okafor "Rice bowls (kids love them), mild flavours", Lindqvist "Roasted vegetables, chickpeas, Mediterranean", Castellano "Spicy food, large portions"). The client form has no field for them. Scheduling notes were put into "Chef Notes". On the edit form the same text also appears under **"Additional Dietary Notes"**, because both boxes edit the same field (see F4). | friction / defect (D5) | 12-lindqvist-edit-form.png, 15-lindqvist-profile-corrected.png |
| 13 | Invalid edit 1 | Lindqvist edit: cleared Full Name and clicked Save. | Red field outline, "Full name is required" under the field, and the toast "Please fix the highlighted fields". Nothing was saved. | pass | 13-lindqvist-edit-empty-name-save.png |
| 14 | Invalid edit 2 | Set Adults to **-1** with the name filled in, then clicked Save. | **Saved.** "Client updated", and `PUT /api/clients/2` returned `"adults":-1`. The profile showed "-1 / 0". | **defect (D3)** | 14-lindqvist-negative-adults-save.png |
| 15 | Correct profile | Set Adults back to 2. Wrote "Strict celiac: GF kitchen practices. Loves roasted vegetables, chickpeas, Mediterranean." into Additional Dietary Notes. | Saved. The same text also appeared in "Chef Notes" (it is one field). Allergens and diets were already correct for all three (API check: Okafor Peanuts + Tree Nuts and Nut-Free Kitchen; Lindqvist Gluten and Vegetarian + Gluten-Free; Castellano Dairy and Dairy-Free). | pass | 15-lindqvist-profile-corrected.png |
| 16 | Okafor session | Client page, "+ Session". The 5-step wizard prefilled the date 2026-11-30 (next Monday) and **10:30** from the client. Changed the date to 2026-12-07, Confirmed, Recurring "Every week". | Hint: "When you mark this session complete, the next one is booked automatically." | pass | 17-new-session-schedule-step.png |
| 17 | Allergy conflict | Menu step: "Hide allergen conflicts" is **on by default** ("Hiding 1 allergen conflicts"), so Satay is not listed. Picked the Bowls and the Curry, turned the switch off, and selected "Satay Noodles (contains peanuts)". | The card shows "⚠ Contains: Peanuts". **There is no prompt or banner on the menu step.** Total feasibility: "Dishes: 3 · Active 0 min · Passive 0 min · Total ~20 min · GREEN". With 2 dishes it had said ~25 min, so **adding a dish lowered the total**. | pass (flagged) / defect (D6) | 19-menu-step-conflicts-shown.png, 20-satay-added-allergy-warning.png |
| 18 | Allergy conflict | Costing step, then Review. | Review: "⚠ Allergen Conflict Detected — Satay Noodles (contains peanuts): contains Peanuts" with a checkbox "I acknowledge this allergen conflict and confirm the chef has verified safety". **"Create Session" is disabled until it is ticked.** That blocks well. To proceed you must tick the acknowledgement. As Robin (a serious child allergy), I went back to Menu and removed Satay instead. | pass | 21-costing-step-with-satay.png, 22-review-step-with-satay.png |
| 19 | Create Okafor | Double-clicked "Create Session". | One `POST /api/sessions` (201). Session 1: Mon Dec 7 10:30, 4 servings, Bowls + Curry, Every week. The Costing step showed Ingredient cost $0, so the margin shows "100%". | pass | 23-okafor-session-created.png |
| 20 | Allergy check after creation | Looked for a way to add Satay to the **saved** session. | The session page has **no menu, date, time or servings editing**, only "Edit costs" (fee and ingredient cost), Recurring, notes, Repeat and Delete. So the conflict could only be tested in the creation wizard. | missing (X2) | 34-lindqvist-session-only-costs-editable.png |
| 21 | Lindqvist session | "+ Session" for Lindqvist (serves 2). | **Serving Size offers only "4 servings" / "6 servings", and 4 is preselected**, although the client profile says 2 servings and the client form allows 2, 4, 6, 8 or a custom value. I had to create a 4-serving session. The menu step hides only allergen conflicts: **Garlic Lemon Chicken (meat) is offered to a vegetarian household with no warning.** | **defect (D4)** / missing (X3) | 24-lindqvist-schedule-step-serving-options.png, 25-lindqvist-menu-step.png, 26-lindqvist-session-created.png |
| 22 | Castellano session | Fri 12-11 15:00, 6 servings, Chili. | Menu step "Hiding 1 allergen conflicts": the **Chickpea curry is hidden** because coconut milk is flagged as Dairy (leg-1 D2). Created session 3. | pass / known defect | 27-castellano-menu-curry-hidden-as-dairy.png, 28-castellano-session-created.png |
| 23 | Repeat Friday | Session 3, "🔁 Repeat", then "+1 week (Friday, Dec 18)". | Session 4 created on Fri Dec 18 15:00 with the same menu, **but status "Pending"** (the original was Confirmed). There is no visible control to change status. | pass / friction (F6) | 29-castellano-repeat-dialog.png, 30-castellano-repeat-dec18-created.png |
| 24 | Calendar conflict | Created a Lindqvist session at **Fri 12-11 15:00**, the same time as Castellano. | **No warning in the wizard** (schedule step or review). Created (201). Afterwards the **Calendar** shows a banner "⚠ 1 day with overlapping sessions — check the times", a ⚠ on the 11th, and "⚠ Multiple sessions" in the day panel. The Sessions list and the Dashboard show both sessions with no warning. | friction (F7: warns only after the fact, only on the Calendar) | 35-conflict-attempt-schedule-step.png, 36-conflict-attempt-review-step.png, 37-calendar-dec-double-booking.png, 38-calendar-dec11-conflict-detail.png |
| 25 | Remove the conflict | "Delete Session" on the conflicting session. | An in-app dialog: "This will permanently delete this session and all associated data." Clicked Delete, `DELETE` returned 200. | pass | 40-delete-session-confirm.png |
| 26 | Move Lindqvist Wed→Thu | Looked on the session page (clicked the date and the header), in the Calendar (the session entry is not draggable and only links to the session), and in the Guide (nothing about rescheduling). | **No way to change a session's date.** "Repeat" can only shift by +1, +2 or +4 weeks. | **missing (X2)**. I needed help (see below). | 33-calendar-dec9-selected.png, 34-…png |
| 27 | Move Lindqvist (workaround) | Created a new Lindqvist session Thu 12-10 13:00 with the same dishes (forced 4 servings again), then deleted the Wed 12-09 session. | Done. The calendar now shows Mon 7, Thu 10, Fri 11 and Fri 18, with no conflict banner. | pass (workaround) | 39-lindqvist-thursday-session-created.png, 55-calendar-december-final.png |
| 28 | Feasibility | Timeline tab of the Okafor session, plus the header badges. | "GREEN — 25min". Timeline: "0:15 — Begin Garlic Lemon Chicken Rice Bowls prep (0 min active)", every step at 0:15, "Estimated Total: ~25 min". **Leg 1 did not set prep or cook times** (every recipe shows "0 min"), so GREEN, 25m and the timeline are **meaningless**. The app never says that times are missing. With 3 dishes it showed ~20 min and with 2 dishes ~25 min. | misleading (M4) / defect (D6) | 41-okafor-timeline-zero-times.png |
| 29 | Okafor grocery | Grocery tab. | A full list sorted by store section, with source dishes. Totals and units are combined correctly (see the table below). | pass | 42-okafor-grocery-tab-initial.png |
| 30 | Tick two items | Clicked "garlic" and "olive oil", tried Space on the focused row, and tried "Mark All". | **Nothing happened.** No request was made and `aria-checked` stayed false. The list on screen was a **preview that had not been saved** (API check on an untouched session, `GET /api/sessions/3/grocery`: the items have **no `id` and no `is_checked`**). Nothing on screen says that. Only after **"↺ Regenerate"** (`POST …/grocery/generate`) could items be ticked (`PUT /api/grocery/:id`). | **defect (D1)** | API verification in log |
| 31 | Ticks persist | Ticked garlic and olive oil after Regenerate, then reloaded. | The ticks stayed after reload (Produce 1/4, Pantry 1/2, strike-through). | pass | 43-okafor-grocery-two-ticks-after-reload.png |
| 32 | Stale list | Session servings and menu cannot be edited (X2), so I changed the **recipe** instead: Garlic Lemon Chicken garlic 4 → 5 clove, saved. (The edit form's "Qty (6)" stayed at "6 clove" and was not recalculated.) Reopened the Okafor grocery tab. | **The list silently stayed at "garlic — 7 cloves / Bowls: 4 clove".** No stale warning. | **defect (D7)** | 46-okafor-grocery-stale-after-recipe-edit.png |
| 33 | Regenerate | Clicked "↺ Regenerate". | In-app dialog: "Rebuild the grocery list? The 2 items you have ticked off will be unticked. Rebuild only if the menu has changed." Clicked "Rebuild list": garlic became **8 cloves (Bowls: 5 · Curry: 3)** and **both ticks were cleared**, including olive oil, which had not changed. When no items are ticked, Regenerate runs without asking. | pass (honest warning) / friction (F8) | 47-okafor-rebuild-list-confirm.png, 48-okafor-grocery-rebuilt-garlic-8-ticks-cleared.png |
| 34 | Restore | Set the recipe back to 4 clove, regenerated, ticked garlic and olive oil again, and reloaded. | 7 cloves again, with 2 ticks kept. | pass | 49-okafor-grocery-final-7-cloves-two-ticks.png |
| 35 | Castellano 6 servings | Grocery tab for the Dec 11 Chili (recipe base = 6). | black beans **4 1/2 cans**, onion **3**, garlic **6 cloves**, olive oil **3 tbsp**, chili powder **3 tbsp**, tomatoes **1 1/2 cans**. The recipe already serves 6, so 6 servings should need 3 cans, 2 onions, 4 cloves, 2 tbsp, 2 tbsp and 1 can. **The grocery list uses the wrong stored "Qty (6)" column: 1.5× too much of everything.** | **defect (D2, leg-1 D1 confirmed in groceries)** | 44-castellano-chili-6-servings-grocery.png |
| 36 | Lindqvist 4 servings | Grocery tab for the Thu 12-10 session (Tray Bake base = 2, served at 4). | zucchini **2**, red onion **1**, bell pepper **1**, tray-bake chickpeas **2 cup**, tray-bake olive oil **2 tbsp**. Those are the amounts for 2 servings, used for 4. **Half of what is needed.** The curry (base 4) is correct. | **defect (D2)** | 45-lindqvist-grocery-4-servings.png |
| 37 | Copy / Share | Clicked "📋 Copy / Share". | Toast "Grocery list copied to clipboard". The text matches the screen item for item (saved as `okafor-grocery-copied.txt`). It **does not mark the 2 ticked items** and leaves out the source dishes. | pass / friction (F9) | 50-okafor-grocery-copy-share.png |
| 38 | PDF | Clicked "PDF". | Toast "PDF ready". Downloaded `grocery-okafor-household-(ada-okafor)-2026-12-07.pdf`. The text matches the screen: 4 sections, all 9 items, same quantities, source dishes, "Serves 4". The footer says "**Prepared Monday, December 7, 2026**", which is the session date, not today's date (Nov 24). The PDF does not show which items are ticked. | pass / misleading (M5) | okafor-grocery.pdf, okafor-grocery-pdf.txt, 51-okafor-grocery-pdf-page1.png |
| 39 | Persistence | Reloaded Sessions, Clients, Inquiries, the Calendar and each session page. Read-only API checks on `/api/sessions` and `/api/clients`. | Everything above was still there (see the table at the top). The Clients list says Okafor has "Sessions through Dec 7, 2026" although the session repeats weekly. | pass / misleading (M6) | 52-…, 53-…, 54-…, 55-calendar-december-final.png |
| 40 | Phone width | 375x812: Okafor grocery tab, Inquiries, New Session. | scrollWidth was 375 on all three. The grocery list is readable and the tick boxes are large. | pass | 56-okafor-grocery-375px.png |

## Okafor grocery list (as shown, Mon Dec 7, 4 servings) with an arithmetic check

Both dishes have base serves = 4, so the session amount should equal the recipe amount, summed across the dishes.

| Item | Shown | Source dishes (as shown) | Expected from recipes | Check |
|---|---|---|---|---|
| garlic | 7 cloves | Bowls 4 clove · Curry 3 clove | 4 + 3 = 7 clove | ✓ combined, unit pluralised |
| onion | 2 | Bowls 1 · Curry 1 | 1 + 1 = 2 | ✓ combined |
| lemon | 1 | Bowls 1 | 1 | ✓ |
| spinach | 4 cups | Curry 4 cup | 4 cup | ✓ |
| chicken thighs | 1 1/2 lb | Bowls 1.5 lb | 1.5 lb | ✓ |
| rice | 2 cups | Bowls 2 cup | 2 cup | ✓ |
| olive oil | 3 tbsp | Bowls 2 tbsp · Curry 1 tbsp | 2 + 1 = 3 tbsp | ✓ combined |
| chickpeas | 2 cups | Curry 2 cup | 2 cup | ✓ |
| coconut milk | 1 can | Curry 1 can | 1 can | ✓ |

All 9 items are correct, and shared ingredients are merged with the right totals and units. Sections: Produce 4, Meat & Seafood 1, Pantry & Dry Goods 2, Canned & Jarred 2. The PDF and the copied text match exactly. The Okafor list is right only because both recipes have base 4 and the session is 4 servings. At any other combination the list is wrong (D2: Castellano 1.5× too much, Lindqvist tray bake half of what is needed).

**Stale-list experiment:** after the list existed (with 2 ticks), I changed the Bowls recipe's garlic from 4 to 5 clove. The list **silently stayed at 7 cloves**, with no badge, banner or prompt. "↺ Regenerate" warned honestly that ticks would be cleared. After "Rebuild list" it showed 8 cloves, and **both ticks were gone**, including the unchanged olive oil. Changing a session's servings or swapping a dish, which is what the goal asked for, **cannot be done at all** (X2), so a chef who needs to change Castellano from 6 to 8 servings would have to delete and recreate the session.

## Where I needed help or code knowledge

- **Rescheduling Lindqvist (Wed → Thu):** I could not find any way to change a session's date in the session page, the Calendar or the Guide. Needed help: I read `client/src/App.tsx` (route list) and grepped `client/src/pages/SessionDetail.tsx`. The only session routes are `/sessions/new` and `/sessions/:id`, and the detail page renders the date read-only. That confirmed there is no edit UI, so I used delete-and-recreate. This was the only code I looked at.
- Everything else was done from the screen. **Finding the Inquiries page** depended on the email alert's link (the nav has none). For Lindqvist, whose alert never arrived, I would not have found it without that email or the Guide's "Open inquiries →" link.
- API calls were read-only checks after the UI step: `/api/sessions`, `/api/clients`, and `/api/sessions/3/grocery` before and after opening the tab.

## Defects (exact repro)

**D1. The first grocery list cannot be ticked until you press Regenerate, and nothing says so.**
Steps: create a session, open its Grocery tab (do not press Regenerate), then click any item, press Space on it, or click "Mark All".
Expected: the item is ticked and saved. Actual: nothing changes. No request is sent and `aria-checked` stays false. `GET /api/sessions/:id/grocery` returns items with no `id` and no `is_checked`, so the screen shows a preview that was never saved. Ticking works after "↺ Regenerate", and the Guide's "Your ticks are saved" is true only after that.

**D2. Grocery quantities ignore the recipe's base servings and use the stored "Qty (4)/Qty (6)" columns literally.**
Steps: (a) Spicy Black Bean Chili (base 6, 3 can black beans) in a 6-serving session, Grocery tab: shows "black beans — 4 1/2 cans" and everything else 1.5×. (b) Roasted Vegetable Tray Bake (base 2, 2 zucchini) in a 4-serving session: shows "zucchini — 2" where 4 are needed. Expected: scale from base servings, as the recipe detail page's Serves +/− control does. This makes 6-serving shopping 50% too large, and 4-serving shopping of base-2 recipes 50% too small. It builds on leg-1 D1.

**D3. The client form saves negative household counts.**
Steps: Clients, a client, Edit, set Adults to `-1`, Save. Result: "Client updated", the API stores `adults: -1`, and the profile shows "-1 / 0". The browser shows no `min` hint and the server does not check the value.

**D4. The new-session wizard offers only 4 or 6 servings and ignores the client's serving size.**
Steps: create a client with Preferred Serving Size "2 servings" (or 8). "+ Session", then Next. Serving Size shows only "4 servings" / "6 servings", with 4 preselected. A 2-serving household cannot be booked correctly. Combined with D2 and the lack of session editing, the Lindqvist session is stuck at 4 servings.

**D5. Inquiry "Cuisines" answers are dropped when converting to a client.**
Steps: convert an inquiry with Cuisines filled in (for example Okafor "Rice bowls (kids love them), mild flavours"). The client profile and edit form have no cuisine field and the text is not copied into notes. Scheduling notes are copied into Chef Notes instead.

**D6. The feasibility total goes down when a dish is added, and it is computed from 0-minute recipes.**
Steps: in the session wizard menu step, select Bowls + Curry: "Total ~25 min". Add a third dish: "Total ~20 min". All recipes have 0 prep and 0 cook, yet the bar shows "GREEN" and the timeline puts every step at "0:15".

**D7. A grocery list does not show that it is out of date after a recipe change.**
Steps: generate a session's list, edit one of its recipes (garlic 4 to 5 clove), reopen the Grocery tab. It still shows the old totals, with no stale marker. Related: in the recipe editor, changing the base quantity of an existing row does not recalculate "Qty (6)" (it stayed "6 clove" for "5 clove"), so the stored 6-serving value is now wrong a second way.

## Usability friction

- **F1.** There is no Inquiries item in the navigation, and the Dashboard does not show new inquiries. The page `/intake` is reached only from the email alert's link or the Guide. On a server where email fails (the Lindqvist case), the chef has no in-app signal at all, although the app's own message says "Check this page for new inquiries".
- **F2.** A customer's later edit (Okafor: 10:00 → 10:30, "Sesame is fine.") has no "updated" marker or timestamp. The unchanged free-text note ("around 10:00") now contradicts the time field.
- **F3.** Convert uses a native browser `confirm()` dialog, which is unlike the app's own dialogs elsewhere (Delete Session, Rebuild list).
- **F4.** "Additional Dietary Notes" (in Dietary Profile) and "Chef Notes" (in Service Preferences) are the same field. Editing one changes the other.
- **F5.** "Hide allergen conflicts" is on by default. That is safe, but the false Dairy flag on coconut milk silently removes the curry for Castellano. The only hint is "Hiding 1 allergen conflicts".
- **F6.** "Repeat" makes the clone "Pending" even when the original was Confirmed, and nothing on the session page lets you change status.
- **F7.** Double-booking produces no warning when creating the session. The warning appears only afterwards, on the Calendar (banner + ⚠). The Sessions list and Dashboard do not show it.
- **F8.** Regenerate clears all ticks, even for unchanged items. The warning is honest, but a merge would be better.
- **F9.** Copy / Share text and the PDF do not show ticked items (they are neither marked nor left out), so a half-done shop cannot be shared as "what's left".
- **F10.** The Costing step leaves Ingredient Cost empty, which gives a "100%" margin on every session.
- **F11.** The calendar day buttons announce only the day number to assistive tech, not whether the day has sessions or a conflict.

## Missing features

- **X1.** Multi-session or repeat requests in an inquiry (Castellano's two Fridays) must be read from free text and booked by hand. Converting does not offer "create sessions".
- **X2.** **A saved session cannot be edited**: date, time, servings and menu are all fixed. Rescheduling means delete and recreate. Changing servings or swapping a dish, which the grocery workflow and the Guide ("Changed the menu? Regenerate…") assume, is impossible.
- **X3.** The menu step checks allergens only, not dietary style. Meat dishes are offered to a vegetarian client without any flag.
- **X4.** Weekly recurrence creates no future sessions on the calendar (Okafor shows only Dec 7). The next one appears only after "Mark as Completed".
- **X5.** No prep or cook-time prompt: recipes saved with 0 minutes produce a confident GREEN feasibility badge with no "times missing" hint.
- **X6.** On the no-email server copy, the chef cannot send Lindqvist a "view/update" link, except by "Try again" on a server that now has email (not tested).

## Misleading text

- **M1.** Lindqvist card: "⚠ Confirmation email failed" with a **Try again** button. The customer was told on purpose that "No confirmation email will be sent" (the :4311 copy has no mail). Calling it a failure, and offering to retry from the email-capable instance, contradicts what the customer was told.
- **M2.** Converting from a stale tab returns the existing client but still shows the toast "Client record created".
- **M3.** Converting changes the inquiry status to **"Booked"** before any session exists.
- **M4.** "GREEN — 25min", "~Time 25m", and timeline steps "(0 min active)" for recipes with no times entered.
- **M5.** The grocery PDF footer says "Prepared Monday, December 7, 2026" (the session date) instead of the date it was generated.
- **M6.** The Clients list says "Sessions through Dec 7, 2026" for a weekly recurring client.
- The Guide's "Tap each item as it goes in the cart. Your ticks are saved." is true only after Regenerate (D1).
