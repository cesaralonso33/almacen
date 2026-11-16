//! Crea la cuenta administradora inicial sin datos de demostración.
//!
//! Uso:
//!   ADMIN_CORREO=jefe@institucion.mx ADMIN_PASSWORD='...' \
//!   ADMIN_NOMBRE='Dirección jurídica' cargo run --bin crear_admin

use anyhow::{bail, Context};
use expedientes_api::auth::hash_password;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL es obligatoria")?;
    let correo = std::env::var("ADMIN_CORREO").context("ADMIN_CORREO es obligatoria")?;
    let password = std::env::var("ADMIN_PASSWORD").context("ADMIN_PASSWORD es obligatoria")?;
    let nombre = std::env::var("ADMIN_NOMBRE").unwrap_or_else(|_| "Administrador".to_string());

    if !correo.contains('@') {
        bail!("ADMIN_CORREO no parece un correo válido");
    }
    if password.chars().count() < 12 {
        bail!("ADMIN_PASSWORD debe tener al menos 12 caracteres");
    }

    let db = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .context("no fue posible conectar a PostgreSQL")?;

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .context("aplicar migraciones")?;

    let id = crear(&db, &nombre, &correo, &password).await?;
    println!("administrador creado: {correo} ({id})");
    Ok(())
}

async fn crear(db: &PgPool, nombre: &str, correo: &str, password: &str) -> anyhow::Result<Uuid> {
    let existente: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM usuarios WHERE lower(correo) = lower($1)")
            .bind(correo)
            .fetch_optional(db)
            .await?;

    if existente.is_some() {
        bail!("ya existe un usuario con el correo {correo}");
    }

    let hash = hash_password(password)
        .map_err(|error| anyhow::anyhow!("no fue posible calcular el hash: {error}"))?;

    let mut tx = db.begin().await?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO usuarios(nombre, correo, password_hash, debe_cambiar_password)
         VALUES ($1,$2,$3,false) RETURNING id",
    )
    .bind(nombre)
    .bind(correo)
    .bind(hash)
    .fetch_one(&mut *tx)
    .await?;

    let asignados = sqlx::query(
        "INSERT INTO usuario_roles(usuario_id, rol_id)
         SELECT $1, id FROM roles WHERE clave = 'ADMINISTRADOR'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if asignados == 0 {
        bail!("no existe el rol ADMINISTRADOR; revise las migraciones");
    }

    tx.commit().await?;
    Ok(id)
}
