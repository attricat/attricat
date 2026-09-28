# Attricat Docs

The public Attricat documentation site is an [Astro Starlight](https://starlight.astro.build/) application.

## Run locally

From the repository root:

```sh
pnpm --dir apps/docs dev
```

Build the production site with:

```sh
pnpm --dir apps/docs build
```

## Visual style

`src/styles/brand.css` maps the semantic colors, type, and radii from the pinned [design snapshot](../catalog-web/design/README.md) into Starlight's light and dark themes. The wordmarks in `src/assets/` are copies of that snapshot's `assets/logos/wordmark-{light,dark}.svg`. When updating the snapshot, refresh the CSS tokens and both wordmarks together, then check both themes and locales.

## Content

Public English guides live in `src/content/docs`; their Polish equivalents use the matching path below `src/content/docs/pl`. Add or change both versions together so the language switcher never directs readers to stale content. Keep guides task-oriented and safe for public publication. Internal engineering, operational, and planning documentation remains under the repository-level `docs/` directory unless it has been deliberately adapted for this site.
