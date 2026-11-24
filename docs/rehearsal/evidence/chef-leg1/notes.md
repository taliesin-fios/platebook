# Chef tester, leg 1: Robin Hale (Juniper Table Kitchen)

Date: 2026-11-24. App: http://localhost:4310 (production-mode instance), driven with Playwright (Chromium, 1280x800, deviceScaleFactor 2).
Screenshots are in `screenshots/`.

## Key results (checked after reload)

- **Booking URL for customers: `http://localhost:4310/book/robin-hale`**
- **Public slug: `robin-hale`.** It comes from the login email, not the business name, and Settings has no way to change it.
- Booking page heading: "Juniper Table Kitchen", subtitle "Robin Hale · Riverside (fictional)". It shows the booking terms and a checkbox to agree to them.
- Settings saved: business email `hello@juniper-table.example.com`, time zone Central (Chicago) / `America/Chicago`, deposit $50, cancellation notice 48 h, payment methods "Venmo, Zelle, or check", response time "within 1 business day", default service fee $175, my own booking terms text, services "Cooking in clients' homes" and "Weekly meal prep".
- Starter content: I **declined** both the starter recipe library and the sample client. The library shows only my 5 recipes and there are 0 clients. (API check: `/api/clients` returns `[]`.)
- Recipes saved. I checked them after reloading the list and the detail pages. Ingredient counts were also confirmed with a read-only API check.

| # | Recipe | Base serves | Ingredients | Allergens the app found |
|---|---|---|---|---|
| 1 | Chickpea & Spinach Curry | 4 | 6 | **Dairy** (wrong: it comes from coconut milk) |
| 2 | Garlic Lemon Chicken Rice Bowls | 4 | 6 | none |
| 3 | Roasted Vegetable Tray Bake | 2 | 5 | none |
| 4 | Spicy Black Bean Chili | 6 | 6 | none |
| 5 | Satay Noodles (contains peanuts) | 4 | 4 | **Gluten, Peanuts** (correct: from noodles and peanut butter) |

## Chronological log

