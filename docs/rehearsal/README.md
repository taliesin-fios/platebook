# Email truthfulness fix and chef/customer rehearsal — November 2026

Branch `claude/exciting-thompson-91l86p`. Nothing was deployed, merged, tagged, or run against
production. All data is fictional (`@example.com`), and every email went to a local mail sink.

| Read this | For |
|---|---|
| **This page** | What changed, what was tested, results, and what is still open |
| [EVIDENCE-MATRIX.md](EVIDENCE-MATRIX.md) | Every scenario: expected, observed, pass/fail, and where the evidence is |
| [WALKTHROUGH.md](WALKTHROUGH.md) | Screenshot walkthrough with captions, plus the screen recording |
| [OWNER-TEST-CARD.md](OWNER-TEST-CARD.md) | A 15-minute normal path and deliberate failure checks you can run yourself |
| [DEFECTS.md](DEFECTS.md) | Prioritised defects with reproduction steps, kept apart from friction and missing features |
| [CLAIMS.md](CLAIMS.md) | Which marketing claims this work supports, and which it doesn't |
| [demo/](demo/) | Repeatable demo: environment, fixtures, seed, recorded walkthrough, retest |
| [evidence/](evidence/) | Raw evidence: tester notes, screenshots, PDFs, exports, test logs |

The "current state and outreach" document mentioned in the brief was not in this environment. I
treated its claims as unverified and worked from the source code.

## 1. What changed (the email fix)

**Root cause.** `email::send` returned success whenever SMTP wasn't configured: it put the message
in an in-memory outbox and reported "sent". As a result:

- invoices became **Sent** and got an `emailed_at` time;
- meal plans and booking confirmations showed "sent";
- reminders were marked sent;
- the sign-in and reset pages said a link was "on its way".

Dev logs also printed full message bodies, including **live sign-in and reset tokens**. I reproduced
this on the original code (`evidence/logs/02-red-token-in-logs-baseline.txt`, tokens redacted).

**Fix.** It is proportionate: no queue, and no migration.

- **Three explicit email modes** (`Config::email_mode`):
  - **SMTP**: `EMAIL_HOST` set; user and password optional but must be set together; `EMAIL_FROM` must be a valid address.
  - **Capture**: `EMAIL_CAPTURE=true`. Dev and tests only; refused when `NODE_ENV=production` or alongside `EMAIL_HOST`. Shown as "captured", never "sent".
  - **Disabled**: anything else.
  - Missing, partial or unusable settings count as **Disabled**. They are never silently captured.
- **No false state.**
  - Invoice status and `emailed_at`, `pdf_emailed_at`, `confirmation_emailed_at` and the reminder "sent" markers change only after real SMTP acceptance.
  - Failures are recorded (`*_failed_at`), and a booking still saves when its emails fail.
- **Truthful responses.**
  - `503 EMAIL_NOT_CONFIGURED`: no transport.
  - `502 EMAIL_SEND_FAILED`: unreachable or temporary failure; try again.
  - `502 EMAIL_REJECTED`: permanent rejection; check the address.
  - A refused SMTP login reads as a settings problem.
  - A successful send says "**accepted by the mail server**" (`delivery: "smtp"`). Platebook can't see whether mail reached the inbox, so it never says "delivered".
- **Chef recovery.** A persistent banner says "…wasn't sent" and offers **Download PDF** and **Try again**. On a server with no email, Try again is hidden (`/api/auth/me` now includes `email_available`).
- **Authentication privacy.**
  - Sign-in and reset requests return identical responses for real and unknown addresses (`email_available` is a server-wide setting).
  - The email is sent after the response, so response time no longer reveals whether an account exists.
  - Tokens never appear in responses (outside the existing dev-only `ALLOW_DEV_LINK`) or in logs.
- **Robustness.**
  - Each SMTP attempt is time-limited (`EMAIL_TIMEOUT_SECS`, default 20). A server that never answered used to hold the request for **242 s**.
  - A timed-out attempt isn't retried, because a retry could send twice.
  - Repeated clicks send once. The browser ignores clicks while a send is running, and the server refuses a repeat of a successful send within 10 s (`409 EMAIL_JUST_SENT`).
  - Paid and void invoices can't be emailed (`409 INVOICE_CLOSED`).
