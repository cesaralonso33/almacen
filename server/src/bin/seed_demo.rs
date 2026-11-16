//! Semilla de desarrollo: usuarios y expedientes ficticios.
//!
//! Uso:
//!   DATABASE_URL=... cargo run --bin seed_demo
//!
//! Crea cuentas de demostración con la contraseña `Demo#2026` y marca
//! `debe_cambiar_password = true`; nunca deben usarse en producción.

use anyhow::Context;
use expedientes_api::auth::hash_password;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

const PASSWORD_DEMO: &str = "Demo#2026";

struct CuentaDemo {
    nombre: &'static str,
    correo: &'static str,
    rol: &'static str,
}

const CUENTAS: &[CuentaDemo] = &[
    CuentaDemo { nombre: "Administración", correo: "admin@demo.mx", rol: "ADMINISTRADOR" },
    CuentaDemo { nombre: "Almacén central", correo: "almacen@demo.mx", rol: "ALMACEN" },
    CuentaDemo { nombre: "Solicitante", correo: "solicitante@demo.mx", rol: "USUARIO" },
    CuentaDemo { nombre: "Consulta", correo: "consulta@demo.mx", rol: "CONSULTA" },
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL es obligatoria")?;
    let db = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .context("conectar a PostgreSQL")?;

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .context("aplicar migraciones")?;

    for cuenta in CUENTAS {
        let id = asegurar_usuario(&db, cuenta).await?;
        sqlx::query(
            "INSERT INTO usuario_roles(usuario_id, rol_id)
             SELECT $1, id FROM roles WHERE clave = $2
             ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(cuenta.rol)
        .execute(&db)
        .await?;
        println!("usuario {} ({}) rol {}", cuenta.correo, id, cuenta.rol);
    }

    let ubicacion: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM ubicaciones WHERE codigo = 'ARCHIVO' LIMIT 1")
            .fetch_optional(&db)
            .await?;

    for i in 1..=6 {
        let numero = format!("DEMO-2026-{i:04}");
        sqlx::query(
            "INSERT INTO expedientes(numero, descripcion, unidad, anio, ubicacion_id)
             VALUES ($1,$2,'JUZGADO DEMO',2026,$3)
             ON CONFLICT DO NOTHING",
        )
        .bind(&numero)
        .bind(format!("Expediente ficticio de demostración {i}"))
        .bind(ubicacion)
        .execute(&db)
        .await?;
    }

    println!("semilla aplicada. Contraseña de demostración: {PASSWORD_DEMO}");
    Ok(())
}

async fn asegurar_usuario(db: &PgPool, cuenta: &CuentaDemo) -> anyhow::Result<Uuid> {
    let existente: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM usuarios WHERE lower(correo) = lower($1)")
            .bind(cuenta.correo)
            .fetch_optional(db)
            .await?;

    if let Some(id) = existente {
        return Ok(id);
    }

    let hash = hash_password(PASSWORD_DEMO)
        .map_err(|error| anyhow::anyhow!("no fue posible calcular el hash: {error}"))?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO usuarios(nombre, correo, password_hash, debe_cambiar_password)
         VALUES ($1,$2,$3,true) RETURNING id",
    )
    .bind(cuenta.nombre)
    .bind(cuenta.correo)
    .bind(hash)
    .fetch_one(db)
    .await?;

    Ok(id)
}
