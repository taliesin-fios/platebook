# Evidence matrix

Result values: **Pass**, **Fail** (a defect remains; see DEFECTS.md), **Fixed** (failed in the
rehearsal, then fixed and retested), **Partial**, **Blocked/Untested**.

Evidence paths are relative to `docs/rehearsal/evidence/`. "cargo:" means a test in
`server-rs/tests/`. "demo" means `demo-run/results.json` (run at `dec3af1`).

## A. Email truthfulness (the fix)

| # | Scenario | Expected | Observed | Result | Evidence |
|---|---|---|---|---|---|
| A1 | No SMTP: send invoice and meal plan (unfixed code) | Should fail | `200 {"success":true}` and nothing sent; invoice moved to Sent | Fail before the fix | logs/01-red-email-regression.txt |
| A2 | No SMTP: send invoice and meal plan (fixed) | 503, invoice stays Draft, no `emailed_at`, failure recorded | As expected | Pass | cargo: email_delivery `without_email_chef_sends_…`; demo check "no email configured…" |
| A3 | No SMTP: booking submitted | Booking saved; confirmation recorded as not sent; page doesn't promise mail | Saved (ref #2); page says "No confirmation email will be sent"; nothing arrived | Pass | customer/notes.md (Lindqvist); customer/screenshots/11; cargo: `without_email_a_booking_still_saves…` |
| A4 | No SMTP: reminders | Not marked sent | `sent: 0`, markers null (unfixed code: `sent: 2`) | Pass | logs/01; cargo: `without_email_reminders…` |
| A5 | Partial settings (user without password; invalid From) | Treated as no email | 503 `EMAIL_NOT_CONFIGURED`; link pages say email unavailable | Pass | cargo: `unusable_smtp_settings_count_as_no_email`; config unit tests |
| A6 | `EMAIL_CAPTURE` in production, or together with `EMAIL_HOST` | Server refuses to start | Refused | Pass | config.rs `email_settings_that_would_mislead_are_refused_at_startup` |
| A7 | Mail server unreachable (refused connection) | 502, "couldn't be reached or didn't accept", nothing marked sent, banner with Download PDF and Try again | As expected; no `pdf_emailed_at`; sink count unchanged | Pass | demo checks 5–7; demo-run/screenshots/03; chef-leg3 matrix (a) |
| A8 | Permanent rejection (550) | 502 `EMAIL_REJECTED` "check the address", no retry, Draft kept | As expected; one connection; sink unchanged | Pass | cargo: `a_rejected_recipient…`; demo checks 14–16; demo-run/screenshots/05; chef-leg3 (c) |
| A9 | Temporary failure (451) | Retried once, then succeeds or fails truthfully | Retried once, then success | Pass | cargo: `a_transient_failure_is_retried_once`; chef-leg3 (d) |
| A10 | Server never answers | Fails within the timeout, not retried | Unfixed code: **242 s hang**; fixed: under 5 s, one connection | Fixed | logs/03-red-smtp-timeout.txt; cargo: `a_silent_mail_server_times_out…` |
| A11 | SMTP login refused (535) | Reads as a settings problem | 503 `EMAIL_NOT_CONFIGURED` | Pass | cargo: `a_refused_smtp_login…` |
| A12 | Successful receipt by the mail sink | 200 `delivery: "smtp"`; toast says "accepted by the mail server"; one message with the PDF; Reply-To is the chef's business email | As expected | Pass | demo checks 9–12, 17–20; demo-run/screenshots/04, 06 |
| A13 | Retry after recovery | Failure cleared, state advances once | Invoice Draft → Sent; failure cleared | Pass | demo checks 17–19; cargo: `retry_after_the_mail_server_recovers…` |
| A14 | Repeated clicks (5 in one tick) | One send | Leg 3, before the fix: meal plan 5, invoice 2. After: 1 and 1 | Fixed | chef-leg3/notes.md D1; logs/21-run-all-demo.txt |
| A15 | Repeated clicks (5 real mouse clicks, 60 ms apart) | One send | Browser guard alone: 5 sent. After the server-side 10 s window: 1 and 1 | Fixed | logs/20 (correction), logs/21; cargo: `a_repeat_send_seconds_later…` |
| A16 | Paid invoice | Not offered for retry; can't be emailed | Leg 3: stale banner with Try again. Now hidden in the UI; API returns 409 `INVOICE_CLOSED` | Fixed | chef-leg3 D3; logs/20; cargo: `a_paid_or_void_invoice_is_never_emailed` |
| A17 | No-email server banner | Download PDF, no Try again that can't work | As expected; no new `pdf_emailed_at`; sink unchanged | Pass | demo checks 21–23; demo-run/screenshots/07 |
| A18 | Magic link and reset: real vs unknown address | Identical page and response; only the real account gets mail | Identical on both servers; one "login link" message for the real account only | Pass | demo checks 24–28; chef-leg3 (f); cargo: `link_flows_hide…`, `…respond_uniformly` |
| A19 | Link response time with a slow mail server | Real account no slower than unknown | Both under 1 s (email sent after the response) | Pass | cargo: `a_slow_mail_server_doesnt_slow…` |
| A20 | Tokens in logs | None | Unfixed code logged live tokens; fixed code logs none | Fixed | logs/02 (redacted); cargo: `a_captured_link_email_never_logs_the_link` |
| A21 | Booking page promise | No "Confirmation sent" before the outcome is known | "We'll email a confirmation…" / "No confirmation email will be sent…" | Fixed | cargo: `the_booking_response_says…`; customer/screenshots/11 |
| A22 | Demo account | Never sends; records nothing | No mail; no sent or failed timestamps | Pass | cargo: demo.rs `demo_sends_no_email…` |
| A23 | Real SMTP provider (TLS/STARTTLS/login) and a real inbox | — | Not available here; real email forbidden | Blocked | — |

