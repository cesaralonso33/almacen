# API de expedientes — imagen de producción
# El builder se fija a bookworm para enlazar con la misma glibc (2.36) que la
# imagen final debian:bookworm-slim. El tag sin sufijo puede resolver a una base
# más nueva y producir "GLIBC_2.39 not found" al iniciar el contenedor.
FROM rust:1.90-slim-bookworm AS compilacion

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
    && install -d /usr/share/postgresql-common/pgdg \
    && curl -fsSL -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc https://www.postgresql.org/media/keys/ACCC4CF8.asc \
    && echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] https://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" > /etc/apt/sources.list.d/pgdg.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends postgresql-client-17 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home expedientes

COPY --from=compilacion /app/target/release/expedientes-api /usr/local/bin/expedientes-api
COPY --from=compilacion /app/target/release/seed_demo /usr/local/bin/seed_demo
COPY --from=compilacion /app/target/release/crear_admin /usr/local/bin/crear_admin

USER 10001:10001
EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/expedientes-api"]
