# Configuration Reference

The API loads `.env` from the working directory at startup. `just setup` and
`just dev` create it from `.env.example` with persistent, worktree-specific
ports. Those port assignments are recorded in the ignored `.catalog-worktree`
file.

| Setting | Default | Used by | Purpose |
| --- | --- | --- | --- |
| `DATABASE_URL` | Required | API and SQLx | PostgreSQL connection string. |
| `BIND_ADDR` | `127.0.0.1:3000` | API | Listener address. |
| `RUST_LOG` | `info` | API | Structured tracing filter (for example, `api=debug`). |
| `PREVIEW_MAX_RELATIONSHIP_DEPTH` | `3` | API | Maximum recursive relationship preview depth. |
| `PREVIEW_MAX_RELATIONSHIP_ITEMS` | `10` | API | Maximum inline targets per relationship. |
| `ENTITY_MAX_PAGE_SIZE` | `100` | API | Maximum page size for relationship browsing. |
| `DATA_HEALTH_CACHE_TTL_SECONDS` | `300` | API | Data-health response cache lifetime. |
| `CATALOG_API_URL` | `http://127.0.0.1:3000` | Vite | API target for the web app's `/api` development proxy. |
| `POSTGRES_DB` | `catalog` | Docker Compose | Local PostgreSQL database name. |
| `POSTGRES_USER` | `postgres` | Docker Compose | Local PostgreSQL user. |
| `POSTGRES_PASSWORD` | `postgres` | Docker Compose | Local PostgreSQL password. |
| `POSTGRES_PORT` | `5432` | Docker Compose | Host port mapped to PostgreSQL. |
| `WEB_PORT` | `5173` | Vite | Listener port for the development web app. |

`CATALOG_SERVER` overrides the CLI's API URL. The CLI otherwise targets
`http://127.0.0.1:3000`.
