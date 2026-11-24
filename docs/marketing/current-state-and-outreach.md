# Platebook — Current State and First Outreach Package

Evidence date: **2026-11-24**. Prepared for Hermes and the owner.

**Nothing in this file has been sent, posted or published, and no outreach results exist yet.** Every external action needs the owner's approval of the exact recipient or channel and the final text.

Legend:
- **[code]**: implemented in the repository.
- **[tested]**: covered by automated tests that ran in this session (§2).
- **[deployed]**: seen in the Railway deployment's status or logs.
- **[run]**: exercised on a local server with synthetic data in this session (§2b).
- **[planned]**: roadmap only.

---

## 1. Verified baseline

| Item | Finding | Evidence |
|---|---|---|
| Repository | `taliesin-fios/platebook`. Work branch `claude/charming-edison-btwwot`, started from `main` at `3cbe45e` ("Roadmap: v0.3.0 is released; v0.4.0 is next (#70)"). The working tree was clean before this work. No other remote branches, no open PRs, no open issues. | `git status`, GitHub API |
| Latest release | **v0.3.0**, published 2026-11-04 (tag on `22f9406`). Earlier: v0.2.0 and v0.1.0. The earlier Hermes observation is confirmed. | GitHub releases |
| Unreleased on `main` | `CHANGELOG.md` `[Unreleased]`: invite-only sign-up (migration `0012_signup_invites.sql`, additive), public demo mode, first-run tour and `/guide`, allergen checks from ingredients, Undo for deleted clients, many usability, accessibility and wording fixes. | `CHANGELOG.md` |
| Roadmap | **Now:** release v0.4.0 (size S, status "Next", not started). **Later:** read-only client portal, menu feedback loop, PWA. No work is in progress in any branch or PR. | `ROADMAP.md` |
| Deployment | Railway project `platebook` (production): app service plus Railway Postgres (5 GB volume, us-west2). Running deployment `44fbafb3` from `b011bb0` (2026-11-22). The later commit `3cbe45e` changed only the roadmap and was skipped by the watch patterns, so the app code running matches `main`. An empty staged change patch has sat pending since 2026-11-06. | Railway status, deployments |
| Live configuration | From the startup log, 2026-11-22: `production: true`, `client_url` = the Railway URL, `signup_mode: Invite`, `demo: enabled, reset_hours: 24`, `email_configured: false`, `payments_mode: Manual`, `stripe_configured: false`, `ai_server_key: false`, `auto_backup: false`. Public URL `https://platebook-production-8c67.up.railway.app` (no custom domain). | Railway deploy log |
| Demo reset | Reset on start (2026-11-22 20:45 UTC) and on 2026-11-23 20:45 UTC; next due 2026-11-24 20:45 UTC. | Railway deploy log |
| Traffic | Only link-preview fetches (`/login`, icons) on 11-19 and 11-23. **No evidence of users or adoption**, and none is claimed. | Railway HTTP log |
| License | **PolyForm Shield 1.0.0**: source-available, **not open source** (not OSI-approved). Free to use and self-host for any purpose except a product that competes with Platebook. README, landing page and NOTICE already say this correctly. | `LICENSE`, `NOTICE`, `README.md` |
| Public terms | Use "source-available", "self-hostable", "free to self-host", "created by Taliesin Fios". Never say "open source". | — |
| AI-assisted development | Most recent commits carry `Co-Authored-By: Claude …`. Honest wording: "built by one developer with a lot of help from AI coding tools." | `git log` |

**Access limit.** This container's network blocks the Railway app domain. I could **not** open the live site or the live demo, and did not try to sign in. Everything about the live server comes from Railway status and logs. Runtime behavior was checked on a local server started with the same settings (§2b).

## 2. Tests and CI

| Check | Result | Who ran it |
|---|---|---|
| CI on `main` @ `3cbe45e` (client lint, tests and build on Node 20; fmt; clippy; cargo test; HTTP suites) | **success** (run 282, 2026-11-23) | Existing GitHub Actions report |
| `npm run lint`: license headers, client ESLint, `cargo fmt --check`, `cargo clippy -D warnings` | **passed**. ESLint 0 errors and 20 warnings (the documented baseline) | This session |
| `npm --prefix client run build` (tsc + vite) | **passed**, no warnings | This session |
| `npm --prefix client test` (vitest) | **passed**: 22 files, 166 tests | This session |
| `cargo test` (throwaway Postgres 16 per test) | **passed**: 250 tests, 0 failed | This session |
| `npm run test:http` (smoke, auth, tenancy against a fresh server) | **passed**: 85/85 | This session |
| `npm run test:migrate` (SQLite → Postgres) | **passed**: 6/6 | This session |