- **Booking page.** "Confirmation sent to …" is now "We'll email a confirmation to …" when a send will be attempted, or "No confirmation email will be sent, so keep this reference number".
- **Supporting changes.** Settings `EMAIL_TLS` (including `none` for a local sink) and `EMAIL_TIMEOUT_SECS`; docs (README, SELF_HOSTING, ARCHITECTURE, `.env.example`) and a CHANGELOG entry with upgrade notes; and a zero-dependency local SMTP sink (`tools/dev-mail-sink`).

Every caller of the shared email function was checked and updated:

- invoice email
- meal-plan email (session page and session list)
- booking confirmation (at submission and on resend)
- new-booking alert to the chef
- day-before reminders to client and chef
- magic link
- password reset

**Commits on the branch** (author: the maintainer; AI help credited in trailers):
`8a6ddd0` fix → `820e322` first-review fixes → `b27abe6` inquiry badge wording → `4fea505` repeat-click
guard and paid-invoice banner → `0a40d19` "couldn't be reached" wording → `dec3af1` second-review and
rehearsal fixes → `0955db4` demo run recorded at `dec3af1`. Rehearsal tooling and evidence are in
`d9abeac`, `6aa278b` and the earlier `Rehearsal:` commits.

## 2. How it was tested (tested commit: `dec3af1`)

| Check | Command | Result |
|---|---|---|
| Regression tests written first | `cargo test --test email_delivery` on the unfixed code | 4/4 **failed for the expected reason** (`evidence/logs/01-red-email-regression.txt`). The log leak (`02-…`) and the 242 s hang (`03-…`) were shown failing too. |
| Rust format and lint | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | clean |
| Rust tests | `cargo test` | **274 passed**, 0 failed, 1 ignored (already ignored before this work) — `evidence/logs/10-cargo-test.txt`. Baseline before the work: 250 passed (seen in the session output; not saved as a log). |
| Email tests (new) | `email_delivery.rs` (7), `email_transport.rs` (14, fake SMTP: accept, 550, 451+retry, refused, silent/timeout, 535, recovery, repeat clicks, paid/void, link timing) | all pass |
| HTTP suites | `npm run test:http` | 85/85 |
| SQLite migrator | `npm run test:migrate` | 6/6 + end-to-end passed (fixture updated for `email_available`) |
| Client | `npm --prefix client run lint` / `test` / `build` | 0 errors, 20 warnings (the same 20 as before; the before-count is from the session output); 172 tests; build clean, no chunk warning — `evidence/logs/13-client-and-headers.txt` |
| License headers | `node scripts/license-headers.mjs` | 191 files OK — `evidence/logs/13-client-and-headers.txt` |
| Browser demo | `node docs/rehearsal/demo/run-all.mjs` | **29/29** outcome checks, **4/4** rapid-click rounds, 1 known defect reported (`evidence/demo-run/results.json`, `evidence/logs/21-run-all-demo.txt`) |
| Independent review | two passes by a separate reviewer agent | every material finding fixed (see §4) |

Where these ran: Linux container, Node 22, Rust 1.97, Postgres 16. Local Node is 22, not the 24
CLAUDE.md mentions; CI uses Node 20.

The container's Postgres was started for this work. An auto-mode permission check blocked changing
its `pg_hba.conf` to password-free access, so I set a throwaway password on the local `postgres` role
and passed it in the URLs instead.

## 3. Rehearsal (Phase 2)

AI agents, each with one role, used a real Chromium browser against an isolated, production-mode
instance. Its mail went to the local sink, and a second instance on the same database had no email
configured. The roles:

- **Chef**, in three legs: signup → recipes; inquiries → grocery list; PDF → invoice → email failures → isolation.
- **Customer**: three households booking through the public link.
- **Skeptical observer**: audited the evidence.

The testers worked from goals, not click paths, and were told not to read the source code. They
disclosed three places where they needed help:

- **Leg 2 broke the no-source rule.** To confirm that sessions can't be rescheduled, it read
  `client/src/App.tsx` and `SessionDetail.tsx`.
- **Leg 3** found the hidden client Delete buttons through the accessibility tree.
- **Leg 3** guessed the payment-method workflow (it put Zelle in the internal notes).

**Limits of this method:**

- **Priming.** Later legs read earlier legs' notes; for example, leg 1 asked leg 2 to check the
  Castellano grocery quantities.
