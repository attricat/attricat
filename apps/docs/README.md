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

## Content

Public English guides live in `src/content/docs`; their Polish equivalents use the matching path below `src/content/docs/pl`. Add or change both versions together so the language switcher never directs readers to stale content. Keep guides task-oriented and safe for public publication. Internal engineering, operational, and planning documentation remains under the repository-level `docs/` directory unless it has been deliberately adapted for this site.
