# Frontend Conventions

These conventions apply to `apps/catalog-web`. Follow the upstream
[Attricat design style guide](https://github.com/attricat/design/blob/main/STYLE.md)
and the [pinned token and asset snapshot](../apps/catalog-web/design/README.md)
for visual changes. The app theme adapts those tokens to its version of MUI;
do not duplicate raw colors or dimensions in feature components. Check both
light and dark modes.

## TypeScript

- Prefer arrow functions for TypeScript functions; route declarations may use a named function when it improves stack traces or route readability.
- Use semicolons and two-space indentation.
- Keep feature code under `src/features/<feature>`.
- Prefer relative imports within and between features; no path aliases are configured.
- Name non-component TypeScript files in camelCase (for example, `queryKeys.ts` and `latestRevisions.test.ts`); name React component files in PascalCase (for example, `EntityForm.tsx`). Keep file-based route filenames aligned with their URL segments, including kebab-case where appropriate, and retain tooling-required filenames such as `vite-env.d.ts`.

## Data Fetching

- Keep API request functions in each feature's `api.ts` and their Zod request/response schemas in that feature's `schemas.ts`.
- Route browser API calls through `src/api/request.ts`: use `request` for Zod-validated JSON, `requestNoContent` for successful empty responses, and `requestText` only for deliberately non-JSON or bounded extension responses. Do not call `fetch` or `apiFetch` from feature API clients.
- Preserve endpoint-specific semantics by handling `ApiRequestError` at the feature boundary (for example, an unauthenticated session may map HTTP 401 to `null`); all other API failures must retain the shared structured error.
- Define TanStack Query key factories in a feature-local `queryKeys.ts` file.
- Use those factories for every `queryKey` and invalidation so equivalent requests share the same cache entry and invalidation can reuse the same key definitions. Define a root key when a feature needs to invalidate all variants of a resource.

## Client State

- Use Zustand for client-only state shared across independent components or updated outside React (for example, toast notifications and Inspector timings). Keep stores feature-local unless the state is truly app-wide, and subscribe to the smallest slice needed.
- Keep server state in TanStack Query, navigable search/filter state in the router URL, fields submitted with an explicit Save button in TanStack Form, and component-scoped UI state in React. Do not move these into a global store just to avoid passing a prop or using a small context.
- Bound and sanitize data before writing it to a shared store; do not store sensitive API responses in diagnostic state.
- Keep unsaved values of editors with an explicit Save button across refreshes with `src/features/drafts/useEditorDraft` and its `DraftRestoreDialog`. Drafts live in tab-scoped session storage keyed by workspace, user, editor, and resource/version/context; they are only applied when the user chooses **Restore draft**, and are cleared after a confirmed save or discard. Never pass passwords, tokens, secrets, or file inputs to a draft. The entity page saves each field as it is committed and does not use drafts.

## Components

- Keep route files thin and compose feature page components from
  `src/features`.
- Keep one route-level page component per feature module. Extract independent
  pages and substantial page sections into descriptive sibling modules rather
  than growing a multi-route page file.
- Use TanStack Form for forms with an explicit Save button (for example, entity create and migration): submitted field values, validation, reset behavior, and submission handling. The entity page instead saves fields individually: `InlineFieldEditor` (`src/features/entities/components`) keeps a field's typing local and commits it on blur, Enter, or an immediate choice, and `useEntityFieldSaves` (`src/features/entities`) queues each committed field as its own save, keeps rejected changes to resend with the next one, and detects concurrent edits. Reserve React state for non-form UI state such as dialogs, notices, and upload progress. Use Material UI for interface components.
- Validate API payloads with Zod before using them in the UI.
- Do not scatter magic values through components or feature clients. Name domain values, repeated UI values, limits, storage keys, query parameters, and API literals as feature-local constants; keep a one-off literal inline only when its meaning is obvious at the use site. Promote cross-feature concepts to a shared, descriptive module rather than duplicating them.
- For imperative integrations that retain a callback (for example Monaco commands or browser event listeners), do not capture render-time values that can change. Re-register and dispose the callback when dependencies change, or read current values through refs.
- A disabled or readonly field must disable every mutation path, including inline chip removal, keyboard shortcuts, and auxiliary actions.
- Follow the WAI-ARIA tabs pattern: each `Tab` needs a stable `id` and matching `aria-controls`; the active content needs `role="tabpanel"`, a matching `id`, and `aria-labelledby`. Generate IDs from stable indices or `useId`, never user-provided labels.
- Blueprint responses can include JSON Schema contracts. Use the feature-local
  Ajv helper for immediate form feedback, but treat server-side `422` schema
  validation as authoritative.

## Adding Pages

- Add the route file under `src/routes` and keep it thin: it renders one page
  component from `src/features/<feature>`.
- Wrap the page in `PageContainer` (or `SettingsPage` for narrow settings
  layouts) and open it with `PageHeader` from `src/components/PageHeader.tsx`.
  Do not hand-roll a page title with `Typography`.
- Every page title is prefixed with the icon of the system concept it presents.
  Pass that icon to `PageHeader` as `icon`, imported from
  `src/components/systemIcons.ts`. TypeScript rejects a `title` without an
  `icon`. Use the same icon the navigation uses for that section, and keep
  child pages on their section's icon (for example, the blueprint list,
  detail, and editor pages all use `BlueprintIcon`). Pages that present a
  different concept use that concept's icon (for example, creating a personal
  API token uses `PersonalTokenIcon` inside Profile).
