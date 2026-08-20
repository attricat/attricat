setup:
    node scripts/setup-worktree.mjs

dev: setup
    set -a; . ./.env; set +a; docker compose --env-file .env --project-name catalog-$POSTGRES_PORT -f apps/api/compose.yml up -d
    process-compose --no-server --env .env up

down: setup
    set -a; . ./.env; set +a; docker compose --env-file .env --project-name catalog-$POSTGRES_PORT -f apps/api/compose.yml down

migrate: setup
    set -a; . ./.env; set +a; sqlx migrate run --source apps/api/migrations --database-url "$DATABASE_URL"

reset-db: setup
    set -a; . ./.env; set +a; sqlx database drop -y --database-url "$DATABASE_URL"
    set -a; . ./.env; set +a; sqlx database create --database-url "$DATABASE_URL"
    set -a; . ./.env; set +a; sqlx migrate run --source apps/api/migrations --database-url "$DATABASE_URL"

generate: setup
    set -a; . ./.env; set +a; CATALOG_SERVER="$CATALOG_API_URL" node examples/generate.mjs
