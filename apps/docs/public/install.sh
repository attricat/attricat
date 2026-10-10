#!/bin/sh
# Runs Attricat on this machine for evaluation with Docker Compose:
#
#   curl -fsSL https://docs.attricat.com/install.sh | sh
#
# Creates ./attricat with a Compose file and a .env holding a generated owner
# password, then starts the published image with PostgreSQL, RustFS and
# Mailpit. Running it again keeps .env, refreshes compose.yml and pulls the
# newest image. For production, see https://docs.attricat.com/operate/deployment/.
#
# Settings, read from the environment on first run:
#   ATTRICAT_DIR             directory to create (default: ./attricat)
#   ATTRICAT_IMAGE           image to run (default: ghcr.io/attricat/attricat:latest)
#   ATTRICAT_PORT            host port for the web app (default: 3000)
#   MAILPIT_UI_PORT          host port for Mailpit (default: 8025)
#   ATTRICAT_OWNER_EMAIL     first owner's email (default: owner@example.com)
#   ATTRICAT_OWNER_PASSWORD  first owner's password (default: generated)

set -eu

# Everything runs from main, so a partially downloaded script does nothing.
main() {
	dir=${ATTRICAT_DIR:-attricat}

	command -v docker >/dev/null 2>&1 || fail "Docker is not installed. See https://docs.docker.com/get-docker/"
	docker compose version >/dev/null 2>&1 || fail "Docker Compose v2 is not available. See https://docs.docker.com/compose/install/"
	docker info >/dev/null 2>&1 || fail "Cannot reach the Docker daemon. Start Docker, or check that you may use it."

	mkdir -p "$dir"
	cd "$dir"

	if [ -f .env ]; then
		say "Keeping existing $(pwd)/.env"
	else
		write_env
		say "Wrote $(pwd)/.env"
	fi
	write_compose
	say "Wrote $(pwd)/compose.yml"

	say "Pulling images"
	docker compose pull --quiet
	say "Starting Attricat"
	docker compose up -d --wait

	# shellcheck disable=SC1091
	. ./.env
	cat <<EOF

Attricat is running.

  Open:       http://localhost:${ATTRICAT_PORT}/login
  Workspace:  default.local
  Email:      ${ATTRICAT_OWNER_EMAIL}
  Password:   ${ATTRICAT_OWNER_PASSWORD}
  Mail:       http://localhost:${MAILPIT_UI_PORT}

The password is stored in $(pwd)/.env.
From $(pwd):
  docker compose pull && docker compose up -d   update to the newest image
  docker compose down                           stop
  docker compose down -v                        stop and delete all data

This setup is for local evaluation only: plain HTTP and fixed internal
credentials. To deploy for real, see https://docs.attricat.com/operate/deployment/
EOF
}

say() {
	printf '==> %s\n' "$1"
}

fail() {
	printf 'error: %s\n' "$1" >&2
	exit 1
}

generate_password() {
	LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 24
}

write_env() {
	password=${ATTRICAT_OWNER_PASSWORD:-$(generate_password)}
	[ -n "$password" ] || fail "Could not generate a password. Set ATTRICAT_OWNER_PASSWORD."
	umask 077
	cat >.env <<EOF
ATTRICAT_IMAGE=${ATTRICAT_IMAGE:-ghcr.io/attricat/attricat:latest}
ATTRICAT_PORT=${ATTRICAT_PORT:-3000}
MAILPIT_UI_PORT=${MAILPIT_UI_PORT:-8025}
ATTRICAT_OWNER_EMAIL=${ATTRICAT_OWNER_EMAIL:-owner@example.com}
ATTRICAT_OWNER_PASSWORD=${password}
EOF
	umask 022
}

