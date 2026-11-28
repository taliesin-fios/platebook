# Marketing claims, checked against this work

This table covers only what this branch and the rehearsal exercised. "Supported" means it was
observed with evidence in an isolated local test. It does **not** mean customers or real chefs
confirmed it.

The AI role-play is synthetic. It is not real chef validation, it does not show customer demand, and
it does not show that anyone enjoys using the product.

| Claim (as it might be written) | Status | Safe wording | Evidence |
|---|---|---|---|
| "Platebook tells you the truth when an email didn't go out" | **Supported** (local SMTP sink) | "If an email can't be sent, Platebook says so, keeps nothing marked as sent, and offers the PDF and a retry." | EVIDENCE-MATRIX A2–A17 |
| "Emails are delivered" / "delivery confirmed" | **Not supported** | "Sent" means the mail server accepted the message. Platebook doesn't track inbox delivery. | Only acceptance by a local sink was observed (A23 blocked) |
| "Works without email set up" | **Supported, with limits** | "Without email, invoices and meal plans download as PDFs; sending stays off and the app says why." | A2–A5, A17 |
| "Sign-in links and password resets keep your account private" | **Supported** for the response and its timing | "Sign-in and reset requests give the same answer whether or not an account exists." | A18–A19 |
| "Booking requests are never lost if email fails" | **Supported** (one test path) | "A booking is saved even if its confirmation email fails." | A3 |
| "Clients get a confirmation email" | **Partly supported** | Only when the server has email configured. The booking page now says which case applies. | A3, A21 |
| "Grocery lists combine shared ingredients" | **Supported at 4 servings, same units** | "Combines the same ingredient across dishes (e.g. garlic from two recipes)." | B19 |
| "Automatic scaling to any household size" / "accurate grocery lists" | **Not supported** | Do not claim. Wrong at 2, 6 or 8 servings, or for recipes not written for 4. | DEFECTS P1-2 |
| "Allergy warnings when planning menus" | **Supported, narrowly** | "Flags recipes whose ingredients match a client's listed allergens, and asks you to confirm." | B16 |
| "Allergen safety" / "allergen detection you can rely on" | **Not supported** | Keyword matching with a known false positive (coconut milk read as dairy), no override, and no testing for missed allergens. | DEFECTS P2-1 |
| "Professional meal-plan PDFs for clients" | **Not supported as is** | The PDF includes invented reheating, freezing and "use by" text. | DEFECTS P1-3 |
| "Reheating and storage guidance" | **Not supported** | Do not claim. | DEFECTS P1-3 |
| "Invoices with manual payment tracking" | **Supported** | "Create invoices, email or download them, share a no-login client link, and mark them paid." | B26–B28 |
| "Online deposits / pay online" | **Not exercised** | Not tested here: the rehearsal ran with manual payments. A Stripe mode exists. | — |
| "Client portal" / "menu approval" | **Not supported** (these features don't exist) | Do not claim. Clients get a booking form, an edit link by email, and an invoice link. | B8, customer notes |
| "Recurring sessions" | **Not exercised** | The setting saves; generating the next session was never observed. | B12 |
| "Cooking-time feasibility" | **Not supported** | Shows GREEN for recipes with no times entered. | B18, DEFECTS P3 |
| "Reschedule in a tap" | **Not supported** | Saved sessions can't be edited in the app. | DEFECTS P1-4 |
| "Your data is isolated from other chefs" | **Supported for read attempts in this test**; writes are covered by the automated tenancy tests | Don't claim it until P1-1 is fixed: `/intake/<id>` can send a customer's details to account #1. After that: "Each chef's records are private to their account." | B33–B35, DEFECTS P1-1 |
| "Export all your data" | **Supported** (full JSON) | "Export everything as JSON." There is no import or restore, and CSV covers costs only. | B29 |
| "Works on your phone" | **Partly supported** | "Responsive layout." Only a 375 px browser emulation was tested; no physical phone. | B36 |
| "Accessible" | **Not supported** | Unlabelled fields; selection chips don't announce their state. | DEFECTS P2-8 |
| "Chefs love it" / any testimonial | **Not supported** | Synthetic AI role-play only. | — |
| "AI recipe import" | **Not exercised** | No AI key, by design. | — |
| "Self-hostable; email optional" | **Supported** | Startup log and docs explain email modes; partial settings count as "no email". | README, SELF_HOSTING, A5–A6 |
