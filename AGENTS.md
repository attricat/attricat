# TypeScript

Prefer arrow function syntax for TypeScript functions.

Use semicolons and two-space indentation.

# Development

The development server is already running via Process Compose when developing the app.

Use SQLx for all migration operations.

# Web E2E Tests

Run `npm run test:e2e --prefix apps/catalog-web` for the Playwright suite. It
uses Testcontainers to create an isolated PostgreSQL database, then starts its
own API and Vite processes; do not point it at or seed the development database.

The suite resolves the active Docker context automatically. Colima users only
need to ensure `colima status` reports a running Docker runtime.
