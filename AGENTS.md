# Frontend

Follow [Frontend Conventions](docs/frontend.md) for `apps/catalog-web`.

# Development

The development server is already running via Process Compose when developing the app.

## Worktree development server

Run `just dev` from the repository root to start the worktree-local stack. It
writes the assigned `POSTGRES_PORT`, `API_PORT`, and `WEB_PORT` to the ignored
`.catalog-worktree` file and prints the web URL. Source that file whenever a
later verification step needs the running services:

```sh
source .catalog-worktree
curl http://127.0.0.1:$API_PORT/health
open http://127.0.0.1:$WEB_PORT
```

Do not assume the default Vite or API ports; parallel worktrees receive unique
ports. Use `just down` to stop the worktree database.

Use SQLx for all migration operations.

## Database policy

SQL migrations are declarative only. They may define tables, columns, indexes,
foreign keys, `NOT NULL`, `UNIQUE`, and `CHECK` constraints, plus required types
and extensions. Never add SQL functions, procedures, triggers, views, RLS
policies, or `DO` blocks. Business authorization, validation, state transitions,
audit behavior, rate limiting, and retention live in Rust repository/application
code and use explicit transactions.