| # | Goal | What I tried | What the screen showed | Outcome | Screenshot |
|---|---|---|---|---|---|
| 1 | Account | Opened `/`. It redirected to `/welcome`, a marketing page. Clicked "Create an account on this server". | Signup form with fields Business name, Email, and Password ("At least 10 characters"). Business name has no asterisk. | pass | - |
| 2 | Account (mistake 1) | Left Business name empty, entered email and password, and submitted. | **The account was created** (POST 201) and the app went straight to onboarding step 1, where Business name * is empty. | friction / possible defect (see F1, D4) | 01-signup-without-business-name-accepted.png |
| 3 | Account (mistake 2) | New signup form with a short password, `juniper1` (8 characters). | Inline error: "Password must be at least 10 characters." The browser also blocks it with minlength=10. Clear. | pass | 02-signup-short-password.png |
| 4 | Account (mistake 3) | Same form with a valid password and the already-used email. | "An account with that email already exists. Try signing in." Clear (it does reveal that the account exists). | pass | 03-signup-duplicate-email.png |
| 5 | Recover | Signed in at `/login` with the fixture credentials. | Taken back to onboarding step 1. | pass | - |
| 6 | Onboarding 1/5 | Clicked Continue with the business name empty. | "Please enter your business name." I then entered Juniper Table Kitchen, Robin Hale, and Riverside (fictional). | pass | 04-onboarding-step1-business.png |
| 7 | Onboarding 2/5 | Picked "Cooking in clients' homes" and "Weekly meal prep", kept 3 h and 4 servings, set the service fee to 175. | Saved. | pass | 05-onboarding-step2-how-you-work.png |
| 8 | Onboarding 3/5 | Set deposit $50, payment methods "Venmo, Zelle, or check", and my own booking terms. The terms box starts empty. | Saved. The step says "Shown to clients on your booking form before they submit." (see M1) | pass | 06-onboarding-step3-booking-payments.png |
| 9 | Onboarding 4/5 | "Get a head start": Starter recipe library (20 recipes and 4 templates) and A sample client are **both pre-checked**. I unchecked both, as Robin wants their own library and real households. The button label changed from "Add & continue" to "Continue". | Nothing was seeded. | pass | 07-onboarding-step4-declined-starter-content.png |
| 10 | Onboarding 5/5 | "You're ready to cook" page. | Booking link `http://localhost:4310/book/robin-hale` with Copy and "Preview it" buttons. | pass | 08-onboarding-step5-booking-link.png |
| 11 | Onboarding | My script reloaded `/onboarding` mid-flow, and later left from step 5 without clicking "Go to my dashboard". | Each reload went back to **step 1**. Values from earlier steps were kept. Going to `/` kept redirecting to onboarding until I walked all 5 steps again and clicked "Go to my dashboard" (POST /api/onboarding/complete). On the second pass the step-4 boxes were **checked again by default**. | friction (F2, F3) | - |
| 12 | Dashboard | Dashboard opened with a 5-step tour (Dashboard, Clients, Sessions, Recipes, Invoices). | All counts are zero. A "Your Booking Link" card shows the URL and a Copy button. | pass | 09-dashboard-first-view.png |
| 13 | Business setup | Looked in Settings for business email and time zone, which onboarding never asked for. Entered `hello@juniper-table.example.com`, picked Central (Chicago), set response time, clicked Save Settings. | "Settings saved". After reload every value was still there. | pass (onboarding gap: F4) | 10-settings-overview.png, 11-settings-saved.png, 12-settings-after-reload-booking-section.png |
| 14 | Booking link | Opened the booking URL in a logged-out browser. | Full booking form with contact, household, allergens, dietary style, scheduling, kitchen, and terms checkbox. The deposit amount, cancellation hours and payment methods appear **only where I had typed them into the terms text**. | pass / misleading (M1) | 13-public-booking-page.png |
| 15 | Recipes | Recipes page in its empty state: "No recipes found". The only action is a "+" floating button, which offers Menu Planner, Import Recipe, and Add Manually. | Manual form: name, category (default "Couples"), session fit, prep/cook, base serves (4), hold and reheat stars (default 4), chef notes, key overlap ingredients, and an ingredient table with columns Name, Qty (base), Qty (6), Section. There is **no allergen field**. | pass / friction | 14-add-recipe-form-empty.png |
| 16 | Recipes (mistake) | Clicked Save with no name. | Red toast: "Name is required". | pass | 15-recipe-save-without-name.png |
| 17 | Recipe 1 | Entered the curry: quantity and unit go in one text field ("2 cup"), and tabbing out fills in the "Qty (6)" column. | **Every ingredient defaulted to section "Produce"**, including chickpeas, olive oil and coconut milk. Saved. The detail page showed "Allergens detected in ingredients: **Dairy**". | pass / friction (F5) / defect (D2) | 16-recipe1-form-filled.png, 17-recipe1-saved.png |
| 18 | Recipe 1 fix | Edit, then fixed the sections by hand and saved. | Saved. Edit opens in place at the same URL. | pass | 25-curry-coconut-milk-flagged-dairy.png |
| 19 | Recipes 2-5 | Added each recipe and set store sections by hand. | All saved. Only Satay was flagged (Gluten, Peanuts). **For recipe 3 (base 2) the second column was still headed "Qty (6)" but held 1.5x the base; for recipe 4 (base 6) the header read "Qty (6) / Qty (6)" with 3 can vs 4 1/2 can.** | pass / defect (D1) | 18-…, 19-…, 20-recipe4-chili-saved.png, 20a-recipe4-chili-form-base6.png, 21-recipe5-satay-saved.png |
| 20 | Persistence | Reloaded the library, the tray bake page, the Satay page and the dashboard. | All 5 recipes were still there with the same ingredients, sections and allergen flags. On the detail page, scaling the tray bake from 2 to 6 servings worked (zucchini 2 to 6). Reopening Edit on the chili still showed the wrong "Qty (6)" values, so they are saved. | pass (D1 confirmed persisted) | 22-recipe-library-after-reload.png, 23-tray-bake-scaled-to-6-after-reload.png, 24-satay-allergens-after-reload.png, 26-chili-edit-qty6-column-wrong.png, 27-dashboard-after-reload.png |
| 21 | Library | Searched "peanut" and "chickpeas". | "peanut" finds Satay because the word is in its name. "chickpeas" finds **No recipes**, so search does not look at ingredients. | missing feature (X3) | - |
| 22 | Email | Read the Settings page for email. | Business Email: "Shown to clients and used for new-booking alerts." Reminder Emails: "Sent automatically about 24 hours before…" with two opt-in checkboxes, both off. **Nothing says whether this server can actually send email.** The mail sink had 0 messages after signup, so no welcome or verification mail is sent. | missing feature (X1) | 10-settings-overview.png |

## Needed help / code knowledge

None. Everything was done from the screen. API calls were used only as read-only checks after the UI step: the recipe list, recipe details with ingredient counts, and the client list. Those checks also showed that ingredients are stored as `qty_4_serving` / `qty_6_serving`, which explains D1.

## Defects

