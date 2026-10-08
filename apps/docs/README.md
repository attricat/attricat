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

The site is organized into sections, each a directory under `src/content/docs`:

| Directory | Contents |
| --- | --- |
| `start/` (plus `introduction.md`) | Concepts and a quickstart. |
| `guides/` | Day-to-day use: Explorer, search syntax, records, contexts, publishing, agents. |
| `builders/` | Catalog design: modeling, blueprints, views, validation, revisions, rules, workflows, solution packs, extension management. |
| `extensions/` | Building extensions: packaging, manifest, server runtime, client contributions, operations. |
| `operate/` | Workspace administration, deployment, monitoring, backup. |
| `reference/` | Configuration, blueprint TOML, permissions, CLI, API, events. |

`apps/catalog-web/src/app/documentation.ts` links to `builders/blueprints/`, `guides/contexts/`, `builders/extensions/`, and `operate/workspaces/`. Keep those slugs, or update that file when moving them. Reference pages describe what the code accepts; when a blueprint key, environment variable, permission, or CLI command changes, update the matching reference page in the same change.

Public English guides live in `src/content/docs`; their Polish equivalents use the matching path below `src/content/docs/pl`. Add or change both versions together so the language switcher never directs readers to stale content. Keep guides task-oriented and safe for public publication. Internal engineering, operational, and planning documentation remains under the repository-level `docs/` directory unless it has been deliberately adapted for this site.
