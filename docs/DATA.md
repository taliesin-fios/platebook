# What data Platebook stores, and who can see it

*Draft, November 2026. Plain-English summary for chefs and their clients. It describes the software; whoever runs a Platebook server (the "server owner") decides where it is hosted, how it is backed up and which outside services are switched on.*

## What is stored

On the Platebook server's database:

- **The chef's account:** email address, password (stored only as a one-way hash), business name and details, booking policy, logo, time zone and other settings. If the chef adds an AI key, it is stored encrypted.
- **Clients:** names, contact details, address, household size, allergies, dietary preferences, likes and dislikes, kitchen notes and the chef's private notes.
- **Booking inquiries** sent through the chef's booking page: everything the person typed in, including allergies, and whether they accepted the booking terms.
- **Work records:** recipes, sessions, menus, grocery lists, invoices and payment records (amounts, dates, status, and for online payments a Stripe reference, not card details).
- **Security records:** sign-in times, short-lived records that stop links being reused, and rate-limit counters that briefly hold IP addresses.

In the browser, Platebook sets one sign-in cookie, and remembers whether the chef has seen the welcome tour. It uses no advertising or analytics trackers.

## Who can see it

- **The chef** sees only their own account's data. Each chef's data is kept apart inside the database, and every screen checks it, so one chef cannot open another chef's clients, sessions or invoices.
- **A client** sees only what the chef sends them: their own booking request (through a private link in their confirmation email), their invoices (through a private link) and the PDFs the chef emails them. Anyone who has one of these links can open it, so treat them like a private email.
- **Anyone** can see the chef's public booking page: business name, logo, location, website and booking policy. Nothing about clients is on it.
- **The server owner** (and their hosting provider) can technically reach the database, its backups and the server logs. On a hosted Platebook server this is the operator, not the chef.
- **The public demo**, where switched on, is a shared account that anyone can open and that resets itself regularly. Don't put real information in it.

## Outside services (only when switched on)

- **Email:** messages to clients (invoices, meal plans, booking confirmations, reminders) go through the email provider the server owner sets up. That provider handles the message contents.
- **Online payments:** with Stripe switched on, clients pay on Stripe's own page; Stripe sees the payment details. Platebook never sees card numbers.
- **AI features:** these are off unless a key is set up. Recipe import sends the recipe text or web address to the chef's chosen AI provider. Menu suggestions send the client's household size, allergies and food preferences, but not their name or contact details.

## Keeping and deleting

- Data stays until the chef deletes it. Deleting a client or recipe hides it but keeps it for past sessions and invoices. Payment records are kept as an audit trail.
- A chef can **export** all of their data (Settings → Data Management) and **delete their account**, which removes all of their data from the server at once.
- Copies may remain in database **backups** until those backups expire, on the server owner's schedule.

## Questions

Clients should ask their chef first: the chef decides what to record about them and can correct or delete it. Chefs can ask the server owner about hosting, backups and email.
