# Screenshot walkthrough

All data is fictional. Screenshots are unedited captures from a real Chromium browser against an
isolated local instance. Defects are shown, not hidden. Phone-width shots are 375 px browser
emulation, not a physical phone.

**Screen recording:** [`evidence/demo-run/walkthrough.webm`](evidence/demo-run/walkthrough.webm),
about 25 seconds at 1280×800, unnarrated and fast (no pauses were added). Playwright captured it
during the scripted demo run at commit `dec3af1`. It covers screens 1–7 below: sign-in, dashboard,
grocery tick-off, meal-plan failure, download and retry, invoice rejection and retry, and the
no-email server. Screens 8–10 (sign-in-link pages, phone width) ran in separate browser contexts
and are not in the video.

## 1. The email fix, end to end (scripted demo, `evidence/demo-run/screenshots/`)

| | Screenshot | What it shows |
|---|---|---|
| 1 | ![](evidence/demo-run/screenshots/01-dashboard-week.png) | The seeded fictional week on the dashboard: four upcoming sessions and the Okafor allergens (Peanuts, Tree Nuts) flagged. The booking-link card is below the fold. |
| 2 | ![](evidence/demo-run/screenshots/02-grocery-aggregated.png) | Okafor grocery list: garlic combined from two dishes (4 + 3 = **7 cloves**) and ticked off. The tick was confirmed saved after a reload. |
| 3 | ![](evidence/demo-run/screenshots/03-mealplan-email-failed.png) | **Mail server down.** The toast says the mail server "couldn't be reached or didn't accept the meal plan, so it was not sent". The banner offers **Download PDF** and **Try again**. Nothing was marked sent and nothing reached the sink (checked through the API and the sink). |
| 4 | ![](evidence/demo-run/screenshots/04-mealplan-retry-ok.png) | **Mail server restored; Try again.** The toast says "sent to … **(accepted by the mail server)**", not "delivered". The sink received exactly one message with the PDF, Reply-To set to the chef's business email. |
| 5 | ![](evidence/demo-run/screenshots/05-invoice-email-rejected.png) | **Mail server rejects the recipient (550).** The invoice header still says **Draft**. The banner says it wasn't sent, and the toast says "refused… check the email address". No `emailed_at` time; sink unchanged. |
| 6 | ![](evidence/demo-run/screenshots/06-invoice-sent.png) | After retrying with the server accepting, the invoice moves to **Sent** ($326.40 = $240.00 + $86.40). |
| 7 | ![](evidence/demo-run/screenshots/07-no-email-server.png) | **Same account on a server with no SMTP settings.** "Email isn't set up on this Platebook server…". The banner offers the PDF only, with no Try again that could never work. The "Last sent" time shown comes from the earlier send on the other instance (the two share a database); no new time was recorded. |
| 8 | ![](evidence/demo-run/screenshots/08-signin-link-no-email.png) | Sign-in link requested on the no-email server: "no sign-in link was sent; sign in with your password". The page is identical for a real and an unknown address (checked). |
| 9 | ![](evidence/demo-run/screenshots/09-signin-link-with-email.png) | Sign-in link requested on the server with email: "we've asked the mail server to send it…". No claim of delivery, and the page is identical for unknown addresses. |
| 10 | ![](evidence/demo-run/screenshots/10-phone-session.png) | Session page at 375 px wide (emulated): no horizontal scrolling. |

## 2. Customer side (`evidence/customer/screenshots/`)

| Screenshot | What it shows |
|---|---|
| ![](evidence/customer/screenshots/02-okafor-bad-email.png) | Booking form validation after an incomplete submit and a mistyped email. |
| ![](evidence/customer/screenshots/03-okafor-submitted.png) | Okafor submitted (reference #1). A double-click produced one request and one confirmation. |
| ![](evidence/customer/screenshots/11-lindqvist-mobile-submitted.png) | Lindqvist on the **no-email** server at 375 px: "**No confirmation email will be sent, so keep this reference number.**" Nothing arrived, so the page told the truth. Also visible: the page promises "within 1 business day", while the email template elsewhere says "1–2 business days" (DEFECTS P2-7). |
| ![](evidence/customer/screenshots/12-castellano-repeat-in-notes.png) | The repeat-Friday request could only go into free-text notes; there is no date or repeat option. Confirming it happens outside Platebook. |

## 3. Chef side (`evidence/chef-leg2/`, `evidence/chef-leg3/`)

| Screenshot | What it shows |
|---|---|
| ![](evidence/chef-leg2/screenshots/22-review-step-with-satay.png) | **Allergy conflict.** Satay Noodles added for the peanut-allergic Okafor household: "Allergen Conflict Detected", and Create Session stays disabled until acknowledged (the panel continues below the fold). |
| ![](evidence/chef-leg2/screenshots/37-calendar-dec-double-booking.png) | **Double booking**, shown only afterwards on the Calendar. No warning appeared while creating the session (DEFECTS P2-2). |
| ![](evidence/chef-leg2/screenshots/44-castellano-chili-6-servings-grocery.png) | **Defect P1-2.** 6-serving chili for Castellano: "black beans — 4 1/2 cans" where the recipe (written for 6) needs 3. Every line is 1.5× too much. |
| ![](evidence/chef-leg2/screenshots/49-okafor-grocery-final-7-cloves-two-ticks.png) | 4-serving list: shared ingredients merged correctly, two items ticked and still ticked after reload. |
| ![](evidence/chef-leg3/screenshots/04-okafor-meal-plan-pdf-page1-1.png) | **Defect P1-3.** The meal-plan PDF tells the client "Warm meatballs separately…" for chicken rice bowls and marks dishes FREEZE with a "use by" date the chef never entered. |
| ![](evidence/chef-leg3/screenshots/38-invoice-paid-after-reload.png) | Manual payment recorded: Paid, still Paid after reload (there is no field for the method). This was **before** fix E2, so it also shows the stale "invoice email wasn't sent… Try again" banner on a paid invoice. That banner is now hidden, and the server refuses to email a paid invoice. |
| ![](evidence/chef-leg3/screenshots/45-client-view-unpaid-draft-castellano.png) | What the client sees from the invoice link with no login, for an unpaid invoice: the amount and a "How to pay" section. The signed link itself is not shown. Screenshot 42 shows the paid version. |
| ![](evidence/chef-leg3/screenshots/53-375-after-undo.png) | Deleted client restored with Undo (375 px). |
| ![](evidence/chef-leg3/screenshots/59-pat-invoice.png) | **Isolation.** A second chef ("Other Fork Catering") opening Robin's invoice URL gets "Invoice not found" and an empty list. |
| ![](evidence/chef-leg3/screenshots/63-pat-intake-999-shows-juniper-form.png) | **Defect P1-1.** `/intake/999` (an id that doesn't exist), opened by the second chef, shows *Juniper Table Kitchen's* booking form, which belongs to account #1. |
