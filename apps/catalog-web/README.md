# Catalog Web

See the repository's [frontend conventions](../../docs/frontend.md) for code
structure, TypeScript, data-fetching, and testing expectations.

For local setup, end-to-end test instructions, and Docker/Colima behavior, see
the repository [documentation index](../../docs/index.md).

## Component tests

Unit tests run in Node by default. Browser-facing component tests use Vitest's
`// @vitest-environment jsdom` directive and React Testing Library; shared test
setup in `src/test/setup.ts` resets rendered DOM after each test. Prefer
role- and label-based queries so tests also protect accessible interaction.

## Production bundle inspection

Run `pnpm --dir apps/catalog-web inspect:bundle` to make a fresh production
build and report the JavaScript loaded by `index.html` (including synchronous
imports), separately from all lazy chunks. The report reads Vite's manifest, so
it does not use a previous `dist` directory.

Enabling TanStack Router automatic route code splitting reduced the initial
JavaScript from **1,534.4 KiB (460.4 KiB gzip)** in the baseline build to
**818.3 KiB (260.0 KiB gzip)**. The route chunks remain available on demand.
Re-run the command when dependencies or routes change rather than comparing
asset hashes.
