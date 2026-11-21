# Podman / Buildah container definition.
# Build: podman build -f Containerfile -t fileuploader:local .
# Keep this file in sync with Dockerfile.
#
# syntax=docker/dockerfile:1

# ---------------------------------------------------------------------------
# Frontend build
# ---------------------------------------------------------------------------
FROM node:22-bookworm-slim AS frontend
WORKDIR /build/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ---------------------------------------------------------------------------
# Rust release build
# ---------------------------------------------------------------------------
FROM rust:1.100-bookworm AS backend
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake perl pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
ENV CARGO_NET_RETRY=5 \
    CARGO_HTTP_TIMEOUT=60
COPY Cargo.toml Cargo.lock ./
# Build dependency crates in a cached layer. Source changes then only rebuild
# this crate instead of recompiling the entire dependency graph. The dummy
# sources use none of the dependencies, which would make Cargo 1.100 emit an
# unused-dependency warning for every crate; allow that lint only for this
# warm-up build and restore the real manifest afterwards.
RUN mkdir -p src \
    && cp Cargo.toml Cargo.toml.warmup \
    && printf '\n[lints.cargo]\nunused_dependencies = "allow"\n' >> Cargo.toml \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf 'pub fn placeholder() {}\n' > src/lib.rs \
    && cargo build --release --locked \
    && mv Cargo.toml.warmup Cargo.toml \
    && rm -rf src
COPY src ./src
# BuildKit keeps the build context's mtimes, which are older than the dummy
# artifacts created above. Cargo then considers the crate fresh and leaves the
# placeholder binary in place, producing an image that exits immediately.
# Drop this crate's artifacts (dependencies stay cached), touch the sources, and
# assert that the real binary was linked.
RUN cargo clean -p fileuploader --release \
    && find src -name '*.rs' -exec touch {} + \
    && cargo build --release --locked \
    && grep -aq "FileUploader is ready" target/release/fileuploader

# ---------------------------------------------------------------------------
# Runtime: Debian 12 + official MEGAcmd package repository
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        fuse \
        gnupg \
        libfuse2 \
        procps \
        tini \
        tzdata \
    && mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://mega.nz/keys/MEGA_signing.key \
        | gpg --dearmor -o /etc/apt/keyrings/meganz-archive-keyring.gpg \
    && printf '%s\n' \
        'deb [signed-by=/etc/apt/keyrings/meganz-archive-keyring.gpg] https://mega.nz/linux/repo/Debian_12/ ./' \
        > /etc/apt/sources.list.d/mega.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends megacmd \
    && rm -rf /var/lib/apt/lists/* \
    && test -x /usr/bin/mega-get \
    && test -x /usr/bin/mega-cmd-server \
    && test -x /usr/bin/mega-version

RUN groupadd --gid 10001 app \
    && useradd --uid 10001 --gid app --create-home --shell /usr/sbin/nologin app \
    && mkdir -p /app/data /app/tmp/mega /app/static /app/data/mega-home \
    && chown -R app:app /app

WORKDIR /app
COPY --from=backend /build/target/release/fileuploader /usr/local/bin/fileuploader
COPY --from=frontend /build/frontend/dist /app/static

ENV APP_HOST=0.0.0.0 \
    APP_PORT=8080 \
    DATA_DIR=/app/data \
    DATABASE_URL=sqlite:///app/data/fileuploader.db \
    STATIC_DIR=/app/static \
    MEGA_HOME=/app/data/mega-home \
    MEGA_TEMP_DIR=/app/tmp/mega \
    MEGA_AUTO_START_SERVER=true \
    RUST_LOG=info,aws_config=warn,aws_smithy_runtime=warn,hyper=warn

LABEL org.opencontainers.image.title="FileUploader" \
      org.opencontainers.image.description="Self-hosted streaming HTTP/MEGA downloader with S3 multipart upload" \
      org.opencontainers.image.source="https://github.com/yusgsysh/fileuploader" \
      org.opencontainers.image.licenses="MIT"

USER app
EXPOSE 8080

HEALTHCHECK --interval=20s --timeout=5s --start-period=20s --retries=5 \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/fileuploader"]
