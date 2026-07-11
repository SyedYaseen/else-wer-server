# syntax=docker/dockerfile:1

# --- Stage 1: build the PWA (src/ui) ---
FROM node:20-slim AS pwa-build
WORKDIR /pwa
COPY src/ui/package.json src/ui/package-lock.json ./
RUN npm ci
COPY src/ui/ ./
RUN npm run build

# --- Stage 2: build the Rust server ---
FROM rust:1-slim-bookworm AS rust-build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev sqlite3 && rm -rf /var/lib/apt/lists/*
RUN cargo install sqlx-cli --version ^0.8 --no-default-features --features rustls,sqlite
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY src ./src
# sqlx::query! macros type-check against a real schema at compile time; build a throwaway
# DB here (never shipped) and run migrations into it just to satisfy the macros.
RUN sqlx database create --database-url sqlite:./build.db \
    && sqlx migrate run --database-url sqlite:./build.db
ENV DATABASE_URL=sqlite:./build.db
RUN cargo build --release --bin else-wer

# --- Stage 3: runtime ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 wget && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-build /app/target/release/else-wer ./else-wer
COPY --from=pwa-build /pwa/dist ./src/ui/dist

ENV HOST=0.0.0.0 \
    PORT=3000 \
    DATABASE_URL=sqlite:/data/rustybookshelf.db \
    AUDIOBOOKS_LOCATION=/audiobooks \
    JWT_LOC=/data/creds/jwt.key \
    PWA_DIST_LOCATION=/app/src/ui/dist \
    SELF_HOSTED=true

VOLUME ["/data", "/audiobooks"]
EXPOSE 3000

# covers/ is created relative to the process cwd (see src/file_ops/book_cover.rs)
RUN mkdir -p /data/covers /data/creds && ln -s /data/covers /app/covers

CMD ["./else-wer"]
