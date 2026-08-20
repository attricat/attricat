setup:
    node scripts/setup-worktree.mjs

dev: setup
    set -a; . ./.env; set +a; docker compose --env-file .env --project-name catalog-$POSTGRES_PORT -f apps/api/compose.yml up -d
    process-compose --no-server --env .env up

down: setup
    set -a; . ./.env; set +a; docker compose --env-file .env --project-name catalog-$POSTGRES_PORT -f apps/api/compose.yml down
