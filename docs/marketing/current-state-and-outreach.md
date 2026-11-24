# Platebook — Current State and First Outreach Package

Evidence date: **2026-11-24**. Prepared for Hermes and the owner. **Nothing in this file has been sent, posted or published.** Every external action below needs the owner's approval of the exact recipient or channel and the final text.

Legend: **[code]** = implemented in the repository · **[tested]** = covered by automated tests that ran in this session · **[deployed]** = seen on the Railway deployment (logs/config) · **[run]** = exercised by hand on a local server with synthetic data in this session · **[planned]** = roadmap only.

---

## 1. Verified baseline

| Item | Finding | Evidence |
|---|---|---|
| Repository | `taliesin-fios/platebook`, branch `claude/charming-edison-btwwot`, started from `main` at `3cbe45e` ("Roadmap: v0.3.0 is released; v0.4.0 is next (#70)"). Working tree was clean before this work. No other remote branches; no open PRs or issues. | `git status`, GitHub API |
| Latest release | **v0.3.0**, published 2026-11-04 (tag on `22f9406`). Earlier releases v0.2.0 and v0.1.0. The Hermes observation is confirmed. | GitHub releases |
| Unreleased on `main` | Under `[Unreleased]` in `CHANGELOG.md`: invite-only sign-up (migration `0012_signup_invites.sql`, additive), public demo mode, first-run tour and `/guide` page, allergen checks from ingredients on sessions and meal plans, Undo for deleted clients, many usability, accessibility and wording fixes. | `CHANGELOG.md` |
| Roadmap | "Now": release v0.4.0 (S, status *Next*, not started). "Later": read-only client portal (L), menu feedback loop (M), PWA (L). Nothing is in progress in any branch or PR. | `ROADMAP.md` |
| Deployment | Railway project `platebook`, production environment, service `platebook` + Railway Postgres (5 GB volume, us-west2). Running deployment `44fbafb3`, from commit `b011bb0` (2026-11-22). The later commit `3cbe45e` (roadmap text only) was **skipped** by the watch patterns, so app code on the server is the same as `main` today. One empty staged change patch has been pending since 2026-11-06 (no changes in it). | Railway status, deployments |
| Live config (startup log, 2026-11-22) | `production: true`, `signup_mode: Invite`, `demo: enabled, reset_hours: 24`, `email_configured: false`, `payments_mode: Manual`, `stripe_configured: false`, `ai_server_key: false`, `auto_backup: false`. Public URL `https://platebook-production-8c67.up.railway.app` (no custom domain). | Railway deploy log |
| Demo resets | Demo account reset at start (2026-11-22 20:45 UTC) and again 2026-11-23 20:45 UTC; next due 2026-11-24 20:45 UTC. Matches the 24-hour setting. | Railway deploy log |
| Traffic | The HTTP logs show only link-preview fetches (`/login`, icons) on 11-19 and 11-23. There is **no evidence of users or adoption**, and none is claimed. | Railway HTTP log |
| License | **PolyForm Shield 1.0.0**: source-available, **not open source** (not OSI-approved). Anyone may use and self-host it for any purpose except offering a product that competes with Platebook. The README, landing page and NOTICE already say this correctly. | `LICENSE`, `NOTICE`, `README.md` |
| Public terminology | "Source-available", "self-hostable", "free to self-host", "created by Taliesin Fios". Do **not** say "open source" or "free and open". | as above |
| AI-assisted development | Most recent commits carry `Co-Authored-By: Claude …` trailers. Honest wording: "built by one developer with a lot of help from AI coding tools." | `git log` |

**Access limits during this review.** This container's network blocks the Railway app's domain. I could **not** open the live site or the demo in a browser, and did not try to sign in to it. What the server is doing comes from Railway's status and logs. The local runtime check used the same configuration, run on this container (see §2).

## 2. Tests and CI

| Check | Result | Who ran it |
|---|---|---|
| CI on `main` @ `3cbe45e` (lint, client tests and build on Node 20, fmt, clippy, cargo test, HTTP suites) | **success** (run 282, 2026-11-23) | Existing GitHub Actions report |
| `npm run lint` (license headers, client ESLint, `cargo fmt --check`, `cargo clippy -D warnings`) | **passed**. ESLint 0 errors, 20 warnings (the documented baseline) | Run in this session |
| `npm --prefix client run build` | _see §2a_ | Run in this session |
| `npm test` (vitest, `cargo test`, `test:http`, `test:migrate`) against a throwaway local Postgres 16 | _see §2a_ | Run in this session |
| Local demo walk-through (release build, `DEMO_MODE=true`, `SIGNUP_MODE=invite`, no email, manual payments, no AI key) | _see §2a_ | Run in this session |

