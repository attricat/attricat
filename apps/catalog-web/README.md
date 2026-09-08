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
