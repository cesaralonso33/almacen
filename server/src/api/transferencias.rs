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
pub struct NuevaTransferencia {
    pub expediente_id: Uuid,
    pub destino_id: Uuid,
    pub observaciones: Option<String>,
}

/// El custodio solicita transferir la custodia. El custodio oficial sigue siendo el
/// originario hasta que el destino acepte.
pub async fn solicitar(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<NuevaTransferencia>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("transferencias:solicitar")?;

    if input.destino_id == user.id {
        return Err(ApiError::inprocesable(
            "No puede transferirse la custodia a sí mismo",
        ));
    }

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

    let destino_valido: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usuarios WHERE id = $1 AND activo)")
            .bind(input.destino_id)
            .fetch_one(&mut *tx)
            .await?;
    if !destino_valido {
        return Err(ApiError::inprocesable("El usuario destino no existe o está inactivo"));
    }

    let devolucion_en_curso: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM devoluciones WHERE expediente_id = $1 AND estado = 'SOLICITADA')",
    )
    .bind(input.expediente_id)
    .fetch_one(&mut *tx)
    .await?;

    if devolucion_en_curso {
        return Err(ApiError::conflicto(
            "Existe una devolución en curso para este expediente",
        ));
    }

    let transferencia_id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO transferencias(expediente_id, origen_id, destino_id, observaciones)
         VALUES ($1,$2,$3,$4)
         ON CONFLICT (expediente_id) WHERE estado = 'SOLICITADA' DO NOTHING
         RETURNING id",
    )
    .bind(input.expediente_id)
    .bind(user.id)
    .bind(input.destino_id)
    .bind(input.observaciones.as_deref())
    .fetch_optional(&mut *tx)
    .await?;

    let transferencia_id = transferencia_id.ok_or_else(|| {
        ApiError::conflicto("Ya existe una transferencia en curso para este expediente")
    })?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "transferencias.solicitar",
        "transferencias",
        transferencia_id,
        "EXITO",
        json!({ "expediente_id": input.expediente_id, "destino_id": input.destino_id }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({
        "transferencia_id": transferencia_id,
        "estado": "SOLICITADA",
        "custodia_actual": "ORIGEN",
    })))
}

pub async fn aceptar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("transferencias:aceptar")?;
    resolver(&app, &user, id, true).await
}

pub async fn rechazar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("transferencias:aceptar")?;
    resolver(&app, &user, id, false).await
}

async fn resolver(
    app: &AppState,
    user: &AuthUser,
    id: Uuid,
    aceptar: bool,
) -> Result<Json<Value>, ApiError> {
    let mut tx = app.db.begin().await?;

    let cabecera = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String)>(
        "SELECT expediente_id, origen_id, destino_id, estado FROM transferencias WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let (expediente_id, origen_id, destino_id, _) =
        cabecera.ok_or_else(|| ApiError::no_encontrado("Transferencia no encontrada"))?;

    // Orden de bloqueo global: primero el expediente, después la transferencia.
    sqlx::query("SELECT id FROM expedientes WHERE id = $1 FOR UPDATE")
        .bind(expediente_id)
        .fetch_one(&mut *tx)
        .await?;

    let estado: String =
        sqlx::query_scalar("SELECT estado FROM transferencias WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;

    if estado != "SOLICITADA" {
        return Err(ApiError::conflicto("La transferencia ya fue resuelta"));
    }

    if destino_id != user.id {
        return Err(ApiError::prohibido(
            "Solo el usuario destino puede aceptar o rechazar la transferencia",
        ));
    }

    if !aceptar {
        sqlx::query(
            "UPDATE transferencias SET estado = 'RECHAZADA', resuelta_en = now() WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;

        audit::registrar(
            &mut tx,
            Some(user.id),
            "transferencias.rechazar",
            "transferencias",
            id,
            "EXITO",
            json!({ "expediente_id": expediente_id }),
        )
        .await?;

        tx.commit().await?;
        return Ok(Json(json!({ "transferencia_id": id, "estado": "RECHAZADA" })));
    }

    let (estado_expediente, custodio) = sqlx::query_as::<_, (String, Option<Uuid>)>(
        "SELECT estado, custodio_id FROM expedientes WHERE id = $1",
    )
    .bind(expediente_id)
    .fetch_one(&mut *tx)
    .await?;

    if estado_expediente != "EN_CUSTODIA" || custodio != Some(origen_id) {
        return Err(ApiError::conflicto(
            "La custodia del expediente ya no corresponde al origen de la transferencia",
        ));
    }

    // Cerrar la custodia anterior antes de abrir la nueva: el índice único parcial
    // solo admite una custodia activa por expediente.
    sqlx::query("UPDATE custodias SET fin = now() WHERE expediente_id = $1 AND fin IS NULL")
        .bind(expediente_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("INSERT INTO custodias(expediente_id, usuario_id) VALUES ($1,$2)")
        .bind(expediente_id)
        .bind(destino_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE expedientes
         SET custodio_id = $2, version = version + 1, actualizado_en = now()
         WHERE id = $1",
    )
    .bind(expediente_id)
    .bind(destino_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE transferencias SET estado = 'ACEPTADA', resuelta_en = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO movimientos(expediente_id, tipo, actor_id, destino_id) VALUES ($1,'TRANSFERENCIA',$2,$3)")
        .bind(expediente_id)
        .bind(origen_id)
        .bind(destino_id)
        .execute(&mut *tx)
        .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "transferencias.aceptar",
        "transferencias",
        id,
        "EXITO",
        json!({ "expediente_id": expediente_id, "origen_id": origen_id, "destino_id": destino_id }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({
        "transferencia_id": id,
        "estado": "ACEPTADA",
        "custodio_id": destino_id,
    })))
}