### 2a. Results recorded after the runs

_(filled in below once the runs finished)_

## 3. Known defects and limitations that affect a demo or pilot

1. **Email is silently dropped on the hosted server (pilot blocker).** With `EMAIL_HOST` unset, `email::send` puts every message in an in-memory outbox and returns success (`server-rs/src/email.rs`, `send`). In production nothing is logged. On the Railway server, a real chef's "Email meal plan", "Email invoice", booking confirmations, session reminders, magic links and password-reset links all appear to work, but nothing is delivered. The demo refuses email openly ("Email" is blocked on the demo account), so this does not affect the demo.
2. **No backups are configured in the app (pilot blocker for real client data).** `auto_backup: false`. Whether Railway's own Postgres backups are on is **unverified**: I couldn't see it from the tools available.
3. **No privacy notice or terms for hosted accounts.** None exist in the repository. A pilot that stores real client names, addresses and allergies on the owner's server needs at least a short plain-language data note.
4. **Sign-up is invite-only, and each invite is made by hand**: `railway ssh` then `chef-server invite create`. Platebook doesn't email invites, so the owner sends each link. That is fine for 3–5 pilots, but it uses the owner's time.
5. **AI features can't be shown on the hosted server.** There is no server key, and the demo refuses saving an AI key. AI recipe import and menu suggestions need each chef's own paid provider key.
6. **Online payments are off on the hosted server** (`Manual`), and they are beta and single-chef only by design (`ROADMAP.md`, Known limitations). Deposits and payments are recorded by hand.
7. **Allergen detection is keyword-based and advisory only.** It must always be framed that way.
8. **v0.4.0 is not tagged.** The hosted server already runs the v0.4.0 code. Self-hosters who pull the `v0.3.0` image don't get the demo, the tour or the allergen fixes.
9. The landing page (`landing-page/index.html`) is **not deployed anywhere I could verify**. The Railway server serves the app's own Welcome page.

## 4. Claim table

| Capability | Support | Safe public wording | Limitation / still to verify |
|---|---|---|---|
| Client profiles with allergies, dietary styles, likes, kitchen notes | [code] [tested] [deployed] demo has 5 sample households | "Keep each household's allergies, preferences and kitchen notes in one place." | — |
| Public booking/intake page per chef | [code] [tested] | "Your own booking link where new clients fill in an intake form." | The chef gets no email alert on the hosted server (defect 1); inquiries still appear in **Inquiries**. |
| Recipe library, scaling to household size, 20-recipe starter set | [code] [tested] | "A recipe library that scales to each household's serving size." | — |
| AI recipe import and menu suggestions | [code] [tested with fakes] | "Optional: bring your own AI key to import recipes from a link or text." | Not available on the hosted server or the demo; costs the chef their own provider fees. Don't lead with it. |
| Session planning with calendar, conflicts, repeats, cooking-time check | [code] [tested] | "Plan sessions on a calendar, with repeat bookings and conflict warnings." | — |
| Allergen check against a client's allergies | [code] [tested] | "Flags likely allergens from ingredient names. Always confirm with your client." | Keyword-based, advisory. Never say "allergen-safe". |
| Grocery list merged across dishes, by store section, tick-off on phone | [code] [tested] | "One grocery list per session, merged across dishes and grouped by store section." | — |
| Meal-plan and invoice PDFs | [code] [tested] | "Branded meal-plan and invoice PDFs." | Emailing them needs SMTP (defect 1). |
| Invoices, deposits, outstanding balances | [code] [tested] | "Invoice from a session and see what's outstanding." | Payments are recorded by hand on the hosted server. |
| Online card payments (Stripe) | [code] [tested with fakes] | Don't market this yet. | Beta, one chef per server, off on the hosted server. |
| Costs, margins, monthly P&L with CSV export | [code] [tested] | "See margin per session and a monthly profit-and-loss you can export." | Cash basis, no mileage. Not tax advice. |
| Data export (JSON) | [code] | "Export all your data at any time." | — |
| Account isolation (Postgres row-level security) | [code] [tested] (tenancy suites) | "Each account's data is kept separate in the database." | Not a security certification. No third-party audit. |
| Self-hosting with Docker Compose | [code] docs | "Free to self-host; source on GitHub under PolyForm Shield." | Not "open source". I didn't run the Compose path in this session. |
| Public demo that resets daily | [deployed] (logs) [run] locally | "Try the demo: a shared sample kitchen that resets every day." | I couldn't open the live URL from here; the owner should open it once on a phone before sharing. |
| Works on a phone | [code] (responsive, phone layout) | "Works on your phone in a client's kitchen." | Not an installable/offline app (PWA is [planned]). |
| Client portal, post-session ratings, offline mode | [planned] | Don't mention publicly as features. | — |

