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
pub struct DespachoInput {
    pub item_ids: Vec<Uuid>,
}

pub async fn despachar(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<DespachoInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("entregas:despachar")?;

    if input.item_ids.is_empty() || input.item_ids.len() > 100 {
        return Err(ApiError::solicitud_invalida("Seleccione entre 1 y 100 renglones"));
    }

    let mut ids = input.item_ids.clone();
    ids.sort();
    ids.dedup();
    if ids.len() != input.item_ids.len() {
        return Err(ApiError::solicitud_invalida("Renglones duplicados en la solicitud"));
    }

    let mut tx = app.db.begin().await?;

    let pares = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT id, expediente_id FROM pedido_items WHERE id = ANY($1) ORDER BY expediente_id",
    )
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;

    if pares.len() != ids.len() {
        return Err(ApiError::no_encontrado("Uno o más renglones no existen"));
    }

    let mut expedientes: Vec<Uuid> = pares.iter().map(|(_, exp)| *exp).collect();
    expedientes.sort();
    expedientes.dedup();

    // Orden de bloqueo global: expedientes primero (ordenados), luego renglones.
    for expediente in &expedientes {
        sqlx::query("SELECT id FROM expedientes WHERE id = $1 FOR UPDATE")
            .bind(expediente)
            .fetch_one(&mut *tx)
            .await?;
    }

    let filas = sqlx::query_as::<_, (Uuid, Uuid, Uuid, String)>(
        "SELECT i.id, i.expediente_id, p.solicitante_id, i.estado
         FROM pedido_items i
         JOIN pedidos p ON p.id = i.pedido_id
         WHERE i.id = ANY($1)
         ORDER BY i.id
         FOR UPDATE OF i",
    )
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;

    let mut entregas = Vec::with_capacity(filas.len());

    for (item_id, expediente, receptor, estado_item) in filas {
        if estado_item != "PREPARADO" {
            return Err(ApiError::conflicto(
                "El renglón no está preparado para despacho",
            ));
        }

        let estado_expediente: String =
            sqlx::query_scalar("SELECT estado FROM expedientes WHERE id = $1")
                .bind(expediente)
                .fetch_one(&mut *tx)
                .await?;
        if estado_expediente != "ALMACEN" {
            return Err(ApiError::conflicto(
                "El expediente ya no está disponible en almacén",
            ));
        }

        let entrega_id: Uuid = sqlx::query_scalar(
            "INSERT INTO entregas(pedido_item_id, entregador_id, receptor_id)
             VALUES ($1,$2,$3) RETURNING id",
        )
        .bind(item_id)
        .bind(user.id)
        .bind(receptor)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query("UPDATE pedido_items SET estado = 'EN_ENTREGA' WHERE id = $1")
            .bind(item_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "UPDATE reservas SET liberada_en = now(), motivo_liberacion = 'DESPACHADA'
             WHERE pedido_item_id = $1 AND liberada_en IS NULL",
        )
        .bind(item_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("INSERT INTO movimientos(expediente_id, tipo, actor_id, destino_id) VALUES ($1,'DESPACHO',$2,$3)")
            .bind(expediente)
            .bind(user.id)
            .bind(receptor)
            .execute(&mut *tx)
            .await?;

        audit::registrar(
            &mut tx,
            Some(user.id),
            "entregas.despachar",
            "entregas",
            entrega_id,
            "EXITO",
            json!({ "pedido_item_id": item_id, "expediente_id": expediente, "receptor_id": receptor }),
        )
        .await?;

        entregas.push(entrega_id);
    }

    tx.commit().await?;
    Ok(Json(json!({ "entregas": entregas, "estado": "DESPACHADA" })))
}

pub async fn confirmar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("entregas:confirmar")?;

    let mut tx = app.db.begin().await?;

    let expediente: Option<Uuid> = sqlx::query_scalar(
        "SELECT i.expediente_id
         FROM entregas t JOIN pedido_items i ON i.id = t.pedido_item_id
         WHERE t.id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let expediente = expediente.ok_or_else(|| ApiError::no_encontrado("Entrega no encontrada"))?;

    sqlx::query("SELECT id FROM expedientes WHERE id = $1 FOR UPDATE")
        .bind(expediente)
        .fetch_one(&mut *tx)
        .await?;

    let fila = sqlx::query_as::<_, (Uuid, Uuid, String, String)>(
        "SELECT t.pedido_item_id, t.receptor_id, t.estado, i.estado
         FROM entregas t JOIN pedido_items i ON i.id = t.pedido_item_id
         WHERE t.id = $1
         FOR UPDATE OF t, i",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let (item, receptor, estado_entrega, estado_item) =
        fila.ok_or_else(|| ApiError::no_encontrado("Entrega no encontrada"))?;

    if receptor != user.id {
        return Err(ApiError::prohibido("Solo el receptor designado puede confirmar la entrega"));
    }

    if estado_entrega != "DESPACHADA" || estado_item != "EN_ENTREGA" {
        return Err(ApiError::conflicto("La entrega ya fue procesada"));
    }

    let estado_expediente: String =
        sqlx::query_scalar("SELECT estado FROM expedientes WHERE id = $1")
            .bind(expediente)
            .fetch_one(&mut *tx)
            .await?;
    if estado_expediente != "ALMACEN" {
        return Err(ApiError::conflicto(
            "La custodia del expediente es incompatible con esta entrega",
        ));
    }

    sqlx::query(
        "INSERT INTO custodias(expediente_id, usuario_id, entrega_id) VALUES ($1,$2,$3)",
    )
    .bind(expediente)
    .bind(receptor)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE expedientes
         SET estado = 'EN_CUSTODIA', custodio_id = $2, version = version + 1, actualizado_en = now()
         WHERE id = $1",
    )
    .bind(expediente)
    .bind(receptor)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE entregas SET estado = 'RECIBIDA', confirmada_en = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE pedido_items SET estado = 'ENTREGADO' WHERE id = $1")
        .bind(item)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "INSERT INTO acuses(entrega_id, receptor_id, sesion_id) VALUES ($1,$2,$3)",
    )
    .bind(id)
    .bind(receptor)
    .bind(user.sesion_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO movimientos(expediente_id, tipo, actor_id, destino_id) VALUES ($1,'RECEPCION',$2,$2)")
        .bind(expediente)
        .bind(receptor)
        .execute(&mut *tx)
        .await?;

    audit::registrar(
        &mut tx,
        Some(receptor),
        "entregas.confirmar",
        "entregas",
        id,
        "EXITO",
        json!({ "expediente_id": expediente, "sesion_id": user.sesion_id }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({
        "entrega_id": id,
        "estado": "RECIBIDA",
        "custodio_id": receptor,
    })))
}




