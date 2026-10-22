# Security policy

Platebook stores chefs' client details (names, addresses, allergies) and payment records, so security reports are very welcome.

## Reporting a vulnerability

Please **don't open a public issue.** Report it privately through GitHub: go to the repository's **Security** tab and choose **Report a vulnerability**. Include what you found, how to reproduce it, and what an attacker could do with it.

You'll get an acknowledgement within a week. Once a fix is released, we'll credit you in the advisory unless you'd rather stay anonymous.

## Scope

In scope: the code in this repository, especially:
- isolation between accounts (one chef seeing another chef's data);
- authentication, magic links and password resets;
- the public booking and intake routes;
- payments webhooks;
- server-side requests made by recipe URL import.

Out of scope:
- issues in a particular deployment's configuration, such as a weak `JWT_SECRET`, the app exposed without a reverse proxy, or missing HTTPS;
- denial of service by volume;
- reports from automated scanners without a demonstrated impact.

## Supported versions

Only the latest release gets security fixes. Upgrade with `git pull` and a rebuild (see [docs/SELF_HOSTING.md](docs/SELF_HOSTING.md#upgrading)).
