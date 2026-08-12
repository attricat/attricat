# Catalog Web

See the repository's [frontend conventions](../../docs/frontend.md) for code
structure, TypeScript, data-fetching, and testing expectations.

## End-to-End Tests

The Playwright suite starts an isolated PostgreSQL container with Testcontainers,
then launches the API and Vite on E2E-only ports. It does not use or modify the
local development database.

The setup resolves the active Docker context automatically, including Colima.
It disables Testcontainers' Ryuk cleanup sidecar because Colima cannot mount its
Docker socket into that container; Playwright global teardown stops PostgreSQL,
the API, and Vite instead.

Start a Docker-compatible container runtime, install Playwright's browser once,
then run the suite:

```sh
npx playwright install chromium
npm run test:e2e
```

The suite runs serially and seeds its own blueprints and entities through the
API, so it is safe to run alongside the normal development server.
