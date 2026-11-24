# Defects found in the rehearsal

These were found by AI testers driving a real browser against the fictional week. The skeptical
observer verified the high-priority ones against screenshots, data or source code.

**Fixed** items were found during the rehearsal and fixed in this branch. Everything else is
**deferred**: it is unrelated to the email fix and not changed here.

Priority: **P1** wrong data, privacy, or a core task blocked. **P2** a real defect with a workaround.
**P3** minor.

Missing features and friction are listed separately at the end.

## Fixed in this branch (found during the rehearsal or review)

| ID | Defect | Fix | Evidence |
|---|---|---|---|
| E1 | Rapid clicks sent duplicate emails: meal plan 5×, invoice 2× | The browser ignores clicks while a send runs. The server refuses a repeat of a successful send within 10 s (`EMAIL_JUST_SENT`). | chef-leg3 D1; logs/21 (4/4 rounds, one message each) |
| E2 | A paid invoice kept "email failed" and Try again, which would re-send "Amount due" | Banner and badge show only for Draft or Sent. The server refuses to email paid or void invoices (`INVOICE_CLOSED`). | chef-leg3 D3; cargo `a_paid_or_void_invoice_is_never_emailed` |
| E3 | An unreachable server was reported as "didn't accept" | Now "couldn't be reached or didn't accept" | chef-leg3 M1 |
| E4 | Inquiry badges said "Confirmation email failed" when email was off on purpose | Now "…not sent". On a no-email server the banners drop Try again. | chef-leg2 M1; demo check 23 |
| E5 | Booking page said "Confirmation sent to …" before the outcome was known | "We'll email…" or "No confirmation email will be sent" | review finding; cargo `the_booking_response_says…` |

## P1

**P1-1. `/intake/<anything>` serves account #1's booking form, and a submission from it goes to account #1.**
Found by leg 3 (D6). Confirmed by the observer from `IntakeForm.tsx`, `settings.rs:272` and `intake.rs:7`.
- Reproduce: on a server with two or more accounts, open `/intake/999` (or any id, or `abc`) without a token. Account #1's business name and booking form appear.
- The observer, reading the code, says a submission leaves out the slug and so lands in account #1. The submission was not actually sent in the rehearsal.
- Risk: a customer following a mistyped or stale link gives another chef their household and allergy details.
- Suggested fix: have `/intake/:id` without a valid token show "link expired" (or redirect to `/book/<slug>`), and stop treating a missing slug as account #1.
- Evidence: chef-leg3/screenshots/63; observer audit.

**P1-2. Serving quantities: a fixed 4/6 model behind a field that looks like free scaling.**
One root cause with three symptoms:
- **(a)** The recipe editor's second column is always 1.5× the base amount and is headed "Qty (6)" whatever the base. A 6-serving chili with 3 cans saves 4½ cans "for 6".
- **(b)** The grocery list takes the 4-serving column below 5 servings and the 6-serving column at 5 or more, with no scaling. The 6-serving chili comes out at 4½ cans instead of 3, a 2-serving tray bake at half what's needed, and 8 servings would get the 6-serving amounts.
- **(c)** The session wizard offers only 4 or 6 servings, so a 2-person household can't be booked correctly.

Reproduce:
1. Create a recipe with base 6 and "3 can black beans".
2. Book a 6-serving session with it.
3. Open the Grocery tab.

Evidence: chef-leg1 D1 and screenshot 26; chef-leg2 D2, D4 and screenshots 24, 44, 45; observer audit (`RecipeDetail.tsx:467`, `grocery.rs:179,211`, `SessionNew.tsx:385`).

**P1-3. The client meal-plan PDF contains invented food-storage instructions.**
- Any dish with "bowl" in its name gets "Warm meatballs separately, serve over cold grain".
- Dishes are marked FREEZE by default, and "Use by" is the session date + 4 days.
- The storage box always warns about yogurt sauces and cucumber.
- The recipe editor has no field to correct any of this.

Reproduce: download the meal plan for a session containing "Garlic Lemon Chicken Rice Bowls".
Evidence: chef-leg3 D2 and okafor-meal-plan-pdf.txt; demo-run/meal-plan-okafor.txt (reported as a KNOWN DEFECT in each run); `pdf/docs.rs:72-135`, `meal_plan.typ:97-99`.

**P1-4. A saved session can't be edited in the app.**
- Date, time, servings and menu are read-only. Rescheduling means deleting and recreating the session.
- The API already supports `PUT /api/sessions/:id` with these fields.
- The Guide promises "Changed the menu? Regenerate…".

Evidence: chef-leg2 X2; observer audit.

**P1-5. New inquiries are hard to find.**
- There is no Inquiries item in the side or bottom navigation, and no count on the dashboard.
- `/intake` is reached through the alert email's link or the Guide.
- When email fails, leads can go unseen, even though the app's own banner says "Check this page for new inquiries".

Evidence: chef-leg2 F1 (re-rated from friction); observer audit.

## P2

