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

`src/styles/brand.css` maps the semantic colors, type, radii and elevation from the [Attricat design repository](https://github.com/attricat/design) (revision noted at the top of the file) into Starlight's light and dark themes. The wordmarks in `src/assets/` and `public/favicon.svg` are copies of that revision's `assets/logos/`. When the design changes, refresh the CSS tokens, both wordmarks and the favicon together, then check both themes and locales. Polish content follows the design repository's terminology table (for example, Blueprint → Schemat).

## Content

Public English guides live in `src/content/docs`; their Polish equivalents use the matching path below `src/content/docs/pl`. Add or change both versions together so the language switcher never directs readers to stale content. Keep guides task-oriented and safe for public publication. Internal engineering, operational, and planning documentation remains under the repository-level `docs/` directory unless it has been deliberately adapted for this site.
