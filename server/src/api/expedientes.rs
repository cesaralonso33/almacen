use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{audit, auth::AuthUser, error::ApiError, AppState};

pub async fn devolver(
    State(app): State<AppState>,
    user: AuthUser,
    Path(expediente): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("devoluciones:solicitar")?;

    let mut tx = app.db.begin().await?;

    let fila = sqlx::query_as::<_, (String, Option<Uuid>)>(
        "SELECT estado, custodio_id FROM expedientes WHERE id = $1 FOR UPDATE",
    )
    .bind(expediente)
    .fetch_optional(&mut *tx)
    .await?;

    let (estado, custodio) =
        fila.ok_or_else(|| ApiError::no_encontrado("Expediente no encontrado"))?;

    if estado != "EN_CUSTODIA" || custodio != Some(user.id) {
        return Err(ApiError::conflicto("El usuario no es el custodio actual del expediente"));
    }

    sqlx::query("UPDATE custodias SET fin = now() WHERE expediente_id = $1 AND fin IS NULL")
        .bind(expediente)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE expedientes
         SET estado = 'ALMACEN', custodio_id = NULL, version = version + 1, actualizado_en = now()
         WHERE id = $1",
    )
    .bind(expediente)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO movimientos(expediente_id, tipo, actor_id) VALUES ($1,'DEVOLUCION',$2)")
        .bind(expediente)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "expedientes.devolver",
        "expedientes",
        expediente,
        "EXITO",
        json!({ "pendiente_acuse_almacen": true }),
    )
    .await?;

    tx.commit().await?;
    Ok(Json(json!({ "expediente_id": expediente, "estado": "ALMACEN" })))
}