The first `cargo test` attempt failed 22 tests with `PoolTimedOut`. The cause was my throwaway Postgres crashing: its data directory lost permissions mid-run. After restarting it elsewhere, the full re-run passed. No test files or generated types changed.

### 2b. Local runtime check (synthetic data)

Debug build of the current code. Settings mirror production: `NODE_ENV=production`, `DEMO_MODE=true`, `SIGNUP_MODE=invite`, no SMTP, `PAYMENTS_MODE=manual`, no AI key. Throwaway Postgres; browser at 375 px phone width.

| Check | Result |
|---|---|
| `GET /api/auth/signup` | `{"open":false,"invite_only":true,"demo":true}` |
| **Try the demo** at 375 px | Opens the dashboard with the demo banner ("resets in about 24 hours, and no emails are sent") and the first-run tour. No page errors. |
| Demo seed | 5 clients: Marcus Johnson (Shellfish), Priya & Sam Patel (Eggs), Ruth & David Goldberg (Sesame), The Chen Family (Tree Nuts), The Okafor Household (Gluten, Peanuts). 28 sessions, 20 recipes, 20 invoices (17 paid, 2 sent, 1 draft), 3 inquiries. |
| Session page and Grocery tab at 375 px | Renders: merged items with the dishes they came from, Regenerate / Uncheck All / Copy-Share / PDF. |
| Meal-plan PDF / invoice PDF | 200, `application/pdf` (44 KB / 39 KB) |
| Session for Marcus with "Shrimp and Vegetable Fried Rice" | **409 ALLERGEN_CONFLICT**: "contains the client's allergens (Shellfish). Acknowledge the conflict to save anyway." |
| Email from the demo / saving an AI key on the demo | **403 DEMO_DISABLED** for both, with a clear message |
| Sign-up without an invite | Refused (403 `INVITE_INVALID`) |
| `chef-server invite create`, then sign up at 375 px | Link printed. Sign-up lands on onboarding step 1 of 5 ("Let's set up your business"). |
| **Email an invoice on a normal account with no SMTP** | **200 `{"success":true,"sent_to":…}`; invoice status changed draft → sent; `emailed_at` set. Nothing was delivered.** This confirms defect 1. |
| Forgot-password with no SMTP | 200 `{"sent":true}`, nothing delivered. Confirms defect 2. |

Screenshots were kept in the session scratchpad only, not committed.

## 3. Known defects and limitations

**Blockers for a pilot with real data on the owner's server**

1. **Email is silently dropped, and the app says it was sent.**
   - **Cause.** If any of `EMAIL_HOST`, `EMAIL_USER` or `EMAIL_PASS` is missing, `email::send` puts the message in an in-memory outbox and returns success. Nothing is logged in production (`server-rs/src/email.rs` `send`; `config.rs` `email_configured`).
   - **What the user sees.** The UI confirms "Meal plan sent to …", "Invoice sent to …" and "Confirmation sent to …". The sign-in and password-reset pages say a link is on its way.
   - **Side effects.**
     - An invoice "emailed" this way is marked **Sent** and later shows as **Overdue**, for a client who never received it.
     - A client who books through the booking page is told "Confirmation sent to …".
     - The chef's new-inquiry alert is dropped too.
   - **Warning.** Nothing in the app tells the chef that email isn't set up.
   - **Demo.** Not affected: the demo account refuses email openly.
2. **A chef who forgets their password is locked out.** Without email, reset and magic links never arrive. There is no in-app password change and no admin command, so recovery means editing the database by hand.
3. **There is no account deletion.** No route, UI or command exists. Tenant tables don't cascade from `accounts`, so deletion means clearing each table in order, as the demo reset does. Deletes inside the app are soft: clients are deactivated, invoices and inquiries are flagged as deleted. That data stays in the database and in the export.
4. **Backups are unverified.** `auto_backup: false` is the intended default on Railway (`server-rs/src/backup.rs`), because Railway's own managed backups are assumed. I couldn't see whether they are on for this plan.
5. **There is no privacy or data note for hosted accounts.** The pilot invitation's data paragraph (§11) is the draft note.

