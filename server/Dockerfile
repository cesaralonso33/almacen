# API de expedientes — imagen de producción
FROM rust:1.90-slim AS compilacion

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
COPY assets ./assets

RUN cargo build --release --locked \
    && strip target/release/expedientes-api

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home expedientes

COPY --from=compilacion /app/target/release/expedientes-api /usr/local/bin/expedientes-api
COPY --from=compilacion /app/target/release/seed_demo /usr/local/bin/seed_demo
COPY --from=compilacion /app/target/release/crear_admin /usr/local/bin/crear_admin

USER 10001:10001
EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/expedientes-api"]
