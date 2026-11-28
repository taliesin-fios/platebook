# Customer tester notes: booking Juniper Table Kitchen (Robin Hale)

Rehearsal date: 2026-11-24. Tester role: three fictional households. I used only the public booking link, the "view or update" link from the confirmation email, and the local mail sink ("my inbox"). I did not use chef screens or app source.
Browser: Playwright Chromium, fresh context per household. Desktop 1280x800 (deviceScaleFactor 2). Lindqvist used phone emulation at 375x812 (isMobile, hasTouch). This is browser emulation, not a physical phone.
Signed links appear below as `token=<REDACTED>`.

## What the booking page tells a customer (before submitting)

- **Deposit / cancellation:** a "Booking Terms" box above the submit button: "A $50 deposit holds your date and is credited to your first invoice. Groceries are billed at cost. Please give 48 hours notice to cancel or reschedule; deposits for later cancellations are non-refundable." There is a required checkbox: "I have read and agree to the booking terms above…".
- **Payment methods:** not mentioned on the form. They appear only after submitting: "Your chef will be in touch to arrange payment (Venmo, Zelle, or check)."
- **Paying a deposit online:** not possible. No pay button, no card field, no payment link, either on the page or in the email. Payment is arranged manually by the chef. This is expected because the server runs with manual payments.
- **Response time:** not stated on the form. The submitted page says "will be in touch within **1 business day**". The confirmation email says "within **1–2 business days**". The two disagree (see Misleading text).
- **Reply-To:** the customer confirmation emails carry `Reply-To: hello@juniper-table.example.com`. From is `"Platebook Rehearsal" <noreply@example.test>`. Hitting Reply would therefore reach the chef's business address. The email body itself gives no chef email address or phone number.

## 1. Okafor Household (Ada Okafor, ada.okafor@example.com), on :4310

**Goal:** a weekly Monday ~10:00 session for 2 adults and 2 children (4 servings). One child has a serious peanut and tree-nut allergy. Mild spice, rice bowls, no mushrooms.

