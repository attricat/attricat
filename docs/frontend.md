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

## Testing

- Run unit tests with `npm test --prefix apps/catalog-web`.
- Run the Playwright suite with `npm run test:e2e --prefix apps/catalog-web`.
- The E2E suite uses Testcontainers with an isolated PostgreSQL database and
  starts its own API and Vite processes. Do not point it at or seed the
  development database.
- The suite resolves the active Docker context automatically. With Colima, make
  sure `colima status` reports a running Docker runtime.