- **Not a human's view.** The agents read pages through the DOM and accessibility tree, not by eye.
  That is how hidden buttons and missing labels surfaced, and it differs from what a human sees.
- **API checks.** Testers made read-only API calls with their session cookie, to confirm saved state
  after a UI step.
- **Not independent retests.** The rapid-click retests (`evidence/logs/20`,
  `demo/retest-rapid-clicks.mjs`) were run by the engineer who wrote the fix.
- **AI reviewers.** The skeptical observer and the code reviewer are AI agents too.
- **Shared database.** Two servers shared one database, one with email and one without. A few
  findings come from that setup rather than from a single real server: leg 2 M1, leg 3 M3 and M5. The fictional week is in `demo/fictional-week.json`:

- **Okafor**: four people, peanut and tree-nut allergy, weekly.
- **Lindqvist**: two people, vegetarian and celiac, moved from Wednesday to Thursday.
- **Castellano**: six people, dairy-free, with a repeat booking.

**This is synthetic testing.** It does not show that real chefs would adopt the app, that customers
want it, or that anyone enjoys it. Phone results come from 375 px browser emulation, not a physical
phone. Email was observed only as acceptance by a local SMTP sink: no real provider, TLS login or
inbox was involved.

Highlights:

- The email work held up in the browser. Wording was truthful, banners stayed after reload, saved
  state was correct after each failure, and the sink contents matched. The link pages answered the
  same for real and unknown addresses.
- The rehearsal found three email defects after the first fix: five duplicate meal plans from rapid
  clicks, a stale "Try again" on a paid invoice, and "didn't accept" shown when the server was
  unreachable. All three are fixed and retested.
- Many product problems unrelated to email surfaced. They are in [DEFECTS.md](DEFECTS.md) and were
  **not** fixed, because they are out of scope.

## 4. Status

**Fixed and verified locally** (at `dec3af1`):

- Every item in §1.
- All material findings from both review passes:
  - the "Confirmation sent" copy on the booking page;
  - the timing difference in the link flows;
  - an invalid `EMAIL_FROM` counting as available;
  - SMTP error wording;
  - no retry after a timeout;
  - the no-email server rendering PDFs without limit;
  - paid/void invoices being emailable through the API;
  - "Try again" offered on a server with no email;
  - the out-of-date demo evidence.
- All email findings from the rehearsal.

**Not deployed and not verified on the hosted server.** Nothing touched Railway or production. The
hosted server's actual `EMAIL_*` settings are unknown. After deploying, check the startup log for
either "SMTP transporter verified" or "Email is not configured…".

**Blocked or untested:**

- A real SMTP provider: TLS, STARTTLS and login, plus delivery to a real inbox (this environment has no provider credentials, and the brief forbids real email).
- A physical phone.
- Reminders sent through a real mail server; they're covered only by cargo tests.
- Cross-account write attempts in the browser (reads were tried; writes are covered by the existing `tests/tenancy.rs` and `tests/http/tenancy.test.js`).
- Recurrence that actually generates the next session.
- Feasibility with real cooking times.
- AI recipe import (no AI key, by design).

**Known limits of the fix:**

- Two tabs sending within the same few milliseconds could still both send; the 10 s window covers sequential repeats only.
- Captured mail (dev only) still sets the "sent" timestamps in a dev database.
- An `EMAIL_HOST` that isn't a valid host name is reported as available until the first send.
- Temporary failures aren't retried automatically; the chef presses Try again.
- Link emails still in progress are dropped if the server shuts down.

**Deferred** (not about email): everything in [DEFECTS.md](DEFECTS.md) §P1–P3. The most important is
**P1-1**: on a server with several accounts, a booking link of the form `/intake/<anything>` sends the
customer's details, allergies included, to account #1.

## 5. Smallest next actions for you

1. Review and merge when ready. There are no migrations; the PR is not opened yet. Read the upgrade notes in `CHANGELOG.md` first: `EMAIL_HOST` with no login now means "SMTP without authentication", and a plain-text relay needs `EMAIL_TLS=none`.
2. After deploying, read the startup log line about email, and send one invoice to an address you control. That is the first real-inbox check.
3. Decide on DEFECTS P1-1 (`/intake/<id>` misrouting) before inviting a second chef onto a shared server.