| Step | What I did | What I saw | Outcome |
|---|---|---|---|
| 1 | Clicked "Send Booking Request" with everything empty | The page stayed put and showed inline red errors: "Your name is required", "Email is required", "Please accept the terms to continue". Fields got red outlines. Nothing was sent (no request). `01-okafor-empty-submit.png` | Pass |
| 2 | Filled the form (Small Family, 2+2, 4 servings, Peanuts + Tree Nuts + Nut-Free Kitchen Required, Mild, Monday, 10:00, notes about the allergy and weekly wish). Email typed as `ada.okafor@example`. Submitted | "Enter a valid email" under the email field. The rest of the form was kept. `02-okafor-bad-email.png` | Pass |
| 3 | Tried the plausible typo `ada.okafr@example.com` (not submitted), then corrected it to `ada.okafor@example.com` | The typo is a valid-looking address, so nothing would catch it. There is no "confirm email" field and no review step | Friction (only the submitted page echoes the address) |
| 4 | Double-clicked Submit, then tried a third click | The third click was impossible because the button disappeared or was disabled at once. Page: "You're on your way! … reference #1. We'll email a confirmation to ada.okafor@example.com; if it doesn't arrive, your chef still has your request." `03-okafor-submitted.png` | Pass |
| 5 | Checked inbox | **Exactly one** email to me (#1). Subject: "Your booking request has been received — Juniper Table Kitchen". Reply-To: hello@juniper-table.example.com. Attachments: none. Body: greeting, "View or update your request: http://localhost:4310/intake/1?token=<REDACTED>", "Your chef will be in touch within 1–2 business days.", signed "Juniper Table Kitchen". `04-okafor-inbox-confirmation.png` | Pass |
| 6 | Opened the view/update link | A prefilled edit page showed my answers. Allergen chips Peanuts / Tree Nuts / Nut-Free Kitchen were visibly selected. The URL bar shows the token, but the page body does not. `05-okafor-edit-page-allergens.png` | Pass |
| 7 | Added "Sesame is fine." to Notes, changed Preferred Time 10:00 → 10:30, clicked "Update My Preferences →" | "Preferences updated! Your changes have been saved… Your intake is saved — reference #1." `06-okafor-update-saved.png` | Pass |
| 8 | Reloaded the link | The notes ended with "Sesame is fine." and the time showed 10:30. `07-okafor-edit-after-reload.png` | Pass |
| 9 | Checked inbox after the update | No email confirming the update | Friction (minor) |

- **Reference number:** #1. **Confirmation emails received:** 1 (no attachments).
- **Duplicate-submission result:** no duplicate. The double-click produced one reference number (#1) and one confirmation email. A diagnostic check of the browser's network log showed a single `POST /api/intake` (201).
- **Weekly recurrence:** the form has no "weekly / recurring" option. I wrote "every Monday" in Scheduling Notes, so it is free text only.

## 2. Lindqvist Household (Sven Lindqvist, sven.lindqvist@example.com), on :4311 (no email configured), phone emulation

**Goal:** 2 adults, 2 servings, vegetarian, strict celiac. Loves roasted vegetables and chickpeas. Wednesday early afternoon.

| Step | What I did | What I saw | Outcome |
|---|---|---|---|
| 1 | Opened http://localhost:4311/book/robin-hale at 375x812 | Same form as :4310. No horizontal scroll (scrollWidth 375 = viewport) before filling, after filling, or on the submitted page. Text was not cut off. `08-lindqvist-mobile-form-top.png` | Pass |
| 2 | Filled it in by tapping (Couple, 2/0, 2 servings, Gluten allergen, Vegetarian + Gluten-Free, Plant-Based Only, Medium, Wednesday 13:00) | Chips and option buttons are all 44 px tall and easy to tap. The selected state is clear (filled green). `09-lindqvist-mobile-dietary.png` | Pass |
| 3 | Terms checkbox | The checkbox itself is only 20x20 px, but tapping its label text also ticks it. `10-lindqvist-mobile-terms.png` | Pass (small target, mitigated by label) |
| 4 | Submitted | Page text: **"Your intake is saved — reference #2. No confirmation email will be sent, so keep this reference number."** Above that: "…will be in touch within 1 business day to confirm your booking." `11-lindqvist-mobile-submitted.png` | Pass |
| 5 | Checked inbox | **Nothing** arrived for sven.lindqvist@example.com (inbox still held only messages #1–#2 at that point) | Page statement was true |

- **Reference number:** #2. **Confirmation emails:** 0.
- **No-email copy result:** the page said exactly "No confirmation email will be sent, so keep this reference number." That was accurate: no mail arrived. However, Sven also gets **no way to view or update** his request later, because the edit link only travels by email. And the only record he has is the on-screen number, which he must note down himself (missing feature).
- **Layout problems at 375px:** none found. The only small tap target is the 20 px terms checkbox, and its label is tappable. The "Platebook" footer link is small (51x14), which is unimportant.

## 3. Castellano Household (Lucia Castellano, lucia.castellano@example.com), on :4310

**Goal:** 4 adults and 2 teenagers, 6 servings, dairy-free, spicy, large portions. Friday ~15:00, **plus a repeat booking the following Friday**.

| Step | What I did | What I saw | Outcome |
|---|---|---|---|
| 1 | Looked for a way to request a specific date, a second date, or a repeat | The form only has "Preferred Day" (weekday dropdown) and "Preferred Time". There is no date picker, no "repeat / recurring" option and no "add another session" control | Missing feature |
| 2 | Filled the form (Large Family, 4/2, 6 servings, Dairy + Dairy-Free, Bold, Friday, 15:00). Wrote in Scheduling Notes: "We would like to book this Friday AND a repeat session the following Friday as well (two sessions)." Same request in Notes for Your Chef. `12-castellano-repeat-in-notes.png` | Accepted | Friction (workaround through free text) |
| 3 | Submitted | "You're on your way! … reference #3. We'll email a confirmation to lucia.castellano@example.com…" Deposit box says "A $50 booking deposit holds your session date" (singular). It is unclear whether two sessions need $50 or $100. `13-castellano-submitted.png` | Pass, with ambiguity |
| 4 | Checked inbox | **One** email to me (#4). Subject "Your booking request has been received — Juniper Table Kitchen", Reply-To hello@juniper-table.example.com, no attachments. It contains a view/update link (`/intake/3?token=<REDACTED>`). It does not repeat what I asked for, so nothing confirms the two Fridays. `14-castellano-inbox-confirmation.png` | Pass / friction |

- **Reference number:** #3. **Confirmation emails:** 1.
- **Outside Platebook:** the repeat booking can only be expressed as a free-text note. To be sure the second Friday is held, Lucia would have to reply to the confirmation email (Reply-To reaches hello@juniper-table.example.com), or text or call Robin. No phone number is shown anywhere to the customer, so that means email.

## What had to happen outside Platebook

- Castellano: asking for a repeat booking the following Friday. It was typed into notes, and getting it confirmed needs an email reply to Robin. Nothing in Platebook tells the customer whether both dates are held.
- Okafor: "weekly, every Monday" was likewise free text only.
- All households: paying the $50 deposit (Venmo, Zelle or check, arranged by the chef). There is no in-app payment.
- Lindqvist (no-email copy): keeping the reference number by hand. Any later change to his request must go to Robin directly. He has no edit link, and the page gives no chef contact details.

## Needed help or code knowledge

None. I made two diagnostic observations from the browser's own network log while driving the UI; no direct API calls. One confirmed a single POST on the double-click. The other noticed that the public page calls `/api/auth/me` and `/api/settings`, which return 401, on every load.

## Defects (with reproduction)

1. **Selected allergen, diet and protein chips are not announced to assistive tech.** Repro: open /book/robin-hale and click "Peanuts". The chip turns green, but it has no `aria-pressed` (stays null). By contrast "Mild" and "Small Family" do report `aria-pressed=true`. A screen-reader user cannot tell which allergens are selected. That matters for a serious nut allergy.
2. **Form fields have no accessible names tied to their labels.** Repro: inspect the accessibility tree of /book/robin-hale. The text fields are named only by their placeholder (e.g. "e.g., The Johnson Family", "you@example.com"). Both dropdowns (serving size, preferred day) and the Preferred Time field have no name at all. Their visible labels are not associated with them.
3. **Public booking page makes failing authenticated calls.** Repro: load /book/robin-hale with devtools open. `GET /api/auth/me` and `GET /api/settings` return 401 on every load (also on /intake/N). The customer sees nothing, but they are noise in the console/logs. Low severity.

## Usability friction

- No "confirm email" field and no review step. A plausible typo (`ada.okafr@example.com`) passes validation, so that customer would never get the confirmation or edit link. Only the submitted page echoes the address.
- Payment methods (Venmo/Zelle/check) are revealed only after submitting, not next to the deposit terms the customer agrees to.
- The confirmation email does not summarize the request (day/time, allergens, servings) or mention the deposit or payment. The customer has to open the link to check anything.
- No email confirms an update made through the edit link. The page confirms on screen only.
- The "Preferences updated!" page has no link back to the form to make another change. The customer must reopen the email link.
- The name field "Full Name / Family Name" leads to awkward greetings ("Hi Castellano Household (Lucia Castellano)"). This is partly my input, but there is no separate contact-name field.
- Terms checkbox is 20x20 px on mobile (label is tappable, so this is mild).
- The edit page shows the deposit box but not the cancellation terms.

## Missing features

- No way to request a specific date, a recurring (weekly) schedule, or a repeat or second session. Only a weekday and a time.
- No online deposit payment (expected with PAYMENTS_MODE=manual, but the customer has no self-service path).
- On a server without email, no way to view or update a request later. The edit link exists only in email, and the reference number alone gives no access.
- No chef contact details (email or phone) shown to the customer on the page or in the email body. Contact relies on replying to the email.
- The deposit wording does not say how multi-session requests are handled.

## Misleading text

- **Response time is inconsistent:** the submitted page says "within 1 business day", while the confirmation email says "within 1–2 business days".
- "Booking Request" / "confirm your booking" language, together with the "$50 deposit holds your date" text, suggests a date is being held. The form never asks for a date (only a weekday), so no date is actually reserved.
- "Your information is private and only shared with your assigned chef" next to the address hint "Used to estimate travel — not shared." These are consistent but slightly redundant; not really misleading.
- The no-email copy's statement ("No confirmation email will be sent, so keep this reference number") was **accurate**.

## Screenshots (docs/rehearsal/evidence/customer/screenshots/)

01-okafor-empty-submit.png, 02-okafor-bad-email.png, 03-okafor-submitted.png, 04-okafor-inbox-confirmation.png, 05-okafor-edit-page-allergens.png, 06-okafor-update-saved.png, 07-okafor-edit-after-reload.png, 08-lindqvist-mobile-form-top.png, 09-lindqvist-mobile-dietary.png, 10-lindqvist-mobile-terms.png, 11-lindqvist-mobile-submitted.png, 12-castellano-repeat-in-notes.png, 13-castellano-submitted.png, 14-castellano-inbox-confirmation.png
