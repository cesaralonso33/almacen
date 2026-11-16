use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{api::entrada::ApiJson, audit, auth::AuthUser, error::ApiError, AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolicitudDevolucion {
    pub expediente_id: Uuid,
    pub observaciones: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmacionDevolucion {
    pub ubicacion_id: Option<Uuid>,
}

/// El custodio solicita devolver el expediente. La custodia oficial **no** cambia;
/// sigue con el custodio hasta que el almacén acuse la recepción física.
pub async fn solicitar(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<SolicitudDevolucion>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("devoluciones:solicitar")?;

    let mut tx = app.db.begin().await?;

    let fila = sqlx::query_as::<_, (String, Option<Uuid>)>(
        "SELECT estado, custodio_id FROM expedientes WHERE id = $1 FOR UPDATE",
    )
    .bind(input.expediente_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (estado, custodio) =
        fila.ok_or_else(|| ApiError::no_encontrado("Expediente no encontrado"))?;

    if estado != "EN_CUSTODIA" || custodio != Some(user.id) {
        return Err(ApiError::conflicto("El usuario no es el custodio actual del expediente"));
    }

    let transferencia_en_curso: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM transferencias WHERE expediente_id = $1 AND estado = 'SOLICITADA')",
    )
    .bind(input.expediente_id)
    .fetch_one(&mut *tx)
    .await?;

    if transferencia_en_curso {
        return Err(ApiError::conflicto(
            "Existe una transferencia en curso para este expediente",
        ));
    }

    let devolucion_id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO devoluciones(expediente_id, custodio_id, observaciones)
         VALUES ($1,$2,$3)
         ON CONFLICT (expediente_id) WHERE estado = 'SOLICITADA' DO NOTHING
         RETURNING id",
    )
    .bind(input.expediente_id)
    .bind(user.id)
    .bind(input.observaciones.as_deref())
    .fetch_optional(&mut *tx)
    .await?;

    let devolucion_id = devolucion_id.ok_or_else(|| {
        ApiError::conflicto("Ya existe una devolución en curso para este expediente")
    })?;

    sqlx::query("INSERT INTO movimientos(expediente_id, tipo, actor_id) VALUES ($1,'SOLICITUD_DEVOLUCION',$2)")
        .bind(input.expediente_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "devoluciones.solicitar",
        "devoluciones",
        devolucion_id,
        "EXITO",
        json!({ "expediente_id": input.expediente_id }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({
        "devolucion_id": devolucion_id,
        "estado": "SOLICITADA",
        "custodia_actual": "CUSTODIO_ACTUAL",
    })))
}

/// El almacén confirma la recepción física y solo entonces se cierra la custodia.
pub async fn confirmar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<ConfirmacionDevolucion>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("devoluciones:confirmar")?;

    let mut tx = app.db.begin().await?;

    let devolucion = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT expediente_id, custodio_id FROM devoluciones WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let (expediente_id, custodio_id) =
        devolucion.ok_or_else(|| ApiError::no_encontrado("Devolución no encontrada"))?;

    // Orden de bloqueo global: primero el expediente.
    let fila = sqlx::query_as::<_, (String, Option<Uuid>)>(
        "SELECT estado, custodio_id FROM expedientes WHERE id = $1 FOR UPDATE",
    )
    .bind(expediente_id)
    .fetch_one(&mut *tx)
    .await?;

    let estado_devolucion: String =
        sqlx::query_scalar("SELECT estado FROM devoluciones WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;

    if estado_devolucion != "SOLICITADA" {
        return Err(ApiError::conflicto("La devolución ya fue procesada"));
    }

    let (estado_expediente, custodio_actual) = fila;
    if estado_expediente != "EN_CUSTODIA" || custodio_actual != Some(custodio_id) {
        return Err(ApiError::conflicto(
            "La custodia del expediente cambió desde que se solicitó la devolución",
        ));
    }

    if let Some(ubicacion) = input.ubicacion_id {
        let existe: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ubicaciones WHERE id = $1)")
            .bind(ubicacion)
            .fetch_one(&mut *tx)
            .await?;
        if !existe {
            return Err(ApiError::inprocesable("La ubicación de destino no existe"));
        }
    }

    sqlx::query("UPDATE custodias SET fin = now() WHERE expediente_id = $1 AND fin IS NULL")
        .bind(expediente_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE expedientes
         SET estado = 'ALMACEN',
             custodio_id = NULL,
             ubicacion_id = COALESCE($2, ubicacion_id),
             version = version + 1,
             actualizado_en = now()
         WHERE id = $1",
    )
    .bind(expediente_id)
    .bind(input.ubicacion_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE devoluciones
         SET estado = 'CONFIRMADA', confirmada_en = now(), recibida_por = $2, ubicacion_destino_id = $3
         WHERE id = $1",
    )
    .bind(id)
    .bind(user.id)
    .bind(input.ubicacion_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO movimientos(expediente_id, tipo, actor_id) VALUES ($1,'DEVOLUCION',$2)")
        .bind(expediente_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "devoluciones.confirmar",
        "devoluciones",
        id,
        "EXITO",
        json!({ "expediente_id": expediente_id, "custodio_anterior": custodio_id }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({
        "devolucion_id": id,
        "estado": "CONFIRMADA",
        "expediente_id": expediente_id,
    })))
}

/// Bandeja de devoluciones pendientes de acuse (almacén).
pub async fn listar(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("devoluciones:confirmar")?;

    let filas = sqlx::query_as::<_, (Uuid, Uuid, String, String, chrono::DateTime<chrono::Utc>, Option<String>)>(
        "SELECT d.id, d.expediente_id, e.numero, u.nombre, d.solicitada_en, d.observaciones
         FROM devoluciones d
         JOIN expedientes e ON e.id = d.expediente_id
         JOIN usuarios u ON u.id = d.custodio_id
         WHERE d.estado = 'SOLICITADA'
         ORDER BY d.solicitada_en",
    )
    .fetch_all(&app.db)
    .await?;

    let devoluciones: Vec<Value> = filas
        .into_iter()
        .map(|(id, expediente_id, numero, custodio, solicitada_en, observaciones)| {
            json!({
                "id": id,
                "expediente_id": expediente_id,
                "numero": numero,
                "custodio": custodio,
                "solicitada_en": solicitada_en,
                "observaciones": observaciones,
            })
        })
        .collect();

    Ok(Json(json!({ "devoluciones": devoluciones })))
}
