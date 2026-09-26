# Attricat design snapshot

These files are copied verbatim from the canonical [Attricat design repository](https://github.com/attricat/design) at commit `69392ef0a305a26a56c2a9787c7969208656b96d`:

- `tokens/{colors,typography,radius,elevation}.json`
- `assets/logos/{mark,mark-dark,wordmark-light,wordmark-dark}.svg`

The upstream `STYLE.md` and `mui/` directory govern how these tokens are used. `src/app/theme.ts` adapts the upstream MUI theme to the app's MUI version; it does not define a second palette. To update, review upstream changes to the style guide and MUI theme, copy the token/asset files from a pinned commit, update the adapter and commit reference, and check both light and dark modes. This in-repo snapshot keeps CI and release builds self-contained without requiring access to the separate design repository.