| ID | Defect | Reproduce | Evidence |
|---|---|---|---|
| P2-1 | Coconut milk (likewise oat, almond and soy milk, per the code) is flagged as Dairy, with no override. The curry is silently hidden for the dairy-free household. | Add a recipe with "coconut milk" | chef-leg1 D2 and screenshot 25; chef-leg2 screenshot 27 |
| P2-2 | No double-booking warning when creating a session; only the Calendar shows the conflict afterwards | Create two sessions at the same date and time | chef-leg2 F7, screenshots 35–38 |
| P2-3 | The first grocery list is an unsaved preview that can't be ticked until Regenerate is pressed | New session → Grocery → tick | chef-leg2 D1 |
| P2-4 | Deleting a client leaves their upcoming sessions active; the dialog mentions only past sessions | Delete a client that has a future session and let Undo expire | chef-leg3 D4, screenshot 54 |
| P2-5 | Clients can be deleted only by swiping on touch; the hidden buttons still take keyboard focus | Desktop → Clients | chef-leg3 D5 |
| P2-6 | The dashboard counts never-sent Draft invoices as outstanding | Create a draft invoice | chef-leg3 M4 |
| P2-7 | Response time is hard-coded and inconsistent: the page says "within 1 business day", the email says "1–2 business days" (`email.rs` hard-codes the text, which breaks the CLAUDE.md branding rule) | Submit a booking | customer notes; customer screenshot 11 |
| P2-8 | Accessibility: unlabelled Settings and booking-form fields; allergen and diet chips don't announce they're selected (no `aria-pressed`) | Accessibility tree of `/settings` and `/book/<slug>` | chef-leg1 D3; customer D1–D2 |
| P2-9 | Invoice email and PDF leave out "How to pay"; only the web view shows it | Email an invoice | chef-leg3 F2, X2 |
| P2-10 | A grocery list doesn't show it's out of date after a recipe change; Regenerate clears every tick | Generate a list, edit a recipe, reopen the list | chef-leg2 D7, F8 |
| P2-11 | Opening a session or recipe you can't access (deleted, or another account's) shows a loading skeleton forever | As a second chef, open `/sessions/<id>` | chef-leg3 F8 |

## P3

- Negative household counts can be saved (adults = -1).
- The feasibility badge shows "GREEN" for recipes with 0-minute times, and the total drops when a dish is added.
- "Repeat" creates the copy as Pending even when the original was Confirmed, and nothing lets you change it.
- The invoice due date (issue date + 14 days) can fall before the service date.
- The client invoice link works for Drafts without marking them as drafts.
- Public pages log 401s from `/api/auth/me` and `/api/settings`.
- Inquiry "Cuisines" answers are dropped when converting to a client.
- "Client record created" toast appears when an existing client is returned.
- An inquiry shows "Booked" before any session exists.
- The grocery and meal-plan PDF footers show the session date as if it were the date prepared.
- Client emails greet "Hi Okafor Household (Ada Okafor)".
- The signup business-name field isn't marked as required, and the slug comes from the email address and can't be changed.
- Every ingredient defaults to the Produce aisle, and every recipe to the "Couples" category.

## Email: known limits (not defects of the fix, but worth knowing)

- Two tabs sending within the same few milliseconds could both send; the 10 s window covers sequential repeats only.
- Temporary failures (451) aren't queued for automatic retry; the chef presses Try again.
- An `EMAIL_HOST` that isn't a valid host name reports email as available until the first send fails.
- In dev capture mode (`EMAIL_CAPTURE=true`), "sent" timestamps are still recorded.
- Sign-in and reset emails still in progress are dropped if the server shuts down.
- At debug log level (`tower_http=debug`), the request trace includes `/api/auth/verify?token=…`. This was already the case before this work; the default `info` level doesn't log it.

## Missing features (not defects)

- Customers can't request a specific date, a recurring schedule or a second session; this must be arranged outside Platebook.
- On a server with no email, customers have no way to view or edit their request later.
- No chef contact details are shown to customers.
- No dietary-style check (meat is offered to a vegetarian client); allergens only.
- Weekly recurrence doesn't put future sessions on the calendar.
- No prep or cook-time prompt, no make-ahead or packing list.
- "Mark as Paid" has no method, reference or date.
- No CSV export for invoices, clients or sessions; no trash or restore after Undo expires.
- No allergen override on recipes; recipe search doesn't match ingredients; the booking slug can't be edited.
- No email status or test-send in Settings.
- Online deposits weren't tested: the rehearsal ran with `PAYMENTS_MODE=manual`. A Stripe path exists.

## Friction (selected)

- Onboarding never asks for a business email or time zone; starter content is pre-checked and re-checked when you return to that step; a reload restarts onboarding.
- Converting an inquiry uses a native `confirm()` dialog.
- "Additional Dietary Notes" and "Chef Notes" are the same field.
- On phones, toasts cover the bottom action bar.
- There's no "I sent it myself" option to dismiss a failure banner after downloading the PDF.

The full lists are in each tester's `evidence/*/notes.md`.