# Keep in step with deploy/compose.quickstart.yml and apps/api/compose.yml.
write_compose() {
	cat >compose.yml <<'EOF'
# Written by https://docs.attricat.com/install.sh; rerunning it replaces this
# file. Settings live in .env next to it.
name: attricat

x-app: &app
  image: ${ATTRICAT_IMAGE:-ghcr.io/attricat/attricat:latest}
  # The published image is amd64-only; arm64 hosts run it under emulation.
  platform: linux/amd64
  read_only: true
  tmpfs:
    - /tmp:size=256m,mode=1777
  security_opt:
    - no-new-privileges:true
  restart: unless-stopped

x-environment: &environment
  DATABASE_URL: postgres://postgres:postgres@postgres:5432/attricat
  ATTRICAT_AUTO_MIGRATE: "false"
  ATTRICAT_BOOTSTRAP_OWNER_EMAIL: ${ATTRICAT_OWNER_EMAIL:-owner@example.com}
  ATTRICAT_BOOTSTRAP_OWNER_PASSWORD: ${ATTRICAT_OWNER_PASSWORD:?Set ATTRICAT_OWNER_PASSWORD in .env}
  SESSION_COOKIE_SECURE: "false"
  ATTRICAT_DEVTOOLS: "false"
  S3_ENDPOINT: http://rustfs:9000
  S3_REGION: us-east-1
  S3_BUCKET: attricat-files
  S3_ACCESS_KEY_ID: attricat-dev
  S3_SECRET_ACCESS_KEY: attricat-dev-secret
  S3_FORCE_PATH_STYLE: "true"
  S3_UPLOAD_TIMEOUT_SECONDS: "30"
  S3_DOWNLOAD_TIMEOUT_SECONDS: "30"
  SMTP_HOST: mailpit
  SMTP_PORT: "1025"
  SMTP_TLS_MODE: disabled
  MAIL_FROM: Attricat <no-reply@example.com>
  PASSWORD_RESET_URL: http://localhost:${ATTRICAT_PORT:-3000}/password-reset/confirm
  WORKSPACE_INVITATION_URL: http://localhost:${ATTRICAT_PORT:-3000}/invitations/accept
  WORKSPACE_ONBOARDING_URL: http://localhost:${ATTRICAT_PORT:-3000}/onboarding

services:
  postgres:
    image: postgres:18-alpine
    environment:
      POSTGRES_DB: attricat
      POSTGRES_PASSWORD: postgres
      POSTGRES_USER: postgres
    volumes:
      - postgres-data:/var/lib/postgresql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready --username=$$POSTGRES_USER --dbname=$$POSTGRES_DB"]
      interval: 5s
      timeout: 5s
      retries: 10
      start_period: 5s
    restart: unless-stopped

  rustfs:
    image: rustfs/rustfs:1.0.0-beta.12
    command: "/data"
    environment:
      RUSTFS_ACCESS_KEY: attricat-dev
      RUSTFS_SECRET_KEY: attricat-dev-secret
    volumes:
      - rustfs-data:/data
    healthcheck:
      test: ["CMD-SHELL", "curl --fail --silent http://127.0.0.1:9000/health/live >/dev/null"]
      interval: 3s
      timeout: 3s
      retries: 20
      start_period: 5s
    restart: unless-stopped

  rustfs-init:
    image: amazon/aws-cli:2.31.0
    depends_on:
      rustfs:
        condition: service_healthy
    environment:
      AWS_ACCESS_KEY_ID: attricat-dev
      AWS_SECRET_ACCESS_KEY: attricat-dev-secret
      AWS_DEFAULT_REGION: us-east-1
      S3_BUCKET: attricat-files
    entrypoint:
      - /bin/sh
      - -ec
      - >-
        if ! aws --endpoint-url http://rustfs:9000 s3api head-bucket --bucket "$$S3_BUCKET" >/dev/null 2>&1;
        then aws --endpoint-url http://rustfs:9000 s3api create-bucket --bucket "$$S3_BUCKET"; fi
    restart: "no"

  mailpit:
    image: axllent/mailpit:v1.28
    ports:
      - "127.0.0.1:${MAILPIT_UI_PORT:-8025}:8025"
    restart: unless-stopped

  migrate:
    <<: *app
    command: ["migrate"]
    restart: "no"
    environment: *environment
    depends_on:
      postgres:
        condition: service_healthy
      rustfs-init:
        condition: service_completed_successfully

  api:
    <<: *app
    command: ["api"]
    environment: *environment
    depends_on:
      migrate:
        condition: service_completed_successfully
    ports:
      - "127.0.0.1:${ATTRICAT_PORT:-3000}:3000"
    healthcheck:
      test: ["CMD", "curl", "--fail", "--silent", "http://127.0.0.1:3000/health/ready"]
      interval: 5s
      timeout: 3s
      retries: 30

  file-worker:
    <<: *app
    command: ["file-worker"]
    environment: *environment
    depends_on:
      migrate:
        condition: service_completed_successfully
    healthcheck:
      test: ["CMD", "curl", "--fail", "--silent", "http://127.0.0.1:3001/health/ready"]
      interval: 5s
      timeout: 3s
      retries: 30

volumes:
  postgres-data:
  rustfs-data:
EOF
}

main "$@"
