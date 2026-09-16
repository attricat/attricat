# Frontend Conventions

These conventions apply to `apps/catalog-web`.

## TypeScript

- Prefer arrow functions for TypeScript functions; route declarations may use a named function when it improves stack traces or route readability.
- Use semicolons and two-space indentation.
- Keep feature code under `src/features/<feature>`.
- Prefer relative imports within and between features; no path aliases are configured.

## Data Fetching

- Keep API request functions in each feature's `api.ts` and their Zod request/response schemas in that feature's `schemas.ts`.
- Route browser API calls through `src/api/request.ts`: use `request` for Zod-validated JSON, `requestNoContent` for successful empty responses, and `requestText` only for deliberately non-JSON or bounded extension responses. Do not call `fetch` or `apiFetch` from feature API clients.
- Preserve endpoint-specific semantics by handling `ApiRequestError` at the feature boundary (for example, an unauthenticated session may map HTTP 401 to `null`); all other API failures must retain the shared structured error.
- Define TanStack Query key factories in a feature-local `query-keys.ts` file.
- Use those factories for every `queryKey` and invalidation so equivalent requests share the same cache entry and invalidation can reuse the same key definitions. Define a root key when a feature needs to invalidate all variants of a resource.

## Components

- Keep route files thin and compose feature page components from
  `src/features`.
- Keep one route-level page component per feature module. Extract independent
  pages and substantial page sections into descriptive sibling modules rather
  than growing a multi-route page file.
- Use TanStack Form for submitted field values, validation, reset behavior, and submission handling; reserve React state for non-form UI state such as dialogs, notices, and upload progress. Use Material UI for interface components.
- Validate API payloads with Zod before using them in the UI.
- For imperative integrations that retain a callback (for example Monaco commands or browser event listeners), do not capture render-time values that can change. Re-register and dispose the callback when dependencies change, or read current values through refs.
- A disabled or readonly field must disable every mutation path, including inline chip removal, keyboard shortcuts, and auxiliary actions.
- Follow the WAI-ARIA tabs pattern: each `Tab` needs a stable `id` and matching `aria-controls`; the active content needs `role="tabpanel"`, a matching `id`, and `aria-labelledby`. Generate IDs from stable indices or `useId`, never user-provided labels.
- Blueprint responses can include JSON Schema contracts. Use the feature-local
  Ajv helper for immediate form feedback, but treat server-side `422` schema
  validation as authoritative.

## Icons

- Import icons assigned to Attricat concepts from `src/components/system-icons.ts` so the same concept is represented consistently across navigation, headings, menus, and other surfaces.
- Keep generic action and status icons, such as add, edit, delete, close, expand, and warnings, local to the component using them.
- Add a semantic export to the registry before introducing an icon for another system concept.

## Bundle size

- Run `pnpm --dir apps/catalog-web inspect:bundle` when changing dependencies
  or imports that affect the client bundle. It enforces a 275 KiB gzip budget
  for JavaScript synchronously loaded by `index.html`; keep new code behind
  route or component boundaries when it is not needed at application startup.

## Testing

- Run unit tests with `pnpm --dir apps/catalog-web test`.
- Run the Playwright suite with `pnpm --dir apps/catalog-web test:e2e`.
- See [the documentation index](index.md#test-the-web-app) for E2E setup and
  Docker/Colima behavior. Do not point it at or seed the development database.