**Limitations to state honestly**

6. **Invites are made by hand.** Run `railway ssh`, then `chef-server invite create`. Links use `client_url`, which is correctly set to the Railway URL. With `--email`, the chef must sign up with that exact address. Platebook doesn't email invites.
7. **AI can't be shown on the hosted server.** There is no server key, and the demo refuses saving an AI key. AI import and suggestions need each chef's own paid key.
8. **Online payments are off on the hosted server** (`Manual`). They are beta and single-chef by design.
9. **Allergen detection is keyword-based and advisory only.**
10. **v0.4.0 isn't tagged.** The hosted server already runs that code, but a self-hoster on the `v0.3.0` image lacks the demo, the tour and the allergen fixes. I didn't run the Docker Compose self-host path in this session.
11. **The landing page (`landing-page/index.html`) isn't deployed anywhere I could verify.** The Railway server shows the app's own Welcome page.

## 4. Claim table

| Capability | Support | Safe public wording | Limitation |
|---|---|---|---|
| Client profiles: allergies, dietary styles, likes, kitchen notes | [code] [tested] [run] | "Keep each household's allergies, preferences and kitchen notes in one place." | — |
| Booking/intake page per chef → Inquiries → convert to client | [code] [tested] | "Your own booking link where new clients fill in an intake form." | No email alert or confirmation without SMTP (defect 1). |
| Recipe library, scaling, 20-recipe starter set | [code] [tested] | "A recipe library that scales to each household's serving size." | — |
| AI recipe import and menu suggestions | [code] [tested with fakes] | "Optional: bring your own AI key to import recipes from a link or text." | Not on the hosted server or demo; chef pays the provider. Don't lead with it. |
| Session planning: calendar, conflicts, repeats, suggested date, cooking-time check | [code] [tested] | "Plan sessions on a calendar, with repeat bookings and conflict warnings." | — |
| Allergen check against the client's allergies | [code] [tested] [run] | "Flags likely allergens from ingredient names. Always confirm with your client." | Keyword-based; advisory. Never "allergen-safe". |
| Grocery list merged across dishes, by store section, tick-off | [code] [tested] [run] | "One grocery list per session, merged across dishes and grouped by store section." | — |
| Meal-plan and invoice PDFs | [code] [tested] [run] | "Branded meal-plan and invoice PDFs." | Emailing them needs SMTP. |
| Invoices and payment records | [code] [tested] [run] | "Invoice from a session and see what's outstanding." | Payments recorded by hand on the hosted server. |
| Online card payments (Stripe) | [code] [tested with fakes] | Don't market yet. | Beta, one chef per server, off on the hosted server. |
| Costs, margins, monthly P&L with CSV | [code] [tested] | "See margin per session and a monthly profit-and-loss you can export to CSV." | Cash basis, no mileage, not tax advice. |
| Data export | [code] | "Export all your data as a JSON file at any time." | Raw archive, not spreadsheet-ready. |
| Account isolation (Postgres row-level security) | [code] [tested] | "Each account's data is kept separate in the database." | No audit or certification. |
| Self-hosting with Docker Compose | [code] docs | "Free to self-host; source on GitHub under PolyForm Shield." | Not run in this session. |
| Public demo, reset daily | [deployed] [run] | "Try the demo: a shared sample kitchen that's wiped every day." | It's shared: visitors see each other's edits until the reset. Owner to open it once on a phone before sharing. |
| Works in a phone browser | [code] [run] | "Works in your phone's browser." | Not installable or offline (PWA is [planned]). |
| Client portal, post-session ratings, offline mode | [planned] | Don't mention as features. | — |

**Never claim:** users, customers, testimonials, time saved, "production-ready", "secure", "certified", "HIPAA", "open source", or a price.

**Core chef journey.**

| Step | State |
|---|---|
| Client preferences | Works. |
| Recipes / menu | Works. AI only with the chef's own key. |
| Session planning | Works, including the allergen check and the cooking-time check. |
| Groceries / prep | Works: merged list, store sections, tick-off, share/print/copy, timeline tab. |
| Client handoff | The PDF works. **Email delivery is broken on the hosted server and reports success** (defect 1). The workaround is to download and send it yourself. |
| Invoicing / payment records | Works with manual payment records. Emailing an invoice without SMTP wrongly marks it Sent. |

## 5. Readiness verdict

