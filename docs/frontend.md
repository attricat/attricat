# Frontend Conventions

These conventions apply to `apps/catalog-web`.

## TypeScript

- Use arrow functions for TypeScript functions.
- Use semicolons and two-space indentation.
- Keep feature code under `src/features/<feature>`.
- Prefer relative imports within and between features; no path aliases are configured.

## Data Fetching

- Keep API request functions and response schemas in each feature's `api.ts`.
- Define TanStack Query key factories in a feature-local `query-keys.ts` file.
- Use those factories for every `queryKey` so equivalent requests share the same
  cache entry and invalidation can reuse the same key definitions.

## Components

- Keep route files thin and compose feature page components from
  `src/features`.
- Use TanStack Form for form state and Material UI for interface components.
- Validate API payloads with Zod before using them in the UI.
- Blueprint responses can include JSON Schema contracts. Use the feature-local
  Ajv helper for immediate form feedback, but treat server-side `422` schema
  validation as authoritative.

## Testing

- Run unit tests with `npm test --prefix apps/catalog-web`.
- Run the Playwright suite with `npm run test:e2e --prefix apps/catalog-web`.
- See [the documentation index](index.md#test-the-web-app) for E2E setup and
  Docker/Colima behavior. Do not point it at or seed the development database.
