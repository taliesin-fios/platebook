## What and why

<!-- What does this change, and what problem does it solve? Link the issue if there is one. -->

## Checklist

- [ ] `npm run lint`, `npm test` and `npm run build` pass locally
- [ ] New data routes have cross-account tests (`server-rs/tests/tenancy.rs`, `tests/http/tenancy.test.js`)
- [ ] A new migration (if any) is a new numbered file and safe on a database with data
- [ ] New public routes are in `auth::is_public_route` and the ARCHITECTURE.md allowlist
- [ ] UI changes checked at phone width (375px)
- [ ] Commits are signed off (`git commit -s`), see CONTRIBUTING.md
