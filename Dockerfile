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
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

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

USER app
EXPOSE 8080

HEALTHCHECK --interval=20s --timeout=5s --start-period=20s --retries=5 \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/fileuploader"]
