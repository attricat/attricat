# Attricat Web

See the repository's [frontend conventions](../../docs/frontend.md) for code
structure, TypeScript, data-fetching, and testing expectations.

For local setup, end-to-end test instructions, and Docker/Colima behavior, see
the repository [documentation index](../../docs/index.md).

## Component tests

Unit tests run in Node by default. Browser-facing component tests use Vitest's
`// @vitest-environment jsdom` directive and React Testing Library; shared test
setup in `src/test/setup.ts` resets rendered DOM after each test. Prefer
role- and label-based queries so tests also protect accessible interaction.

## Frontend-only browser tests

Run `pnpm --dir apps/web test:frontend` to build the production app and
run Playwright smoke tests against an isolated preview server with mocked API
responses. No backend process or database is used. These tests cover local Monaco
runtime/worker loading with external requests blocked and on-demand locale chunks.
Install Chromium first with `pnpm --dir apps/web exec playwright install chromium`.
The normal `test:e2e` suite remains the integration check against a real API.

## Production bundle inspection

Run `pnpm --dir apps/web inspect:bundle` to make a fresh production
build and report the JavaScript loaded by `index.html` (including synchronous
imports), separately from all lazy chunks. The report reads Vite's manifest, so
it does not use a previous `dist` directory.

Route chunks and locale dictionaries load on demand. Monaco and its worker are
built from the installed package and served locally, only on editor routes;
`src/components/monacoRuntime.ts` owns their setup rather than relying on a CDN.

Re-run the command when dependencies or routes
change; inspect its current size report rather than relying on historical build
numbers or asset hashes.
