set dotenv-load := true

setup:
    node scripts/setup-worktree.mjs
    pnpm install --frozen-lockfile

_assert-env:
    test -f .env || { echo "Missing .env; run 'just setup' first." >&2; exit 1; }

dev: _assert-env
    docker compose --env-file .env --project-name catalog-$POSTGRES_PORT -f apps/api/compose.yml up -d
    docker compose --env-file .env --project-name catalog-$POSTGRES_PORT -f apps/api/compose.yml wait rustfs-init
    trap 'just down' EXIT; process-compose --no-server --env .env up

down: _assert-env
    docker compose --env-file .env --project-name catalog-$POSTGRES_PORT -f apps/api/compose.yml down

migrate: _assert-env
    sqlx migrate run --source apps/api/migrations --database-url "$DATABASE_URL"

sql: _assert-env
    docker compose --env-file .env --project-name catalog-$POSTGRES_PORT -f apps/api/compose.yml exec postgres psql --username=postgres --dbname=catalog

test-s3-compat: _assert-env
    cargo test -p api --test s3_compat -- --ignored

reset-db: _assert-env
    sqlx database drop -y --database-url "$DATABASE_URL"
    sqlx database create --database-url "$DATABASE_URL"
    sqlx migrate run --source apps/api/migrations --database-url "$DATABASE_URL"

generate size="small" industry="pc-components": _assert-env
    CATALOG_SERVER="$CATALOG_API_URL" node examples/generate.mjs --industry "{{industry}}" --size "{{size}}"

generate-resume size="small" industry="pc-components": _assert-env
    CATALOG_SERVER="$CATALOG_API_URL" node examples/generate.mjs --industry "{{industry}}" --size "{{size}}" --resume

test-generator:
    node --test examples/generator/industries/*.test.mjs

perf profile="smoke": _assert-env
    test -n "$CATALOG_TOKEN" || { echo "Set CATALOG_TOKEN before running performance tests." >&2; exit 1; }
    command -v k6 >/dev/null || { echo "Install k6 before running performance tests." >&2; exit 1; }
    CATALOG_SERVER="$CATALOG_API_URL" PERF_PROFILE="{{profile}}" k6 run perf/explorer.js
