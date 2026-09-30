# syntax=docker/dockerfile:1.7
FROM node:22-bookworm-slim AS web-builder
ENV NODE_OPTIONS=--max-old-space-size=1536
RUN npm install --global pnpm@11.25.0
WORKDIR /src
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/catalog-web/package.json apps/catalog-web/package.json
COPY apps/docs/package.json apps/docs/package.json
RUN pnpm install --filter catalog-web --frozen-lockfile
COPY contracts contracts
COPY apps/catalog-web apps/catalog-web
RUN pnpm --dir apps/catalog-web build

FROM rust:bookworm AS rust-builder
# Wasmtime/Cranelift is memory-intensive under full dependency optimization.
# Level 1 still produces an optimized runtime while keeping release builds viable
# on ordinary CI runners; one cargo job prevents parallel compiler spikes.
ENV CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_OPT_LEVEL=1
# This copy intentionally sequences the memory-heavy frontend and Rust builds.
COPY --from=web-builder /src/apps/catalog-web/dist /tmp/web-dist
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps apps
COPY crates crates
COPY contracts contracts
COPY docs docs
# The build context excludes `.git`, so the API build script reads its source
# branch and commit from these arguments instead.
ARG VCS_REF=unknown
ARG VCS_BRANCH=unknown
ENV ATTRICAT_BUILD_COMMIT=$VCS_REF ATTRICAT_BUILD_BRANCH=$VCS_BRANCH
RUN cargo build --locked --release -p api --bins -p cli --bin acli

FROM debian:bookworm-slim AS runtime
ARG VCS_REF=unknown
ARG VERSION=dev
LABEL org.opencontainers.image.source="https://github.com/attricat/attricat" \
      org.opencontainers.image.revision="$VCS_REF" \
      org.opencontainers.image.version="$VERSION"
# PostgreSQL, object-storage, and backup utilities are deliberately external.
# The runtime includes only certificates and curl for HTTPS and health checks.
RUN apt-get update \
 && apt-get install --yes --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd --system --gid 10001 attricat \
 && useradd --system --uid 10001 --gid attricat --home /srv/attricat attricat
WORKDIR /srv/attricat
COPY --from=rust-builder /src/target/release/api /usr/local/bin/attricat-api
COPY --from=rust-builder /src/target/release/file-worker /usr/local/bin/attricat-file-worker
COPY --from=rust-builder /src/target/release/migrate /usr/local/bin/attricat-migrate
COPY --from=rust-builder /src/target/release/acli /usr/local/bin/acli
COPY --from=rust-builder /tmp/web-dist /srv/attricat/web
COPY scripts/docker-entrypoint.sh /usr/local/bin/
RUN chmod 0555 /usr/local/bin/docker-entrypoint.sh \
 && chown -R attricat:attricat /srv/attricat
ENV BIND_ADDR=0.0.0.0:3000 \
    WEB_DIST_DIR=/srv/attricat/web \
    CATALOG_AUTO_MIGRATE=false
EXPOSE 3000 3001
USER 10001:10001
ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["api"]
