//! Emisión de eventos de sincronización.
//!
//! Cada evento se escribe en la misma transacción que la operación de negocio: no puede
//! existir un cambio sin evento ni un evento de un cambio que no ocurrió.

use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

pub const ROL_ALMACEN: &str = "ALMACEN";

#[allow(clippy::too_many_arguments)]
pub async fn emitir(
    conexion: &mut PgConnection,
    entidad: &str,
    entidad_id: Uuid,
    operacion: &str,
    visibilidad: &str,
    rol_clave: Option<&str>,
    usuario_id: Option<Uuid>,
    version: i64,
    payload: Value,
) -> Result<(), sqlx::Error> {
    // La tabla exige version > 0; las entidades sin versión propia usan 1.
    let version = version.max(1);
    sqlx::query(
        "INSERT INTO sync_events
            (entidad, entidad_id, operacion, version, visibilidad, rol_clave, usuario_id, payload)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(entidad)
    .bind(entidad_id)
    .bind(operacion)
    .bind(version)
    .bind(visibilidad)
    .bind(rol_clave)
    .bind(usuario_id)
    .bind(payload)
    .execute(conexion)
    .await?;
    Ok(())
}

/// Evento visible para el dueño del pedido y para el rol de almacén.
pub async fn a_usuario_y_almacen(
    conexion: &mut PgConnection,
    entidad: &str,
    entidad_id: Uuid,
    operacion: &str,
    usuario_id: Uuid,
    version: i64,
    payload: Value,
) -> Result<(), sqlx::Error> {
    emitir(
        conexion,
        entidad,
        entidad_id,
        operacion,
        "USUARIO",
        None,
        Some(usuario_id),
        version,
        payload.clone(),
    )
    .await?;
    emitir(
        conexion,
        entidad,
        entidad_id,
        operacion,
        "ROL",
        Some(ROL_ALMACEN),
        None,
        version,
        payload,
    )
    .await
}

/// Evento visible para cualquier usuario autenticado (inventario y custodia).
pub async fn global(
    conexion: &mut PgConnection,
    entidad: &str,
    entidad_id: Uuid,
    operacion: &str,
    version: i64,
    payload: Value,
) -> Result<(), sqlx::Error> {
    emitir(conexion, entidad, entidad_id, operacion, "GLOBAL", None, None, version, payload).await
}
