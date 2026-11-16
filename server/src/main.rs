use std::time::Duration;

use anyhow::Context;
use expedientes_api::{api, config::Config, limiter::LoginLimiter, AppState};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let config = Config::desde_entorno()?;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await
        .context("no fue posible conectar a PostgreSQL")?;

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .context("no fue posible aplicar las migraciones")?;

    let estado = AppState::nuevo(db, config.clone(), LoginLimiter::nuevo(5, 900));
    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .with_context(|| format!("no fue posible escuchar en {}", config.bind_addr))?;

    tracing::info!(direccion = %config.bind_addr, "API de expedientes iniciada");

    axum::serve(listener, api::router(estado))
        .with_graceful_shutdown(apagar())
        .await?;

    Ok(())
}

async fn apagar() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("señal de apagado recibida; cerrando la API");
}
