# Chef tester, leg 3: Robin Hale (Juniper Table Kitchen): meal plan, invoice, email failure, payment, export, isolation, phone

Date: 2026-11-24. App: http://localhost:4310 (with email to the local sink) and http://localhost:4311 (same database, no email). I drove both with Playwright Chromium at 1280x800, deviceScaleFactor 2, plus a 375x812 isMobile/hasTouch emulation.
Screenshots are in `screenshots/`. Files: `okafor-meal-plan.pdf` (+ `okafor-meal-plan-pdf.txt`), `okafor-meal-plan-from-banner.pdf` (byte-identical, downloaded from the failure banner), `okafor-invoice-draft.pdf` (+ `okafor-invoice-draft-pdf.txt`), `chef-data-2026-11-24.json` (Settings export, fictional data only), `costs.csv`.
The client invoice link has a signed token. I never printed it or took a screenshot of it, and the temporary file that held it has been deleted. Below it is called `<client invoice link>`.

## Mail-sink mode changes (environment actions, UTC)

| Time | Mode | Why |
|---|---|---|
| 15:51:30 | `down` | 3a: email the meal plan while the server is unreachable |
| 15:52:20 | `accept` | 3b: retry |
| 15:52:41 | `reject` | 3c: email the invoice and get a 550 |
| 15:53:10 | `accept` | 3c: retry |
| 15:53:27 | `tempfail` | 3d: one send that gets a 451 |
| 15:53:57 | `accept` | 3e: rapid clicking |
| 15:54:47 | `tempfail` | check the button's disabled state during a send (meal plan + invoice) |
| ~15:54:52 | `accept` | |
| 15:55:00 | `tempfail` | same check, run again to capture the output |
| 15:55:09 | `accept` | |
| 16:10:17 | `down` | 8: show the failure toast and banner at phone width |
| 16:10:29 | `accept` | **final state: accept** (checked on the sink page) |

The sink started with 4 messages and ended with 21. The 17 new ones: 11 × "Your meal plan — Monday, December 7" to Ada, 4 × "Your invoice from Juniper Table Kitchen — INV-2026-0001" to Ada (all with Reply-To `hello@juniper-table.example.com` and a PDF attached), 1 password reset and 1 sign-in link to robin.hale@example.com (from goal 3f). Most of the duplicates come from the rapid-click test (D1).

## State left behind (checked after reload)

- INV-2026-0001 (Okafor, Dec 7): **Paid** Nov 24, $326.40. Internal note "Paid by Zelle, Nov 24 2026 …". It still shows a stale "⚠ Email failed" warning (D3).
- **INV-2026-0002** (Castellano, Dec 11): **Draft**, $175 "Chef Service". I created it only to see the unpaid client view. It was never sent. The dashboard counts it as "$175 outstanding" (M4).
- Sessions unchanged from leg 2 (Okafor Dec 7, Lindqvist Dec 10, Castellano Dec 11 and Dec 18). Okafor and Lindqvist now show "⚠ Email failed Nov 24" from my tests. Okafor's failure is from the 16:10 phone test; its last successful send was 15:54.
- The throwaway client "Test Delete Household" and its Dec 15 session are deleted. The client was deleted without undo, and I then deleted the leftover session by hand (see D4).
- A second account, "Other Fork Catering" (pat.other@example.com), exists with no content.

## Chronological log