| Question | Verdict | Why |
|---|---|---|
| Public demonstration? | **Yes, one-to-one and in small numbers.** Not broad promotion. | The demo is live per the logs: synthetic data, reset daily, email and AI refused. Its flows worked on the local equivalent. **Condition:** the owner opens the live demo once on a phone before sharing, because I couldn't reach it. Not broad yet: the URL is a raw Railway subdomain and the landing page isn't deployed. |
| Small assisted pilot with real client data? | **Not yet.** | Defects 1–5 above. All are owner-side setup or small operational work (SMTP, a password-reset and deletion procedure, a backup check, a data note), not new features. |
| Private feedback only? | **Yes, now.** | One-to-one walkthroughs on the shared demo. No accounts, no real data. |

**Plan while the owner is on vacation: Phase 1 only.** 2–3 warm contacts (Draft A), short demo calls or self-serve demo plus questions. Defer Draft B, Draft C and the pilot until the owner is back and the gates in §11 are met.

## 6. Primary audience

**Solo in-home personal chefs who cook regularly (weekly or biweekly) for a few recurring households.**

- The one complete, tested path is their week: household → menu → session → grocery list → meal-plan PDF → invoice.
- The product's limits match a solo chef: invite-only hosting, single-chef payments, no team features.
- They're reachable through people the owner already knows.
- Allergies are a demo talking point, not a requirement for who to contact.
- In the tracker, mark weekly meal-prep chefs (strongest fit) separately from occasional dinner-party chefs (weaker fit), so the feedback is read correctly.

**Secondary, not a buyer:** self-hosters and developers. Useful for code feedback; doesn't validate chef demand.

## 7. Positioning copy

**Description (≈45 words).** Platebook is a web app for personal chefs who cook in clients' homes. It keeps each household's allergies and preferences, your recipes, your session calendar, the grocery list for each cook day, meal-plan PDFs and invoices in one place. It works in a phone browser and is free to self-host.

**Value proposition.** The client notes, menus, grocery lists and invoices for each cook day, in one place instead of spread across notes, spreadsheets and texts.

**Founder introduction.** The owner writes the bracketed sentence; don't accept a generated one.

> I'm Taliesin Fios. I built Platebook, a small web app for personal chefs, as a solo project with a lot of help from AI coding tools. [Owner: one true sentence on why.] It's early, and I'm looking for a few chefs to tell me where it fits their week and where it doesn't.

## 8. Demo script (≈10 minutes, demo account only)

Use **Try the demo** on the welcome or sign-in page. Show only what's supported: no email, no AI, no online payment.

1. **Dashboard (30 s).** Skip the first-run tour, or use it as the intro. Point out the banner: "a shared sample kitchen, wiped daily."
2. **Clients → The Chen Family (1 min).** Household size, allergies (tree nuts), kitchen notes. "This is what you check before you shop."
3. **Recipes (1 min).** Open a recipe and change the servings. Mention the 20-recipe starter set.
4. **Sessions → + Session for Marcus Johnson (shellfish allergy) (2 min).** The date is suggested from his preferred day. On the Menu step, turn **off** "Hide allergen conflicts". "Shrimp and Vegetable Fried Rice" is now flagged for shellfish, and choosing it asks you to acknowledge the conflict. Show the cooking-time check. Say: "It flags likely allergens from ingredient names. You still confirm with the client." (Alternative: Ruth & David Goldberg (sesame) with "Sesame Ginger Chicken Stir-Fry".)
5. **That session → Grocery tab (1.5 min).** A merged list by store section. Tick items off as you would in the store.
6. **Meal-plan PDF (1 min).** Download it: "This is what the client gets." Don't press Email; it's blocked on the demo.
7. **Invoice (1.5 min).** Create one from the new session. Then show the list: paid, sent (one overdue) and draft. Payments are recorded by hand.
8. **Inquiries (1 min).** Open a sample inquiry and **Convert to Client Record**. "Clients fill this in from your booking link."
9. **Costs → Profit & Loss (30 s).** The monthly view and CSV export.
10. **Questions, not features.** "Where does this not match how you work?" "What would you need before trusting it with a real client?"

Don't show AI, Stripe, a client portal or offline use. Make no claim about time saved.

## 9. Channels

Rules were checked 2026-11-24 where reachable. **Re-check on the day of posting.**