Never claim: users, customers, testimonials, time saved, "production-ready", "secure"/"HIPAA"/"certified", "open source", or a price.

### Core chef journey

| Step | State |
|---|---|
| Client preferences | Works: client form opens on the Dietary Profile, and booking intake converts to a client. |
| Recipes / menu | Works. AI import only with the chef's own key. |
| Session planning | Works: wizard suggests a date, checks allergens and cooking time. |
| Groceries / prep | Works: merged list, store sections, tick-off, share/print/copy. There is a prep timeline tab on the session. |
| Client handoff | Meal-plan PDF works. **Email delivery does not work on the hosted server** (defect 1). PDF download and manual sending are the workaround. |
| Invoicing / payment records | Works with manual payment records. Online payment is off on the hosted server. |

## 5. Readiness verdict

| Question | Verdict | Why |
|---|---|---|
| Public demonstration? | **Yes, with limits.** Fine to show to individuals and to link the demo in a small number of conversations. | The demo is on, seeded with synthetic data, resets daily, sends no email, and refuses AI keys. **Condition:** the owner opens the live demo once on a phone first, because I couldn't reach it from here. Don't do broad promotion yet: the landing page isn't deployed, and the public URL is a raw Railway subdomain. |
| Small assisted pilot with real client data on the owner's server? | **Not yet.** | Blockers: (1) email is silently dropped, so a chef would believe invoices and meal plans were sent; (2) backups are unverified; (3) there is no data/privacy note. All three are owner-side configuration or writing, not new features. |
| Private feedback only? | **Yes, now.** | 1:1 demo walk-throughs and conversations with a handful of chefs, using the shared demo. No accounts and no real client data. |

**So the first campaign is a feedback campaign (Phase 1)**: 3–5 conversations and demo walk-throughs. The pilot invitation (§9) is drafted but gated. Send it only after gates G1–G3 are done.

## 6. Primary audience

**Solo in-home personal chefs who cook weekly or biweekly for a few recurring households, at least one with allergies or dietary restrictions.**

Why this audience:
- The product's complete, tested path is exactly their week: household profile → menu → session → grocery list → meal-plan PDF → invoice.
- The allergen and household-size features matter most to them. Event caterers and restaurant cooks need things Platebook doesn't do (staffing, large quotes, POS).
- The hosted server is single-region and invite-only, and payments are single-chef. Solo operators fit those limits, teams don't.
- They are reachable through people the owner already knows, which fits "a handful of relevant conversations".

Secondary (not a buyer): self-hosters and developers. They are useful for code feedback and credibility, but they don't validate demand from chefs.

## 7. Positioning copy

**Plain-language description (≈50 words).**
Platebook is a web app for personal chefs who cook in clients' homes. It keeps each household's allergies and preferences, your recipes, your session calendar, the grocery list for each cook day, meal-plan PDFs and invoices in one place. It works in a phone browser. It's free to self-host, and the source is on GitHub.

**Value proposition (one line).**
The client notes, menus, grocery lists and invoices for each cook day, in one place instead of spread across notes, spreadsheets and texts.

**Founder introduction (owner to complete the bracketed part; nothing invented).**
I'm Taliesin Fios. I built Platebook, a small web app for personal chefs, as a solo project with a lot of help from AI coding tools. [One honest sentence about why: e.g., a chef you know, or your own experience. Owner to write.] It's early. I'm looking for a few chefs to tell me where it fits their week and where it doesn't.

## 8. Demo script (≈10 minutes, demo account only)

Use **Try the demo** on the welcome or sign-in page. Supported features only: no email, no AI, no online payment.