| # | Goal | Action | What the screen showed | Verified outcome | Evidence |
|---|---|---|---|---|---|
| 1 | 1 | Opened the Okafor session (`/sessions/1`) and clicked "📄 PDF". | File `meal-plan-okafor-household-(ada-okafor)-2026-12-07.pdf`. No toast. | pass | 01, 02, okafor-meal-plan-pdf.txt |
| 2 | 1 | Checked the PDF against reality. | Correct: "Okafor Household (Ada Okafor) — Weekly Meals", Monday, December 7, 2026, Serves 4, both dishes, "Allergies on file: Peanuts, Tree Nuts. No ingredient in these dishes matched them." **Wrong or irrelevant:** under the Garlic Lemon Chicken Rice Bowls it says **"Warm meatballs separately, serve over cold grain"**, and there are no meatballs. Both dishes are tagged **FREEZE** and "Use by Dec 11". The storage box talks about **"Yogurt sauces … Do not reheat cucumber, yogurt, or fresh herbs"**, and neither dish has any of those. The recipe editor has **no reheating, freezer or shelf-life field** (I opened Edit and "Cooking Method" holds only one instructions box), so none of this text came from me. The footer date "Monday, December 7, 2026" is the session date, not the date it was generated (same as leg-2 M5). | **defect D2** / misleading | 04-…pdf-page1, 05, 06, 07 |
| 3 | 1 | Timeline tab, to see what prep information I get for cooking day. | Every step at "0:15", "(0 min active)", "~25 min". Session Chef Notes are empty. There are no prep notes per dish. The grocery list (leg 2) is the only useful cooking-day aid. | known (leg-2 M4/D6), missing (X1) | 03 |
| 4 | 2 | "Create Invoice →" on the session. | **The invoice is created immediately** (`POST /api/invoices/from-session/1`, 201) as Draft INV-2026-0001 with one line "Chef Service 1 × $175" (the session fee), issued Nov 24 and due Dec 8. A live "Client preview" sits beside the form. | pass / friction F1 (a draft appears before you decide to keep it) | 08 |
| 5 | 2 | "+ Add Line Item". | A menu opens: "✏️ Blank line item" plus 12 presets (Meal Prep Session, Grocery Shopping, …). The presets fill in **$0**, so no prices are hardcoded. | pass | 11 |
| 6 | 2 | Line 1 "Weekly meal prep service, Mon Dec 7 (4 servings, 2 dishes)" 1 × 240. Line 2 "Groceries (at cost, receipt available)" 1 × 86.40. Message "Thank you, Ada! Payment by Venmo, Zelle, or check within 14 days." Clicked Save. | Toast "Invoice saved". Subtotal $326.40, Tax 0% $0.00, Total $326.40. Same values after reload. The line-item header has columns Qty/Price/Amount and **no "Description" label** over the text box. | pass | 09, 10, 12 |
| 7 | 2 | "⬇️" (Download as PDF). | `invoice-INV-2026-0001.pdf`: both lines, Subtotal $326.40, Total Due $326.40, my note, "Made with Platebook". The PDF has **no "How to pay"** section, even though the web view has one (see #27). | pass / friction F2 | okafor-invoice-draft-pdf.txt, 13 |
| 8 | 3a | Sink `down`. Clicked "✉️ Email" on the session. | The button turns into "⟳" while it waits. Toast: **"The mail server didn't accept the meal plan, so it was not sent. Try again in a minute. Download the PDF and send it yourself if it keeps failing."** A persistent red banner: **"⚠ The meal plan email wasn't sent (Nov 24 at 3:51 PM). You can download the PDF and send it yourself."** with [Download PDF] [Try again]. API returned 502. | pass (honest, actionable). Wording nit M1: "didn't accept" when the server was actually unreachable. | 14, 15 |
| 9 | 3a | Reloaded. | The banner is still there. The Sessions list shows "⚠ Email failed Nov 24" on the Okafor row. The Dashboard shows nothing about it. | pass / friction F3 | 16 |
| 10 | 3a | Banner "Download PDF". | The same PDF, byte-identical. The banner stays, and there is no "I sent it myself" option to clear it. | pass / friction F4 | okafor-meal-plan-from-banner.pdf |
| 11 | 3b | Sink `accept`. Banner "Try again". | Toast **"Meal plan sent to ada.okafor@example.com (accepted by the mail server)"**. The banner is gone and "Last sent Nov 24 at 3:52 PM" appears near the action bar, and stays after reload. The text does **not** claim the mail was delivered. Sink: exactly **one** new message (#5), to Ada, Reply-To `hello@juniper-table.example.com`, subject "Your meal plan — Monday, December 7", attachment `meal-plan-2026-12-07.pdf` (43,059 bytes, the same size as my download). The body starts **"Hi Okafor Household (Ada Okafor),"** (M2). | pass | 17, 18 |
| 12 | 3c | Sink `reject`. Invoice "📧 Email". | Toast: **"The mail server refused the invoice, so it was not sent. Check the email address; if it looks right, the server's mail settings may need attention. Download the PDF and send it yourself meanwhile."** This message is different from the temporary-failure one, which is good. | pass | 19 |
| 13 | 3c | Reloaded. | Header still **Draft**. Banner "⚠ The invoice email wasn't sent (Nov 24 at 3:52 PM) …" with Download PDF and Try again. The invoice list shows "Draft · ⚠ Email failed". API verification: `status: draft`, `emailed_at: null`, `email_failed_at: 15:52:44`. Sink unchanged (5). | pass | 20, 21 |
| 14 | 3c | Sink `accept`. "Try again". | Toast **"Invoice sent to ada.okafor@example.com (accepted by the mail server)"**. After reload the status is **Sent**. API verification: `status: sent`, `emailed_at: 15:53:13`, `email_failed_at: null`. Sink #6: Reply-To the chef, `invoice-INV-2026-0001.pdf` attached, body "Amount Due: $326.40 / Due: December 8, 2026 / Questions? Reply to this email." The email has **no payment methods and no link to the online invoice** (X2). | pass | 22, 23 |
| 15 | 3d | Sink `tempfail`. Session Email. | Answered after **~2.1 s** (502). The toast is the same as for `down`. After reload the banner says "wasn't sent (3:53 PM)" **and** "Last sent Nov 24 at 3:52 PM" also shows. Both are true, but they look contradictory side by side. No automatic retry or queue. Sink unchanged. | pass / friction F5 | 24, 25 |
| 16 | 3e | Sink `accept`. Clicked the session "✉️ Email" 5 times, 60 ms apart (real mouse clicks at the same spot). | **5 POSTs, 5 identical meal-plan emails to Ada** (#14–#18). The button is disabled while a send is running (seen during tempfail: text "⟳", `disabled=true`), but not quickly enough to stop the extra clicks. In an earlier run with forced clicks I got 5 more (#7–#11). | **defect D1** | 26 |
| 17 | 3e | Invoice "📧 Email", 5 clicks 60 ms apart, then a double-click. | 5 clicks → **2 POSTs / 2 emails** (#12, #13). Double-click → 1. The button shows "Sending…" and `disabled`. Better than the meal plan, but still not fully guarded. | defect D1 (partial) | 27 |
| 18 | 3d/3e | A tempfail resend of an already-Sent invoice. | Status stays **Sent** (API: `emailed_at` 15:54:33 is kept, and `email_failed_at` 15:55:07 is set). The page shows a failure banner on a Sent invoice. That is accurate. | pass | 28 |
| 19 | 3f | :4311, Lindqvist session (`/sessions/6`), Email. | Answered in **64 ms** (503). Toast: **"Email isn't set up on this Platebook server, so the meal plan was not sent. Download the PDF and send it yourself, or ask whoever runs the server to add SMTP settings."** After reload the persistent banner offers **"Try again"**, which can never work on this server (M3). The same banner also appears on :4310, where Try again *would* send. Sink unchanged. | pass / misleading M3 | 29, 30 |
| 20 | 3f | :4311 sign-in page, "Email me a sign-in link instead", for robin.hale@ and nobody@. | Both: **"This Platebook server isn't set up to send email, so no sign-in link was sent. Sign in with your password, or ask whoever runs this server to configure email."** Identical for both, both 200 in ~1050 ms. Nothing in the sink. | pass | 32-4311-* |
| 21 | 3f | :4311 "Forgot password?" for both addresses. | Both: **"This Platebook server isn't set up to send email, so no reset link was sent. Ask whoever runs this server to configure email."** Identical, ~1250 ms each. | pass | 33-4311-* |
| 22 | 3f | Same on :4310 for comparison. | Link: "If an account exists for <address>, we've asked the mail server to send it a sign-in link. It expires in 15 minutes. If nothing arrives, check your spam folder or sign in with your password." Reset: "If an account exists for <address>, we've asked the mail server to send it a reset link. It expires in 30 minutes. …" Identical for both addresses apart from the echoed address, with the same timing. Sink: one reset and one login link, **only for robin.hale@**. The wording does not reveal whether an account exists. Minor: the first click on "Email me a sign-in link instead" only switches the form, so a second click is needed (F6). | pass | 32-4310-*, 33-4310-* |
| 23 | 3g | Inquiries (`/intake`), Lindqvist card. | List banner: "⚠ Booking emails didn't go through for 1 inquiry. Open one to see what failed and try again." Card: "⚠ Confirmation email not sent", "⚠ Alert email to you not sent". Detail: "⚠ The booking confirmation email wasn't sent (Nov 24 at 3:28 PM). [Try again]" and "⚠ Platebook couldn't email you about this inquiry (Nov 24 at 3:28 PM): the mail server was unavailable or isn't set up. Check this page for new inquiries until email works." Fairness: "not sent" is now accurate (leg 2 saw "failed"). But the page still presents it as a problem to retry and does not say the customer was **told on screen that no confirmation would be sent**. Pressing Try again here (the :4310 copy) would now email Sven a confirmation weeks late. The alert-to-chef text is fair and clear. | pass / misleading (leg-2 M1 partly fixed) | 34, 35 |
| 24 | 4 | "✅ Paid" on the Sent invoice. | In-app dialog "Mark as paid? A paid invoice is a record: its lines, amounts and status can no longer be changed (internal notes stay editable)." **There is no field for payment method, reference or date.** I wrote "Paid by Zelle, Nov 24 2026 …" into Internal Notes, saved, then confirmed "Mark as Paid". | pass / missing X3 | 36, 37 |
| 25 | 4 | Reloaded the invoice, the list and the dashboard. | Invoice **Paid**, locked ("This invoice is paid, so its details are locked …"), and the line controls are disabled. List: "INV-2026-0001 · Paid · ✓ Paid Nov 24, 2026 · $326.40", but **still "⚠ Email failed"**. The invoice page still shows the failure banner with **"Try again"**, which would email "Amount Due: $326.40" for a paid invoice (D3). Session page: "Paid · ✓ Paid Nov 24, 2026". Dashboard: "Collected (MTD) $326 · All invoices paid". The dashboard's Okafor row still shows the session fee "$175", not the invoiced amount. API verification: `status: paid`, `paid_at: 2026-11-24 15:59:14`. The paid date is always "now", so a payment received yesterday can't be backdated. | pass / defect D3 | 38, 39, 40 |
| 26 | 5 | Invoice "🔗" (Copy the client's invoice link). | Toast "Invoice link copied" (the URL itself is not shown on screen). In a **new browser with no login** the `<client invoice link>` showed: "Juniper Table Kitchen / Invoice INV-2026-0001 / ✓ Paid on November 24, 2026. Thank you! / Okafor Household (Ada Okafor) / Issued Nov 24, 2026", both lines, Total $326.40, my note, "Powered by Platebook". No login asked. 375 px: no horizontal scroll. The same address without the token showed "🔒 Unable to load. This invoice link is invalid or has expired. Please ask for a new one." | pass | 41, 42, 43, 63-pat-client-invoice-page-without-token |
| 27 | 5 | To see an *unpaid* client view, I created a Castellano invoice from session 3 (Draft, not sent) and opened its link logged-out. | "Invoice INV-2026-0002 / Castellano … / Issued Nov 24 / Due Dec 8 / Chef Service $175.00 / Total $175.00 / **How to pay: Venmo, Zelle, or check**". No pay button and no handles. The customer pays **outside Platebook** (Venmo/Zelle/check) and the chef marks it paid by hand. The page **does not say it is a Draft**, so a chef could share a draft by accident. The invoice is due Dec 8, **before** the Dec 11 service (due date = issue + 14 days). | pass / friction F7 | 45 |
| 28 | 5 | Searched for other client-facing links. | Only the public booking link (`/book/robin-hale`, "Copy client's booking link" on inquiries) and the invoice link exist. There is no client portal, menu approval or meal-plan link. The meal plan reaches the client only as an email attachment or a PDF the chef sends. | (as expected, not a defect) | — |
| 29 | 6 | Settings → Data Management. | "Export All My Data (JSON)" → `chef-data-2026-11-24.json` (20 KB): account, settings, 3 clients, 5 recipes, 27 ingredients, 4 sessions, 6 session dishes, 9 grocery items, 3 inquiries, 2 invoices, 3 lines, 0 payment events. No password, hash or token strings. "Export Costs as CSV" → `costs.csv`: the 4 sessions (date, client, fee, ingredient cost, margin, status). The Costs page also has "Export CSV". **Clients, Sessions and Invoices have no CSV export** (an accountant would want invoices). | pass / missing X4 | 46, files |
| 30 | 6 | Created "Test Delete Household" (test.delete@example.com) and a Confirmed session on Tue Dec 15 (Chili, 4 servings). | Client 4 and session 7 created. | pass | — |
| 31 | 6 | Tried to delete the client **on desktop**. | The client page and its Edit form have **no Delete**. The list has hidden "Edit"/"Delete" buttons behind each card, revealed only by a **touch swipe**. Hover, a mouse drag (it opened the client instead) and Tab (it skips them) don't reveal them. The Guide does not mention deleting clients. **On desktop a client cannot be deleted.** | **defect D5** | 48, 49 |
| 32 | 6 | At 375 px with touch, swiped the card left. | It slides to show [Edit] [Delete]. Tapped Delete: in-app dialog "Delete Client. Remove Test Delete Household from your client list? **Their past sessions and invoices are kept.**" (it is not exposed as `role=dialog`). Tapped Delete. | pass / a11y note | 50, 51 |
| 33 | 6 | Undo. | Toast "Test Delete Household deleted [Undo] ×" (it covers the "+" button). Undo after ~0.9 s → `POST /api/clients/4/restore` 200 and toast "Test Delete Household restored". After reload the client is back and the Dec 15 session is in the Sessions list and opens normally. | pass | 52, 53 |
| 34 | 6 | Deleted it again and let the toast expire. | After reload the client is gone from Clients, there is no "Deleted"/trash view, and `/clients/4` shows "Client not found". **The Dec 15 *upcoming* session stays** in Sessions ("Upcoming (5)") and on the Dashboard as a Confirmed session for a client that no longer exists. The dialog spoke only of "past sessions". I deleted session 7 by hand afterwards. | **defect D4** | 54, 55 |
| 35 | 7 | New browser: signed up "Other Fork Catering" / pat.other@example.com. Onboarding: name "Pat Other", unchecked the starter library and the sample client, "Go to my dashboard". | Signup 201, onboarding completed, slug `other-fork-catering`. | pass | — |
| 36 | 7 | As Pat, opened Robin's URLs (see the isolation table). | None of Robin's private records appeared. API verification with Pat's cookie: 404 for each id, and `[]` for every list. Two presentation problems: the session and recipe pages **show a loading skeleton forever** with no "not found" message, and `/intake/<anything>` shows **Juniper Table Kitchen's public booking form** inside Pat's dashboard (D6). | pass (isolation) / defect D6, friction F8 | 56–64 |
| 37 | 8 | 375x812 checks (see the phone findings below). | No horizontal scroll on any page I checked (`scrollWidth` 375). Toasts sit on top of the bottom action bar. The failure banner squeezes its text into a narrow column. | pass / friction | 64–71 |
| 38 | 9 | Reloaded Sessions, Invoices, Clients, session 1, invoice 1 and the Dashboard. Set the sink to `accept`. | Everything listed under "State left behind" is there. Sink mode "accept". | pass | 72, 73 |

## Email-failure matrix

| Scenario | Wording shown (toast) | Persistent banner / alternatives | State after reload | Sink result | Pass/fail |
|---|---|---|---|---|---|
| a. Meal plan, server **down** (connection refused) | "The mail server didn't accept the meal plan, so it was not sent. Try again in a minute. Download the PDF and send it yourself if it keeps failing." | Banner "⚠ The meal plan email wasn't sent (Nov 24 at 3:51 PM). You can download the PDF and send it yourself." [Download PDF] [Try again]. Sessions list row "⚠ Email failed Nov 24". | Banner kept. Download PDF worked (identical file). | 0 new | **pass** (nit: "didn't accept" for an unreachable server) |
| b. Retry after **accept** | "Meal plan sent to ada.okafor@example.com (accepted by the mail server)" | Banner gone, "Last sent Nov 24 at 3:52 PM" | Kept after reload | Exactly 1, PDF attached, Reply-To hello@juniper-table.example.com | **pass**: does not claim "delivered" |
| c. Invoice, **reject** (550) | "The mail server refused the invoice, so it was not sent. Check the email address; if it looks right, the server's mail settings may need attention. Download the PDF and send it yourself meanwhile." | Banner "⚠ The invoice email wasn't sent (…3:52 PM) …" [Download PDF] [Try again]. List "Draft · ⚠ Email failed" | **Draft**, `emailed_at` null, `email_failed_at` set | 0 new | **pass** |
| c. Retry after accept | "Invoice sent to ada.okafor@example.com (accepted by the mail server)" | Banner gone | **Sent**, `emailed_at` 15:53:13 | 1, `invoice-INV-2026-0001.pdf`, Reply-To the chef | **pass** |
| d. **tempfail** (451) | Same text as (a) | Banner as in (a). The old "Last sent 3:52 PM" also stays visible | Banner kept. No automatic retry | 0 new. Took ~2.1 s | **pass** (F5: both messages show together) |
| e. Rapid clicks, meal plan (5 × 60 ms) | 5 success toasts | none | — | **5 duplicate emails** | **fail (D1)** |
| e. Rapid clicks, invoice (5 × 60 ms / double-click) | 2 success toasts | "Sending…" disabled button | — | **2** / 1 | **partial fail (D1)** |
| f. :4311 no email, meal plan | "Email isn't set up on this Platebook server, so the meal plan was not sent. Download the PDF and send it yourself, or ask whoever runs the server to add SMTP settings." | Banner with **Try again** (useless on this server) | Banner kept (shared DB, so also on :4310) | 0. Answered in 64 ms | **pass** / M3 |
| f. :4311 sign-in link, real vs fake address | "This Platebook server isn't set up to send email, so no sign-in link was sent. …" | "Back to sign in" | — | 0 | **pass** (identical) |
| f. :4311 reset, real vs fake | "This Platebook server isn't set up to send email, so no reset link was sent. …" | — | — | 0 | **pass** (identical) |
| f. :4310 link/reset, real vs fake | "If an account exists for …, we've asked the mail server to send it a … link." | — | — | 1 each, for the real address only | **pass** |
| Paid invoice after an earlier failed resend | — | Banner "wasn't sent … [Try again]" still on a **Paid** invoice, and "⚠ Email failed" in the list | Paid | — | **fail (D3)** |

## Invoice arithmetic check

| Line | Qty | Price | Amount on screen | Amount in PDF | Expected |
|---|---|---|---|---|---|
| Weekly meal prep service, Mon Dec 7 (4 servings, 2 dishes) | 1 | $240.00 | $240.00 | $240.00 | 1 × 240.00 = 240.00 ✓ |
| Groceries (at cost, receipt available) | 1 | $86.40 | $86.40 | $86.40 | 1 × 86.40 = 86.40 ✓ |
| Subtotal | | | $326.40 | $326.40 | 240.00 + 86.40 = 326.40 ✓ |
| Tax 0% | | | $0.00 | (not printed) | 0 ✓ |
| Total | | | $326.40 | Total Due $326.40 | ✓ |

The client web view, the email ("Amount Due: $326.40"), the list and the API (`total: 326.4`) all agree. The price box shows "86.4" (no trailing zero) while you edit. The amounts are formatted correctly everywhere else. The dashboard rounds to whole dollars ("$326").

## Isolation table (as Pat, Other Fork Catering)

| URL type | URL | What Pat saw | API with Pat's cookie | Pass/fail |
|---|---|---|---|---|
| Session (Okafor) | `/sessions/1` | Header "Session" and a loading skeleton that never finishes. Nothing of Robin's. | 404 "Session not found" | pass (F8: no "not found" message) |
| Session (Lindqvist) | `/sessions/6` | Same | 404 | pass |
| Session grocery | — | — | `/api/sessions/1/grocery` 404 | pass |
| Client | `/clients/1` | "Client not found" | 404 | pass |
| Invoice | `/invoices/1` | Silently redirected to Pat's empty Invoices list | 404 | pass |
| Recipe | `/recipes/2` | Header "Recipe" and an endless skeleton | 404 | pass (F8) |
| Inquiry | `/intake?id=2` | Pat's own empty Inquiries list (0 total) | `/api/intake/2` 401, `/api/intake` `[]` | pass |
| Inquiry (path form) | `/intake/2` (and `/intake/1`, `/intake/999`, `/intake/abc`) | **Juniper Table Kitchen's public booking form** ("Robin Hale · Riverside (fictional)"), empty, inside Pat's app frame. It loads `GET /api/settings/public` without a slug, which returns Robin's public settings. Nothing private (no inquiry data was prefilled), but it is the wrong business. | — | pass for private data, **defect D6** |
| Client invoice page, no token | `/invoice/1` | "🔒 Unable to load. This invoice link is invalid or has expired." | — | pass |
| Pat's lists | `/clients`, `/sessions`, `/invoices`, `/recipes`, `/intake`, `/` | All empty ("No clients yet", "No sessions yet", "No invoices yet", "No recipes found", "0 total", dashboard zeros) | `[]` for each | pass |

## Phone findings (375x812, isMobile + hasTouch emulation)

- **No horizontal scrolling** on the dashboard, the session page with the banner, the grocery tab, the invoice editor, the client list or the public client invoice (`scrollWidth` 375 everywhere).
- **The email-failure banner** (session) puts the message in a narrow left column (one or two words per line, nine lines) and the two buttons beside it. It is readable but takes most of the first screen (65, 66). Friction F9.
- **Toasts cover the bottom action bar.** The error toast (top 628 px to bottom 732 px) sits on top of the session's PDF/Email/Repeat bar (668–718), and it stays visible across tab switches until dismissed. On the Grocery tab it covers the first item ("onion — 2"). The "deleted / Undo" toast on Clients covers the "+" button (52, 66, 67). Friction F10.
- **The Grocery list** is fine. The session action bar is hidden on that tab and the last item (coconut milk) scrolls clear of the bottom nav. The rows are large tap targets (343x76) (70).
- **The invoice editor** is fine. Inputs are 42–44 px high, "Remove line" is 32x44, and the sticky Save/Email/Paid/🔗/⬇️ bar sits above the nav with no overlap once scrolled (68, 69, 71).
- Small tap targets: Back arrow 20x20, "Edit costs" 73x16, Chef Notes "Add" 40x16, toast "×" 9x24, dashboard settings icon 22x22.
- **Deleting a client works only by swipe** (touch). That is a good phone pattern but there is no desktop equivalent (D5). The swipe-revealed buttons are named only "Edit"/"Delete", without the client's name.
- Pages scroll inside `main.app-main`, so full-page screenshots capture only the viewport. This does not affect users.

## Where I needed help

- I did not read client or server source. Everything was found from the screen.
- **Deleting a client on desktop:** I could not find a way. I tried the client page, the Edit form, the Guide, hover, a mouse drag and keyboard Tab. I found the hidden "Delete" buttons only through the accessibility tree (they exist but cannot be clicked), then used a touch swipe at phone width. A desktop-only chef would have needed help.
- **Recording a Zelle payment:** there is no field for it, so I put it in Internal Notes. This is a guess at the intended workflow.
- **Diagnosis only:** I used API reads with Robin's and Pat's cookies to confirm `status` / `emailed_at` / `email_failed_at` / `paid_at` and Pat's 404s. I also logged the network calls behind `/intake/999` (which revealed the slug-less `/api/settings/public`).

## Defects (repro steps)

**D1. Rapid clicks send duplicate emails to the client (meal plan: no effective guard; invoice: partial).**
Steps: sink in accept mode. Session page, click "✉️ Email" 5 times about 60 ms apart. Result: 5 POSTs to `/api/sessions/1/email-pdf` and 5 identical "Your meal plan" emails to the client. The button becomes "⟳"/disabled only after the extra clicks are already queued. Invoice "📧 Email" with the same clicks: 2 emails (a double-click gives 1). Expected: one email per intended send (disable synchronously on the first click, or deduplicate on the server). Neither button asks for confirmation before emailing a client.

**D2. The meal-plan PDF prints reheating and storage text that has nothing to do with the dishes.**
Steps: session with "Garlic Lemon Chicken Rice Bowls" and "Chickpea & Spinach Curry" (recipes with no reheating or freezer information, which the recipe editor cannot even hold). Download "📄 PDF". Result: the Bowls say "Warm meatballs separately, serve over cold grain". Both dishes say "FREEZE" and "Use by Dec 11". "Storage & Reheating" warns about "Yogurt sauces", "cucumber, yogurt, or fresh herbs". The client receives this as instructions from the chef. Expected: only information the chef entered, or none at all, plus a way to enter it per recipe.

**D3. A paid invoice keeps an old "email failed" warning and offers "Try again".**
Steps: email an invoice successfully (Sent). Re-send it while the mail server fails (tempfail). Mark it Paid. Result: the invoice page shows "⚠ The invoice email wasn't sent … [Download PDF] [Try again]" above "This invoice is paid", and the list shows "Paid · ⚠ Email failed". Try again would email the client "Amount Due: $326.40" for a paid invoice. Expected: clear the warning (or stop offering resend) once paid, or at least once a send has succeeded.

**D4. Deleting a client leaves their upcoming sessions active.**
Steps: client with a future Confirmed session. Delete the client (swipe → Delete → Delete) and let Undo expire. Result: the client is gone (`/clients/4` "Client not found", no trash or restore view), but the Dec 15 session stays in Sessions ("Upcoming") and on the Dashboard as Confirmed. The dialog says only "Their past sessions and invoices are kept." Expected: the dialog says upcoming sessions are kept too, or offers to cancel them, or the sessions are flagged. (Undo within the toast window restores everything correctly.)

**D5. Clients cannot be deleted on desktop.**
Steps: 1280x800 with a mouse. Clients → any client. There is no Delete on the client page or its Edit form. On the list, the "Edit"/"Delete" buttons are behind the card and appear only with a touch swipe. Hover, mouse drag and keyboard focus don't reveal them, and Tab skips them. Expected: a visible Delete (for example on the client page) usable with mouse and keyboard.

**D6. `/intake/<anything>` shows the first account's public booking form to everyone.**
Steps: as Pat (or logged-out), open `/intake/999`, `/intake/2` or `/intake/abc`. Result: "Juniper Table Kitchen · Robin Hale · Riverside (fictional)" booking form (from `GET /api/settings/public`, which needs no slug). Inside Pat's dashboard it looks like Pat's page. No private data is exposed, but a customer of another chef following a stale or mistyped link would book with the wrong business. (I did not submit the form, to avoid creating data in Robin's account.)

## Usability friction

- **F1.** "Create Invoice →" creates and numbers a Draft immediately (INV-2026-0001 existed before I edited anything). Abandoning it leaves a draft that counts as outstanding (M4).
- **F2.** The invoice PDF and the invoice email omit "How to pay" (Venmo, Zelle, or check from Settings). Only the web view shows it. I had to type the payment methods into the message.
- **F3.** Email failures show on the session/invoice page and in the Sessions/Invoices lists, but not on the Dashboard. A chef who doesn't reopen the session won't see them.
- **F4.** After "Download PDF" from the failure banner there is no "I sent it myself" or "dismiss" option, so the banner stays forever unless the chef resends through Platebook.
- **F5.** After a failed resend the page shows both "⚠ … wasn't sent (3:53 PM)" and "Last sent … 3:52 PM". It is correct, but it takes a moment to work out that the earlier send went and the later one didn't.
- **F6.** "Email me a sign-in link instead" first only switches the form, and a second button actually sends.
- **F7.** The client invoice link works for Drafts and does not mark them as drafts. The default due date (issue + 14 days) can fall before the service date (Castellano: due Dec 8 for a Dec 11 session).
- **F8.** Opening a session or recipe you can't access (deleted, or another account's) shows a loading skeleton forever. Clients show "Client not found" and invoices redirect silently. These should all say "not found" consistently.
- **F9.** At phone width the failure banner is a tall, narrow text column.
- **F10.** At phone width toasts (especially sticky error toasts) cover the bottom action bar, the first grocery item and the "+" button.
- **F11.** Delete-client and Mark-as-paid dialogs are not exposed as `role=dialog`. The swipe buttons are named only "Edit"/"Delete", and the "⟳" sending state has no accessible name.
- **F12.** The Dashboard shows a 5-step guided tour ("Step 1 of 5 … Skip tour") the first time it is opened after onboarding. It was harmless but appeared unexpectedly mid-task.
- **F13.** The meal-plan PDF title says "Weekly Meals" and its footer date is the session date (leg-2 M5 also applies to the meal plan).

## Missing features

- **X1.** No cooking-day prep information beyond the 0-minute timeline: no per-dish prep notes, no make-ahead or order-of-work, and no packing or labeling list.
- **X2.** Invoice email has no link to the online invoice page and no payment instructions. Customers are paid outside Platebook, so the email is the only place to say how.
- **X3.** "Mark as Paid" has no payment method, reference or date. The paid date is always "now", so it can't record when Zelle actually arrived. Payment events stay at 0 in the export.
- **X4.** No CSV export for invoices, clients or sessions (only Costs CSV and the full JSON).
- **X5.** No trash or restore for deleted clients after the Undo toast is gone.
- **X6.** No automatic retry or queue for temporary mail failures (451). The chef must remember to press Try again.

## Misleading text

- **M1.** For a mail server that is down (connection refused), the toast says "The mail server didn't accept the meal plan". It never reached the server. The banner wording ("wasn't sent") is accurate.
- **M2.** The client emails start "Hi Okafor Household (Ada Okafor)," (the client record name with the parentheses) instead of "Hi Ada,".
- **M3.** On the no-email server (:4311), the persistent "wasn't sent" banner still offers "Try again", which can never work there. Because the database is shared, the same failure banner on :4310 would send for real.
- **M4.** The Dashboard counts a never-sent Draft as outstanding ("$326 outstanding" before sending; "$175 outstanding" now for draft INV-2026-0002).
- **M5.** Lindqvist inquiry: "The booking confirmation email wasn't sent … [Try again]" frames it as a failure to retry, although on that server the customer was told on purpose that no confirmation would come. Better than leg 2's "failed", but it still invites sending a confirmation weeks late.
- **M6.** Delete-client dialog: "Their past sessions and invoices are kept." Upcoming sessions are kept too and stay on the calendar (D4).
- Honest wording worth keeping: "(accepted by the mail server)" on success, "refused … check the email address" for 550, "isn't set up on this Platebook server" for the no-email copy, and identical "If an account exists for …" messages for real and fake addresses.
