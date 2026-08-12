# Catalog Web

## End-to-End Tests

The Playwright suite starts an isolated PostgreSQL container with Testcontainers,
then launches the API and Vite on E2E-only ports. It does not use or modify the
local development database.

Start a Docker-compatible container runtime, install Playwright's browser once,
then run the suite:

```sh
npx playwright install chromium
npm run test:e2e
```