**D1. The edit form's second quantity column ignores the recipe's base servings, and the wrong values are saved.**
Steps: Recipes, +, Add Manually. Set Base Serves to 2 (or 6). Add an ingredient "zucchini" with Qty "2" and tab out. Save, then open Edit again.
Expected: the second column holds the amount for 6 servings (6 for base 2; equal to the base amount for base 6).
Actual: it is always 1.5x the base amount (3 for base 2; "4 1/2 can" for a 6-serving recipe with "3 can"). The header reads "Qty (2) / Qty (6)" or "Qty (6) / Qty (6)". The values are saved as `qty_6_serving`.
Scaling on the detail page is correct. Later legs should check whether grocery lists for 6-serving households use this stored column (Castellano household, serving size 6).

**D2. Coconut milk is flagged as the Dairy allergen.**
Steps: create a recipe with the ingredient "coconut milk" and save. The detail page shows "⚠ Allergens detected in ingredients: Dairy".
Coconut milk is dairy-free. For the dairy-free Castellano household this false positive would wrongly block or warn about a suitable dish. There is no way to correct the flag (X2).

**D3. Accessibility: several Settings fields have no accessible label.**
Business Name, Phone, Booking Deposit, Cancellation Notice, Default Session Window, Serving Size and Service Fee are exposed with no name. Business Email and others are named only by their placeholder (Business Email is announced as "robin.hale@example.com"). Seen in the aria snapshot of `/settings`. The onboarding forms are labelled correctly.

**D4. Signup accepts an empty business name, and the public slug then uses the login email.**
Steps: `/signup`, leave Business name empty, fill email and password, click Create account. Result: 201, account created.
The business name is added later in onboarding, but the slug stays `robin-hale` (from the email's local part) and cannot be changed (X4). It is not clear whether this is intended. Either way the signup field gives no sign that it is optional or required.

## Usability friction

- **F1.** The signup "Business name" field is not marked required, yet onboarding requires it.
- **F2.** Reloading onboarding always restarts at step 1. Leaving from step 5 without clicking "Go to my dashboard" leaves onboarding unfinished, and every page then redirects back into it.
- **F3.** Starter recipes and the sample client are pre-checked, and they are checked again each time step 4 is shown. A chef who walks through onboarding twice can seed sample data by accident.
- **F4.** Onboarding never asks for the business email (used for client Reply-To and booking alerts) or the time zone (used for "today" and overdue). Both stay at defaults (login email, UTC) unless the chef finds them in Settings.
- **F5.** Every new ingredient defaults to the "Produce" store section, even olive oil, canned chickpeas, coconut milk, chicken and chili powder. Each one has to be fixed by hand, or the "sorted by store aisle" grocery list will be wrong.
- **F6.** New recipes default to category "Couples" and to 4/5 hold and reheat stars. My 6-serving chili is now a "Couples" recipe unless I notice.
- **F7.** The empty recipe library shows only "No recipes found" and a small "+" button, with no "Add your first recipe" call to action.
- **F8.** Recipe cards in the library show only name, category and "0 min total". There are no servings, ingredient count or allergen badges.
- **F9.** Quantity and unit share one free-text field ("2 cup"). That works, but nothing checks units, which matters when the grocery list merges "garlic" across recipes.
- **F10.** Shared ingredients (onion, garlic, olive oil, chickpeas, rice) are not linked across recipes. "Key Overlap Ingredients" has to be typed in by hand for each recipe.
- **F11.** "Clear Sample Data" appears in Settings even though I declined sample data. It is unclear what it would remove.
- **F12.** Public pages (`/welcome`, `/signup`, `/book/robin-hale`) log two 401 console errors (`/api/auth/me`, `/api/settings`) for logged-out visitors. Nothing visible, just noise.

## Missing features

- **X1.** No email status in Settings: nothing says whether the server can send mail or where alerts go, and there is no "send test email".
- **X2.** No way to set, add to or override a recipe's allergens. Detection is automatic and read-only, so false positives (D2) and misses cannot be corrected.
- **X3.** Recipe search does not match ingredient names.
- **X4.** The booking-link slug cannot be edited. It is `robin-hale` rather than something like `juniper-table-kitchen`.
- **X5.** No automatic store-section guess for common ingredients (see F5).

## Misleading text

- **M1.** Onboarding step 3 ("Booking & payments") says: "Shown to clients on your booking form before they submit." The booking form does not show the deposit amount, the cancellation hours, or the payment methods ("Venmo, Zelle, or check" does not appear on `/book/robin-hale`). It shows only the free-text booking terms, so the policy appeared only because I had written it into the terms myself.
- **M2.** The welcome page says new inquiries "land in your inbox", while the onboarding finish page says requests "will land in your Inquiries". It is unclear whether the chef gets an email. The customer leg will check.
- **M3.** Settings says "Reminder Emails … Sent automatically", with no hint about whether this server can send email (see X1).
- **M4.** The edit form's "Qty (6)" column header is wrong whenever the base is not 4 (see D1).
- Not checked: the welcome-page claim "AI recipe import: Paste a link or any recipe text…". I configured no AI provider, as instructed.
