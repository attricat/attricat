set dotenv-load := true

setup:
    node scripts/setup-worktree.mjs
    pnpm install --frozen-lockfile

_assert-env:
    test -f .env || { echo "Missing .env; run 'just setup' first." >&2; exit 1; }

dev: _assert-env
    docker compose --env-file .env --project-name attricat-$POSTGRES_PORT -f apps/api/compose.yml up -d
    docker compose --env-file .env --project-name attricat-$POSTGRES_PORT -f apps/api/compose.yml wait rustfs-init
    trap 'just down' EXIT; process-compose --no-server --env .env up

down: _assert-env
    docker compose --env-file .env --project-name attricat-$POSTGRES_PORT -f apps/api/compose.yml down

migrate: _assert-env
    sqlx migrate run --source apps/api/migrations --database-url "$DATABASE_URL"

fmt-check:
    cargo fmt --all -- --check

check:
    cargo check --locked --workspace --all-targets

benchmark-api-compile mode="all":
    scripts/benchmark-api-compile.sh {{mode}}

clippy:
    cargo clippy --locked --workspace --all-targets -- -D warnings

test-rust:
    cargo test --locked --workspace

# Regenerate the definition JSON Schemas derived from the Rust parser types.
contracts:
    UPDATE_CONTRACTS=1 cargo test --locked -p attricat-validation status_contract_is_current
    UPDATE_CONTRACTS=1 cargo test --locked -p attricat-validation principal_contract_is_current
    UPDATE_CONTRACTS=1 cargo test --locked -p attricat-blueprint --test definition_schema definition_schema_contract_is_current
    UPDATE_CONTRACTS=1 cargo test --locked -p attricat-repository --lib definition_schema_contract_is_current
    UPDATE_CONTRACTS=1 cargo test --locked -p attricat-lexicon --test conformance lexicon_schema_contract_is_current

deny:
    cargo deny check

ci: fmt-check check clippy test-rust deny

# Bump the workspace version, then commit, tag and push it to start a release.
release bump:
    scripts/release.sh {{bump}}

# Build the production application image used by the local release stack.
production-build image="attricat:local":
    docker build --build-arg VCS_REF="$(git rev-parse HEAD)" --build-arg VCS_BRANCH="${GITHUB_HEAD_REF:-${GITHUB_REF_NAME:-$(git rev-parse --abbrev-ref HEAD)}}" --build-arg VERSION="$(git describe --always --dirty)" -t "{{image}}" .

# Start the production image with disposable PostgreSQL, RustFS, and Mailpit services.
production-up image="attricat:local":
    ATTRICAT_IMAGE="{{image}}" docker compose -f deploy/compose.ci.yml up -d

# Build the production image and start the local release stack.
production-start image="attricat:local":
    just production-build "{{image}}"
    just production-up "{{image}}"

# Follow logs from the local production stack.
production-logs:
    docker compose -f deploy/compose.ci.yml logs -f

# Remove the local production stack and all of its disposable data.
production-down:
    docker compose -f deploy/compose.ci.yml down --volumes --remove-orphans

# Exercise an already-built image through the complete release-path verification.
production-verify image="attricat:local":
    ATTRICAT_IMAGE="{{image}}" SKIP_IMAGE_BUILD=true scripts/verify-deployment.sh

# Build and exercise the local production image.
production-test image="attricat:local":
    just production-build "{{image}}"
    just production-verify "{{image}}"

sql: _assert-env
    docker compose --env-file .env --project-name attricat-$POSTGRES_PORT -f apps/api/compose.yml exec postgres psql --username=postgres --dbname=attricat

test-s3-compat: _assert-env
    cargo test -p api --test s3_compat -- --ignored

reset-db: _assert-env
    #!/usr/bin/env bash
    set -euo pipefail
    worktree="$(pwd -P)"
    pids=()
    while IFS= read -r pid; do
        cwd="$(lsof -a -p "$pid" -d cwd -Fn 2>/dev/null | awk '/^n/ { print substr($0, 2); exit }')"
        if [[ "$cwd" == "$worktree" ]]; then
            pids+=("$pid")
        fi
    done < <(pgrep -f 'target/debug/(api|file-worker)( |$)' || true)

    restart_services() {
        if ((${#pids[@]})); then
            touch apps/api/src/lib.rs
            echo "Restarting API and file worker through watchexec."
        fi
    }
    trap restart_services EXIT

    if ((${#pids[@]})); then
        echo "Stopping API and file-worker processes: ${pids[*]}"
        kill "${pids[@]}"
        for _ in {1..100}; do
            running=false
            for pid in "${pids[@]}"; do
                if kill -0 "$pid" 2>/dev/null; then
                    running=true
                    break
                fi
            done
            $running || break
            sleep 0.1
        done
        for pid in "${pids[@]}"; do
            if kill -0 "$pid" 2>/dev/null; then
                echo "Timed out waiting for process $pid to stop." >&2
                exit 1
            fi
        done
    fi

    sqlx database reset -y --force --source ./apps/api/migrations --database-url "$DATABASE_URL"

generate size="small" industry="pc-components": _assert-env
    ATTRICAT_SERVER="$ATTRICAT_API_URL" node examples/generate.mjs --industry "{{industry}}" --size "{{size}}" --concurrency 8

generate-resume size="small" industry="pc-components": _assert-env
    ATTRICAT_SERVER="$ATTRICAT_API_URL" node examples/generate.mjs --industry "{{industry}}" --size "{{size}}" --resume

test-generator:
    node --test examples/generator/industries/*.test.mjs

perf profile="smoke": _assert-env
    test -n "$ATTRICAT_TOKEN" || { echo "Set ATTRICAT_TOKEN before running performance tests." >&2; exit 1; }
    command -v k6 >/dev/null || { echo "Install k6 before running performance tests." >&2; exit 1; }
    ATTRICAT_SERVER="$ATTRICAT_API_URL" PERF_PROFILE="{{profile}}" k6 run perf/explorer.js

test-reference-extension-e2e: _assert-env
	@source .worktree; ATTRICAT_API_URL="http://127.0.0.1:$API_PORT/api" scripts/test-reference-extension-e2e.sh
