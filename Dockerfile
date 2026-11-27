# syntax=docker/dockerfile:1.7
FROM node:26-trixie-slim AS web-builder
ENV NODE_OPTIONS=--max-old-space-size=1536
RUN npm install --global pnpm@11.25.0
WORKDIR /src
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json apps/web/package.json
COPY apps/docs/package.json apps/docs/package.json
RUN --mount=type=cache,id=attricat-pnpm-store,target=/pnpm-store \
    pnpm install --store-dir /pnpm-store --filter web --frozen-lockfile
COPY contracts contracts
# The web type check covers the UI kit contract test, which imports its generator.
COPY scripts/ui-kit scripts/ui-kit
COPY apps/web apps/web
RUN pnpm --dir apps/web build

FROM rust:trixie AS rust-builder
# Wasmtime/Cranelift is memory-intensive under full dependency optimization.
# Level 1 still produces an optimized runtime while keeping release builds viable
# on ordinary CI runners; one cargo job prevents parallel compiler spikes.
# Local snapshot builds may raise it with ATTRICAT_SNAPSHOT_JOBS.
ARG CARGO_BUILD_JOBS=1
ENV CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS CARGO_PROFILE_RELEASE_OPT_LEVEL=1
# This copy intentionally sequences the memory-heavy frontend and Rust builds.
COPY --from=web-builder /src/apps/web/dist /tmp/web-dist
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
# BuildKit cache mounts keep the registry and compiled dependencies between
# builds on the same host, so repeat local builds reuse compiled
# dependencies. A fresh CI runner starts with empty caches. The target directory is a
# cache mount, so the binaries are copied out before the step ends.
# Cargo judges workspace crates fresh by file times, and `git archive` dates
# every file to its commit, so an older commit built after a newer one would
# reuse the newer build. Dating the sources now rebuilds the workspace crates
# every time; compiled dependencies stay cached.
RUN --mount=type=cache,id=attricat-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=attricat-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=attricat-cargo-target,target=/src/target \
    find apps crates contracts docs Cargo.toml Cargo.lock -type f -exec touch {} + \
 && cargo build --locked --release -p api --bins -p acli --bin acli \
 && mkdir -p /out \
 && cp target/release/api target/release/file-worker target/release/migrate target/release/acli /out/

FROM debian:trixie-slim AS runtime
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
COPY --from=rust-builder /out/api /usr/local/bin/attricat-api
COPY --from=rust-builder /out/file-worker /usr/local/bin/attricat-file-worker
COPY --from=rust-builder /out/migrate /usr/local/bin/attricat-migrate
COPY --from=rust-builder /out/acli /usr/local/bin/acli
COPY --from=rust-builder /tmp/web-dist /srv/attricat/web
COPY scripts/docker-entrypoint.sh /usr/local/bin/
RUN chmod 0555 /usr/local/bin/docker-entrypoint.sh \
 && mkdir -p /var/tmp/attricat \
 && chown -R attricat:attricat /srv/attricat /var/tmp/attricat
ENV BIND_ADDR=0.0.0.0:3000 \
    WEB_DIST_DIR=/srv/attricat/web \
    ATTRICAT_AUTO_MIGRATE=false
EXPOSE 3000 3001
USER 10001:10001
ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["api"]