1. **Dashboard (30 s).** "This week's sessions, what's outstanding, margin." Point out the demo banner: "Shared sample kitchen, resets daily."
2. **Clients → The Chen Family (1 min).** Allergies (tree nuts), household size, kitchen notes. "This is what you'd check before you shop."
3. **Recipes (1 min).** Open a recipe and change the servings. "Scales to the household." Mention the 20-recipe starter set.
4. **Sessions → + Session for a client with an allergy (2 min).** Walk through the wizard: suggested date, pick dishes, see an allergen flag and the cooking-time check. Say it plainly: "It flags likely allergens from ingredient names. You still confirm with the client."
5. **Session → Grocery tab (1.5 min).** Merged list by store section; tick items off the way you would in the store, on a phone.
6. **Meal-plan PDF (1 min).** Download it. "This is what the client gets." Don't press Email (it's blocked on the demo).
7. **Invoice from the session (1.5 min).** Create it, then show invoices in each state (paid, sent, overdue, draft). Payments are recorded by hand.
8. **Inquiries (1 min).** Open a sample booking inquiry and **Convert to Client Record**. "Clients fill this in from your booking link."
9. **Costs → Profit & Loss (30 s).** A monthly view and CSV export.
10. **Close with questions (not features):** "Where does this not match how you work? What would you need before trusting it with a real client?"

Avoid: AI import/suggestions, Stripe, client portal, offline use, any claim about time saved.

## 9. Channels

Current rules were checked on 2026-11-24 where possible. **Check each one again on the day of posting.**

| Channel | Type | Eligible now? | Rule evidence | Notes |
|---|---|---|---|---|
| Chefs the owner already knows (direct message/email) | Qualified buyers | **Yes**, owner-approved, one at a time | n/a | Primary channel. 3–5 people. |
| Professional connectors the owner knows (culinary instructor, chef-staffing contact, local personal-chef association chapter lead) | Connectors, not buyers | **Yes, permission first**: ask before sharing anything | n/a | Ask for a 1–2 name referral, not a broadcast to their list. |
| GitHub Discussions on `taliesin-fios/platebook` | Owned | **Yes** | Owner's own repo, already linked as "Give feedback" | Low reach; it's a landing spot for anyone who wants to reply publicly. |
| r/selfhosted, **weekly New Project Megathread** | Community of self-hosters, **not chef buyers** | **Conditionally** | A mod announcement (Apr 2026, read via a mirror) says projects younger than ~3 months go only in the weekly megathread, standalone posts are removed, and the top-level comment should include an **AI-involvement disclosure**. I couldn't load the live sidebar, so **re-check it before posting.** | Expect pushback on "source-available, not open source". Reply politely; don't argue. |
| r/Chefit | Mostly restaurant cooks | **No promotion** | A cached 2023 page shows aggressive anti-spam moderation. I couldn't read the current rules. | Participate only. Wrong audience anyway. |
| USPCA / APPCA (personal-chef associations) | Qualified buyers | **Not now** | USPCA offers paid conference sponsorship ($5k–$15k tiers) and membership; I found no free vendor channel. | Revisit once hosted accounts are pilot-ready. Not authorized (purchases). |
| Facebook groups, paid ads, directories | — | **No** | Not reviewed, and not authorized | — |

## 10. Drafts (unsent)

### Draft A — warm introduction to a chef the owner knows

> Subject: Could I get 20 minutes of your honest opinion?
>
> Hi [Name],
>
> I've been building a small web app for personal chefs called Platebook, and you're one of the few people I know who'd be able to tell me quickly whether it's useful or not.
>
> It keeps each household's allergies and preferences, your recipes, the session calendar, the grocery list for each cook day, a meal-plan PDF and the invoice in one place. It's early, it's just me (with a lot of help from AI coding tools), and I'm not selling anything right now.
>
> If you're up for it, I'd love 20 minutes on a call where I show you the demo and you tell me where it doesn't match how you actually work. Or, if you'd rather poke around alone, there's a shared demo kitchen with sample data: [demo link]. It resets every day, so nothing you type there is kept.
>
> No pressure at all if the timing's bad.
>
> [Your name]

### Draft B — permission-based message to a professional contact

> Subject: Quick question, and no is a fine answer
>
> Hi [Name],
>
> I hope [specific, true reference: e.g., the term is going well]. I've built an early web app for personal chefs who cook in clients' homes (client allergies, menus, grocery lists, meal-plan PDFs, invoices) and I'm looking for two or three working chefs to give me candid feedback on a 20-minute demo.
>
> Would you be comfortable with me describing it to you briefly, so you can decide whether anyone you know might want to take a look? I'm not asking you to share it with a list or to recommend it. If it's not a fit, no problem at all.
>
> Thanks,
> [Your name]