| Channel | Type | Eligible | Rule evidence | Timing |
|---|---|---|---|---|
| Chefs the owner knows (DM/email) | Qualified buyers | **Yes**, one at a time, owner-approved | — | **Now** (2–3 people) |
| Professional connectors the owner knows (culinary instructor, chef-staffing contact, association chapter lead) | Connectors, not buyers | **Yes**, consent-based referral only | — | After vacation |
| GitHub Discussions, own repo | Owned | **Yes** | Owner's repo; already linked as "Give feedback" | After vacation. Only worth it if someone answers. |
| r/selfhosted weekly **New Project Megathread** | Self-hosters, **not chef buyers** | **Conditional** | April 2026 mod update, read through mirrors: new projects go in a weekly megathread (replaced each Friday); standalone project posts are removed; the template asks for **AI involvement**; an auto-mod bot holds the comment until the author replies. **Not verified:** the current sidebar, the exact project-age threshold, and whether non-OSI licenses face a rule or only pushback (reddit.com is blocked here). | Only after v0.4.0 is tagged, the Compose path has run on a clean machine, and the owner can reply for 1–2 days |
| r/Chefit | Mostly restaurant cooks | **No promotion** | A cached 2023 page shows strict anti-spam moderation; current rules not readable | Participate only |
| USPCA / APPCA | Qualified buyers | **Not now** | USPCA offers paid conference sponsorship ($5k–$15k tiers) and membership; no free vendor channel found | Revisit when hosted pilots are ready; purchases aren't authorized |
| Facebook groups, ads, directories | — | **No** | Not reviewed; not authorized | — |

## 10. Drafts (unsent)

### Draft A — warm note to a chef the owner knows (use now)

> Subject: Could I get 20 minutes of your honest opinion?
>
> Hi [Name],
>
> I've been building a small web app for personal chefs called Platebook, and you'd know quickly whether it's useful or not.
>
> It keeps each household's allergies and preferences, your recipes, your cook-day grocery lists, meal-plan PDFs and invoices in one place. It's early, it's just me (with a lot of help from AI coding tools), and there's nothing to buy. I'm trying to find out whether it's worth continuing.
>
> Would you give me 20 minutes on a call where I show you the demo and you tell me where it doesn't match how you actually work? Or, if you'd rather poke around on your own, there's a demo kitchen with sample data: [demo link]. It's shared with anyone else trying it and wiped every day, so please don't type anything real.
>
> No pressure at all if the timing's bad.
>
> [Your name]

### Draft B — permission-based message to a professional contact (after vacation)

> Subject: Could you point me to one or two personal chefs?
>
> Hi [Name],
>
> [One specific, true line, or delete this line.] I've built an early web app for personal chefs who cook in clients' homes: client allergies, menus, grocery lists, meal-plan PDFs and invoices.
>
> Do you know one or two personal chefs who might give me 20 minutes of blunt feedback on a demo? If so, could you ask them first whether they're OK with me getting in touch? Please don't pass it to a list, and you don't need to vouch for it. If you mention me, just say I'm someone building this.
>
> If nobody comes to mind, no problem at all.
>
> Thanks,
> [Your name]

### Draft C — r/selfhosted weekly New Project Megathread, top-level comment (after the gates in §9)

