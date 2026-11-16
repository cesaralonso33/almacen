use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

/// Registra un evento en la bitácora append-only.
///
/// Debe invocarse dentro de la misma transacción que la operación auditada para que
/// no existan acciones sin evidencia ni evidencia sin acción.
pub async fn registrar(
    conexion: &mut PgConnection,
    actor: Option<Uuid>,
    accion: &str,
    entidad: &str,
    entidad_id: Uuid,
    resultado: &str,
    detalle: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO auditoria(actor_id, accion, entidad, entidad_id, resultado, detalle)
         VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(actor)
    .bind(accion)
    .bind(entidad)
    .bind(entidad_id)
    .bind(resultado)
    .bind(detalle)
    .execute(conexion)
    .await?;
    Ok(())
}