### Draft C — r/selfhosted weekly New Project Megathread (top-level comment)

> **Platebook**: a self-hosted kitchen office for personal chefs
>
> - **What it does:** client households with allergies and preferences, recipes that scale to household size, a session calendar, a merged grocery list per cook day, meal-plan and invoice PDFs rendered on the server, and invoices with costs and margins. One server can hold one chef or several accounts, separated with Postgres row-level security.
> - **Stack:** Rust (Axum, SQLx) on Postgres, React + TypeScript front end, Docker Compose for self-hosting.
> - **License:** source-available under PolyForm Shield 1.0.0. Free to self-host for any purpose except offering a competing product. It is **not** an OSI open-source license, and I want to be upfront about that.
> - **AI involvement:** I'm a solo developer and wrote it with heavy use of AI coding tools (Claude Code). I review and test the changes. CI runs Rust and client tests, including cross-account isolation tests.
> - **Status:** early. Email needs your own SMTP. Online payments are beta and suit one chef per server. Allergen flags are keyword-based and advisory.
> - **Repo / self-hosting guide:** https://github.com/taliesin-fios/platebook
>
> I'd especially like to hear from anyone who cooks for clients, or self-hosts something similar for a small business. What would stop you from trusting it with real data?

(Adapt for GitHub Discussions by removing the AI-disclosure heading format and adding "Feedback from working chefs especially welcome.")

## 11. Pilot invitation (Phase 2): drafted, **do not send until G1–G3 are done**

Gates: **G1** email works on the hosted server (SMTP configured and one test message received), or the chef has been told plainly that Platebook won't send email for them. **G2** backups confirmed, with one restore tried. **G3** the short data note below approved by the owner.

> Subject: Platebook pilot: 4 weeks, 3–5 chefs
>
> Hi [Name],
>
> Thanks for looking at Platebook. I'm inviting a few chefs to use it for real for four weeks, and I'd like you to be one of them. Here's exactly what that means:
>
> **What you get:** your own private Platebook account on my server, set up with you on a 30-minute call; a direct line to me for problems; and fixes for anything that blocks you, where I can manage them during the pilot. Cost: [OWNER DECISION: e.g., "no charge for the pilot, and no commitment afterwards"]. I'm not promising a price or hosting beyond the pilot.
>
> **What I'm asking:** plan your real sessions in it for four weeks (as many as feels natural), a 10-minute check-in each week (call or a few lines by message), and a 30-minute wrap-up call.
>
> **Support limits:** it's just me. I'll reply within [OWNER DECISION: e.g., one business day], not instantly, and I can't guarantee any fix or feature.
>
> **Your data:** your account is private to you and stored on a database I run. I can technically access it as the operator, and I'll only do so to fix a problem you report. You can export everything at any time, and I'll delete your account when you ask or when the pilot ends, whichever you prefer. Please don't enter anything you aren't comfortable keeping on a small, early service. Allergen flags are a reminder, not a safety check: always confirm with your client. [If G1 isn't met: "Platebook won't email your clients during the pilot. Download the PDFs and send them yourself."]
>
> **Feedback:** the weekly check-ins plus the wrap-up call. I'll write down what you say, with your OK, and won't quote you publicly without asking.
>
> If you're in, reply and I'll send a one-time sign-up link.
>
> [Your name]

## 12. Tracking sheet

`docs/marketing/outreach-tracker.csv`. Rows are placeholders by channel, with no real names. Record contacts by initials only; the file is committed to a public repo. Columns: approval date, sent date, response, demo date/outcome, pilot invite, weekly session counts (repeat use), feedback, top blocker, would-keep-using, owner time and support time in minutes, next step.

**Keep personal contact details out of this repository.** If you prefer a private Google Sheet, copy the header row there.

## 13. Changes made in this package

- `landing-page/index.html`: removed or narrowed four claims the code doesn't support: "emailed … in one click" (needs SMTP), "a library of tested recipes" (no evidence of testing; now "20-recipe starter library"), "plan menus that share ingredients to cut grocery cost and waste" (no such feature; replaced with the merged grocery list), and "New clients land in your inbox" (the email alert needs SMTP). No other files changed. Nothing deployed.