## B. Chef and customer journey

| # | Scenario | Expected | Observed | Result | Evidence |
|---|---|---|---|---|---|
| B1 | Chef signup with mistakes | Clear errors, then recovery | Short password and duplicate email rejected clearly; an empty business name was accepted | Pass (friction) | chef-leg1 notes #1–3; screenshots 01–03 |
| B2 | Onboarding | Done; booking link shown | Done; link `/book/robin-hale` (taken from the email, can't be changed) | Pass (friction) | chef-leg1 screenshots 04–09 |
| B3 | Five recipes with shared ingredients | Saved; still there after reload | Saved; still there after reload | Pass | chef-leg1 screenshots 17–22 |
| B4 | Saved 6-serving quantities | Correct for the recipe's base servings | Always 1.5 × the base amount | Fail (P1-2) | chef-leg1 D1; screenshot 26 |
| B5 | Customer booking with incomplete input and a bad email | Inline errors | "Your name is required", "Enter a valid email", and others | Pass | customer screenshots 01–02 |
| B6 | Double-click on submit | One request | One POST, one reference number, one email | Pass | customer notes (Okafor) |
| B7 | Customer edits through the confirmation link | Saved after reload | 10:00 → 10:30 and a note saved | Pass | customer screenshots 05–07 |
| B8 | Repeat booking request (Castellano) | — | Only possible in free-text notes; done outside Platebook by email | Missing feature | customer notes; screenshot 12 |
| B9 | Chef finds new inquiries | Visible in the app | No Inquiries item in the nav or on the dashboard; reached only through the alert email link | Fail (P1-5) | chef-leg2 F1; observer audit |
| B10 | Convert to client, including a double convert | One client; allergies carried over | One client; allergies and diet carried over; cuisines dropped | Pass (defect P3) | chef-leg2 screenshots 07–11 |
| B11 | Invalid client edits | Rejected | Empty name blocked; adults = -1 saved | Partial | chef-leg2 screenshots 13–14 |
| B12 | Weekly recurring session | Recurs | Stored "Every week"; next session never generated or observed | Partial | chef-leg2 #16 |
| B13 | Schedule change Wed → Thu | Edit the session | Not possible in the UI (the API supports it); done by delete and recreate | Fail (P1-4) | chef-leg2 X2 |
| B14 | 2-serving household | Book 2 servings | The wizard offers only 4 or 6 | Fail (P1-2) | chef-leg2 D4; screenshot 24 |
| B15 | Double booking | Warned | No warning when creating; a banner only on the Calendar | Fail (P2) | chef-leg2 screenshots 35–38 |
| B16 | Allergy conflict (Satay, Okafor) | Warning that blocks until acknowledged | "Allergen Conflict Detected"; Create disabled until acknowledged | Pass | chef-leg2 screenshots 20–22 |
| B17 | Coconut milk tagged Dairy | Not flagged | Flagged Dairy; can't be overridden; curry hidden for the dairy-free household | Fail (P2) | chef-leg1 screenshot 25; chef-leg2 screenshot 27 |
| B18 | Cooking-time feasibility | A meaningful signal | "GREEN" shown for recipes with 0-minute times; the total drops when a dish is added | Fail (P3) | chef-leg2 D6; screenshot 41 |
| B19 | Grocery aggregation (Okafor, 4 servings) | Shared items merged with correct totals | garlic 7 cloves, onion 2, olive oil 3 tbsp; all 9 items correct | Pass | chef-leg2 grocery table; screenshot 49; demo check 3 |
| B20 | Grocery quantities at 6 (or 2) servings | Scaled from base servings | Chili 4½ cans instead of 3; tray bake half of what's needed | Fail (P1-2) | chef-leg2 D2; screenshots 44–45 |
| B21 | Grocery tick-off persists | Ticks still there after reload | Yes, but only after pressing Regenerate once | Partial (P2) | chef-leg2 D1; screenshot 43; demo check 4 |
| B22 | Change after the list is generated | List updates or warns | Session can't be changed; a recipe edit left the list silently stale; Regenerate warned, then cleared ticks | Partial | chef-leg2 screenshots 46–48 |
| B23 | Grocery export (copy and PDF) | Matches the screen | Matches | Pass | chef-leg2 okafor-grocery-copied.txt, okafor-grocery-pdf.txt |
| B24 | Meal-plan PDF contents | Household, dishes, servings, allergens; no invented content | All correct, but includes invented reheating, FREEZE, "use by" and yogurt/cucumber text | Fail (P1-3) | chef-leg3 okafor-meal-plan-pdf.txt; demo-run/meal-plan-okafor.txt |
| B25 | Prep information for cooking day | Useful | Timeline with 0-minute steps only | Missing feature | chef-leg3 X1 |
| B26 | Invoice line items and total | $240.00 + $86.40 = $326.40 | Correct on screen, in the PDF, in the email and in the client view | Pass | chef-leg3 #4–7; demo check 13 |
| B27 | Manual payment | Status Paid and date kept after reload | Works; no field for method or date | Pass (missing field) | chef-leg3 screenshots 36–40 |
| B28 | Client invoice link without login | Amount and how to pay | Shows both; payment happens outside Platebook | Pass | chef-leg3 screenshot 42 |
| B29 | Data export | Contains the week's data; no secrets | Correct; observer found no secrets | Pass | chef-leg3 chef-data-2026-11-24.json |
| B30 | Delete client, then Undo | Client and sessions restored | Restored after reload | Pass | chef-leg3 screenshots 50–53 |
| B31 | Delete client without Undo | Upcoming sessions handled | Upcoming session stays on the calendar; dialog mentions only past sessions | Fail (P2) | chef-leg3 D4; screenshot 54 |
| B32 | Delete a client on desktop | Possible | Swipe gesture only | Fail (P2) | chef-leg3 D5 |
| B33 | Cross-account reads (Pat → Robin's records) | Not found | Session, client, invoice, recipe and inquiry: none of Robin's data; Pat's lists empty | Pass | chef-leg3 isolation table; screenshots 56–64 |
| B34 | `/intake/<anything>` | Not found | Shows account #1's booking form; submitting would send the data to account #1 (observer, from the code) | Fail (P1-1) | chef-leg3 D6; screenshot 63; observer audit |
| B35 | Cross-account writes | Refused | Not tried in the browser; covered by existing tenancy test suites | Untested here | server-rs/tests/tenancy.rs, tests/http/tenancy.test.js |
| B36 | Phone width 375 px (emulated) | No horizontal scroll; usable | No horizontal scroll; toasts cover the bottom action bar | Pass (friction) | customer screenshots 08–11; chef-leg3 screenshots 64–71; demo check 29 |
| B37 | Reload persistence | Work still there | Yes, in every leg | Pass | all notes |