- If the page presents a concept with no registry export yet, add a semantic
  export to `systemIcons.ts` first (see [Icons](#icons)) rather than importing
  a Lucide icon directly.
- A heading rendered outside `PageHeader`, such as an entity heading built from
  a view definition, uses `PageTitle` from `src/components/PageTitle.tsx` so
  its icon, colour, and sizing match every other page.
- A header that shows only an `eyebrow` and no title does not take an icon.
- Add a management section to `src/components/navigation.ts` with the same
  registry icon so the side navigation and management dashboard card match
  the page header.

## Timestamps

- The API stores and returns instants in UTC. Render every instant through `src/time`: `<Timestamp value={iso} />` in JSX, or `useInstantFormat()` where only a string fits (for example a tooltip title or accessible label). Do not call `toLocaleString`, `toLocaleDateString`, `toLocaleTimeString`, or `Intl.DateTimeFormat` elsewhere; ESLint rejects them outside `src/time`.
- Instants are shown in the user's preferred zone (`time_zone` on the session, set on the Profile page) and fall back to the browser zone when it is unset. A short zone name is appended only when the preferred zone differs from the browser zone. `<Timestamp>` renders a semantic `<time>` element with the exact UTC value in a tooltip on hover and keyboard focus; pass `focusable={false}` inside links and buttons.
- To place a timestamp inside a translated sentence, use a `<timestamp/>` tag in the message and `<Trans components={{ timestamp: <Timestamp … /> }} />`.
- Pick a named style from `instantStyles` (`dateTime`, `dateTimeSeconds`, `date`, `time`). If you need another presentation, add a style there instead of passing ad-hoc options.
- Calendar dates (`date` attribute values) are not instants: format them with `formatCalendarDate`, which never shifts them into another zone. Zoned `time` values carry their own zone and are shown as stored.
- Interpret `datetime-local` input as wall-clock time in the effective zone with `zonedDateTimeToIso(value, useTimeZone())`, and fill such inputs with `isoToZonedDateTime`. Exports and API payloads keep UTC ISO timestamps.

## Icons

- Use `lucide-react` icons; `@mui/icons-material` is not a dependency. Import the `…Icon` export names (for example `PencilIcon`) so they do not collide with MUI components such as `Menu` or `List`.
- `AppProviders` sizes icons at `1em` of a `1.5rem` baseline, so they follow MUI component sizing (button icons, chips) like `SvgIcon`. For explicit sizes in dense controls use `compactIconSize` (18px) or `smallIconSize` (20px) from `src/components/iconSizes.ts`, and pass theme palette values to `color` rather than MUI color names.
- `src/app/theme.ts` routes MUI’s built-in component icons (Select and NativeSelect arrows, Alert severity icons, the default Chip delete icon, TableSortLabel) to Lucide, and gives `ListItemIcon` a 36px minimum width so a 24px icon keeps a 12px gap before its label. Checkbox and Radio keep their MUI glyphs.
- Import icons assigned to Attricat concepts from `src/components/systemIcons.ts` so the same concept is represented consistently across navigation, headings, menus, and other surfaces.
- Keep generic action and status icons, such as add, edit, delete, close, expand, and warnings, local to the component using them.
- Add a semantic export to the registry before introducing an icon for another system concept.

## Drag and drop

`ExplorerColumnPreferencesDialog.tsx` (Explorer column order) is the reference
implementation. Follow it for any new drag interaction.

- Use dnd-kit's React package: `DragDropProvider` from `@dnd-kit/react`,
  `useSortable` from `@dnd-kit/react/sortable` and `move` from
  `@dnd-kit/helpers`. Do not hand-roll HTML5 drag events or add another drag
  library. The `@dnd-kit/*` packages are pre-1.0 and pinned to one exact
  version; upgrade them together.
- Start drags from a dedicated handle: a small `IconButton` with
  `GripVerticalIcon`, wired to `handleRef`, with `touchAction: 'none'` and an
  accessible name that includes the item (for example "Drag to reorder
  Title"). The rest of the row, such as checkboxes and buttons, stays usable.
- Dragging must never be the only way to make a change. Keep an explicit
  alternative, such as move up/down buttons, so people who cannot drag can
  still reorder (WCAG 2.5.7).
- dnd-kit's built-in keyboard support works on the handle: Space or Enter picks
  up, the arrow keys move, Space or Enter drops, and Escape cancels. Its
  default screen reader instructions and announcements are English and name
  raw item IDs. Replace them by configuring the `Accessibility` plugin with
  translated text that uses item labels and positions.
- Update state once, in `onDragEnd`. Ignore cancelled drags and apply
  `move(items, event)`. Let dnd-kit's optimistic sorting move items during the
  drag, and do not persist on `dragover`.
- Constrain list drags to one axis (`RestrictToVerticalAxis` from
  `@dnd-kit/abstract/modifiers`). Lift the dragged item with theme tokens: a
  `boxShadow` plus a `divider` outline, because shadows barely show in dark
  mode.
- dnd-kit adds about 40 KiB gzip. Keep drag UIs out of the startup bundle by
  loading their component with `lazy()` when it sits on an initial route (see
  [Bundle size](#bundle-size)).
- jsdom has no layout, so unit tests cover labels, instructions and the
  non-drag controls (the test setup stubs `ResizeObserver`). Cover real
  pointer drags in Playwright, moving the mouse in steps so the pointer sensor
  activates.

## Bundle size

- Run `pnpm --dir apps/catalog-web inspect:bundle` when changing dependencies
  or imports that affect the client bundle. It enforces a 320 KiB gzip budget
  for JavaScript synchronously loaded by `index.html`; keep new code behind
  route or component boundaries when it is not needed at application startup.

## Testing

- Run unit tests with `pnpm --dir apps/catalog-web test`.
- Run the Playwright suite with `pnpm --dir apps/catalog-web test:e2e`.
- See [the documentation index](index.md#test-the-web-app) for E2E setup and
  Docker/Colima behavior. Do not point it at or seed the development database.