> **Platebook**: a self-hosted web app for personal chefs who cook in clients' homes
>
> - **What it does:** client households with allergies and preferences, recipes that scale to household size, a session calendar, a merged grocery list per cook day, meal-plan and invoice PDFs rendered on the server, and invoices with costs and margins. One server can hold one chef or several accounts, separated with Postgres row-level security.
> - **Stack:** Rust (Axum, SQLx) on Postgres, a React + TypeScript front end, and Docker Compose for self-hosting.
> - **License:** source-available under PolyForm Shield 1.0.0. Free to self-host for any purpose except offering a competing product. It is **not** an OSI open-source license, and I want to be upfront about that.
> - **AI involvement:** I'm a solo developer and built it with heavy use of AI coding tools (Claude Code). I review the changes and rely on CI, which runs the Rust and front-end tests, including cross-account isolation tests.
> - **Status:** early. Use the latest release (v0.4.0) [owner: confirm it's tagged]. Email needs your own SMTP. Online payments are beta and suit one chef per server. Allergen flags are keyword-based and advisory.
> - **Repo and self-hosting guide:** https://github.com/taliesin-fios/platebook
>
> What would stop you from trusting it with real data, if you self-host something similar for a small business?

**For GitHub Discussions:** use the same text, with the AI-involvement line kept as a sentence. Add: "Feedback from working chefs especially welcome. Replies may take a few days."

## 11. Pilot invitation (Phase 2): drafted, **do not send until all gates are met**

**Gates**

- **G1 — Email works on the hosted server.** SMTP is configured and a test message has been received. This is a hard gate, because without email a forgotten password locks a chef out (defect 2).
- **G2 — Backups confirmed** on Railway, with one restore tried.
- **G3 — Data procedures.** The owner approves the data paragraph below, and has written and tested on a throwaway database:
  - a delete-account script;
  - a manual password-reset step.
- **G4 — Owner availability.** The owner is back and can reply within the stated time. Cap the pilot at **2–3 chefs**.

**Expected owner time:** about 2–3 hours per chef over four weeks. That covers a 30-min setup call, four 10-min check-ins, a 30-min wrap-up, making the invite, and support.

> Subject: Trying Platebook for real for four weeks?
>
> Hi [Name],
>
> Thanks for looking at Platebook. I'm asking a couple of chefs to use it for their real work for four weeks. Here's exactly what that involves.
>
> **What you get:** your own Platebook account on my server, set up with you on a 30-minute call. You can message me directly about problems. I'll try to fix anything that stops you working, but I can't promise a fix or a timeline. Cost: [OWNER DECISION]. I'm not promising a price or hosting beyond the pilot.
>
> **What I'm asking:** plan your real sessions in it for four weeks (as many as feels natural), a 10-minute check-in each week (a call or a few lines by message), and a 30-minute wrap-up call. If you stop using it partway through, that's useful feedback too. Just tell me.
>
> **Support limits:** it's just me. I'll reply within [OWNER DECISION], not instantly.
>
> **Your data:** your account is separate from other chefs' accounts. It's stored in a database I run on Railway (a hosting company, US region). As the operator I can see the data. I'll only look at it to fix a problem you've reported, and I'll tell you when I do. You can export everything at any time. When you ask, or when the pilot ends, I'll delete your account and its data from the live database myself (there's no self-serve button yet). Backup copies are gone within [OWNER DECISION] days. Things you delete inside the app are hidden, not erased, until then. You'll be storing your clients' details, including allergies, so only enter what you and your clients would be comfortable keeping on a small, early service. Allergen flags are a reminder, not a safety check: always confirm with your client.
>
> **Feedback:** the weekly check-ins and the wrap-up call. With your OK I'll take notes, and I won't quote you publicly without asking.
>
> If you're in, reply and I'll send a one-time sign-up link.
>
> [Your name]

## 12. Tracking sheet

**File:** `docs/marketing/outreach-tracker.csv`.

- **Public repo: header row only.** Copy the header into a **private** sheet and record real contacts there. Never commit rows with people in them.
- **One table, two kinds of rows.** `sheet` is `outreach` or `pilot`. Pilot rows add `pilot_week`, `sessions_planned`, `would_keep_using` and `support_min`.
- **Dates:** `YYYY-MM-DD`.
- **Fixed values:**
  - `response`: yes / no / no-reply.
  - `demo_outcome`: fit / partial / no-fit.
  - `would_keep_using`: yes / maybe / no.
  - `segment`: meal-prep / dinner-party / other / self-hoster.
- **Time:** `owner_min` and `support_min` in minutes.

## 13. Changes in this package

- **Added:** this file and `docs/marketing/outreach-tracker.csv` (header only).
- **`landing-page/index.html`, unsupported claims narrowed:**
  - "emailed … in one click" now says email needs SMTP.
  - "tested recipes" is now "20-recipe starter library".
  - "share ingredients to cut … waste" (no such feature) is replaced with the merged grocery list.
  - "land in your inbox" is now "land in Platebook".
  - "Allergens flagged on every dish … nothing slips past you" is now "Likely allergens flagged from ingredient names (advisory)".
  - The mock no longer pairs a coconut dish with a "checked" tree-nut line.
- **`landing-page/index.html`, other changes:**
  - The hero and bottom calls to action now say **Try the demo** (to `/login`) instead of sign-up, since the hosted server is invite-only. The nav "Get started" still goes to `/signup`.
  - The setup step notes that some servers are invite-only.
- **Not changed:** no server, client, migration, deployment or settings change. Nothing published.
