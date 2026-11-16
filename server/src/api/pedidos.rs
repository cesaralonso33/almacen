use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{api::entrada::ApiJson, audit, auth::AuthUser, error::ApiError, AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrearPedido {
    pub expedientes: Vec<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescarteInput {
    pub motivo_clave: String,
    pub detalle: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelacionPedido {
    pub motivo: String,
}

pub async fn crear(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<CrearPedido>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pedidos:crear")?;

    if input.expedientes.is_empty() || input.expedientes.len() > 100 {
        return Err(ApiError::solicitud_invalida(
            "Debe solicitar entre 1 y 100 expedientes",
        ));
    }

    let mut ids = input.expedientes.clone();
    ids.sort();
    ids.dedup();
    if ids.len() != input.expedientes.len() {
        return Err(ApiError::solicitud_invalida("Expedientes duplicados en la solicitud"));
    }

    let existentes: i64 = sqlx::query_scalar("SELECT count(*) FROM expedientes WHERE id = ANY($1)")
        .bind(&ids)
        .fetch_one(&app.db)
        .await?;
    if existentes as usize != ids.len() {
        return Err(ApiError::inprocesable("Uno o más expedientes no existen"));
    }

    let mut tx = app.db.begin().await?;
    let pedido_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pedidos(solicitante_id, version) VALUES ($1,1) RETURNING id",
    )
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;

    let mut items = Vec::with_capacity(ids.len());
    for expediente in &ids {
        let item_id: Uuid = sqlx::query_scalar(
            "INSERT INTO pedido_items(pedido_id, expediente_id) VALUES ($1,$2) RETURNING id",
        )
        .bind(pedido_id)
        .bind(expediente)
        .fetch_one(&mut *tx)
        .await?;
        items.push((item_id, *expediente));
    }

    crate::sync::a_usuario_y_almacen(
        &mut tx,
        "pedido",
        pedido_id,
        "CREAR",
        user.id,
        1,
        json!({ "id": pedido_id, "solicitante_id": user.id, "estado": "PENDIENTE", "version": 1 }),
    )
    .await?;

    for (item_id, expediente) in &items {
        crate::sync::a_usuario_y_almacen(
            &mut tx,
            "pedido_item",
            *item_id,
            "CREAR",
            user.id,
            1,
            json!({ "id": item_id, "pedido_id": pedido_id, "expediente_id": expediente, "estado": "PENDIENTE" }),
        )
        .await?;
    }

    audit::registrar(
        &mut tx,
        Some(user.id),
        "pedidos.crear",
        "pedidos",
        pedido_id,
        "EXITO",
        json!({ "expedientes": ids }),
    )
    .await?;

    tx.commit().await?;
    Ok(Json(json!({ "pedido_id": pedido_id })))
}

pub async fn detalle(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pedidos:leer")?;

    let cabecera = sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>, Option<String>)>(
        "SELECT solicitante_id, cancelado_en, cancelado_motivo FROM pedidos WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&app.db)
    .await?;

    let Some((solicitante, cancelado_en, cancelado_motivo)) = cabecera else {
        return Err(ApiError::no_encontrado("Pedido no encontrado"));
    };

    if solicitante != user.id && !user.puede("pedidos:preparar") {
        // No se revela la existencia de pedidos ajenos.
        return Err(ApiError::no_encontrado("Pedido no encontrado"));
    }

    let filas: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, estado FROM pedido_items WHERE pedido_id = $1 ORDER BY expediente_id",
    )
    .bind(id)
    .fetch_all(&app.db)
    .await?;

    let total = filas.len();
    let entregados = filas.iter().filter(|f| f.1 == "ENTREGADO").count();
    let descartados = filas.iter().filter(|f| f.1 == "DESCARTADO").count();
    let cancelados = filas.iter().filter(|f| f.1 == "CANCELADO").count();
    let resueltos = entregados + descartados + cancelados;
    let en_proceso = filas
        .iter()
        .any(|f| matches!(f.1.as_str(), "RESERVADO" | "PREPARADO" | "EN_ENTREGA"));

    let estado = if cancelado_en.is_some() {
        "CANCELADO"
    } else if total > 0 && resueltos == total {
        "COMPLETADO"
    } else if resueltos > 0 {
        "PARCIAL"
    } else if en_proceso {
        "EN_PROCESO"
    } else {
        "PENDIENTE"
    };

    let items: Vec<Value> = filas
        .into_iter()
        .map(|(item_id, estado_item)| json!({ "id": item_id, "estado": estado_item }))
        .collect();

    Ok(Json(json!({
        "id": id,
        "solicitante_id": solicitante,
        "estado": estado,
        "entregados": entregados,
        "descartados": descartados,
        "cancelados": cancelados,
        "cancelado_en": cancelado_en,
        "cancelado_motivo": cancelado_motivo,
        "items": items,
    })))
}

pub async fn preparar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pedidos:preparar")?;

    let mut tx = app.db.begin().await?;

    let expediente: Option<Uuid> =
        sqlx::query_scalar("SELECT expediente_id FROM pedido_items WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let expediente = expediente.ok_or_else(|| ApiError::no_encontrado("Renglón no encontrado"))?;

    // Orden de bloqueo global: primero el expediente, después el renglón.
    sqlx::query("SELECT id FROM expedientes WHERE id = $1 FOR UPDATE")
        .bind(expediente)
        .fetch_one(&mut *tx)
        .await?;

    let (expediente_releido, estado_item, pedido_id, solicitante): (Uuid, String, Uuid, Uuid) = sqlx::query_as(
        "SELECT i.expediente_id, i.estado, i.pedido_id, p.solicitante_id
         FROM pedido_items i JOIN pedidos p ON p.id = i.pedido_id
         WHERE i.id = $1 FOR UPDATE OF i",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    debug_assert_eq!(expediente, expediente_releido);

    if estado_item != "PENDIENTE" {
        return Err(ApiError::conflicto("El renglón no está pendiente"));
    }

    let estado_expediente: String =
        sqlx::query_scalar("SELECT estado FROM expedientes WHERE id = $1")
            .bind(expediente)
            .fetch_one(&mut *tx)
            .await?;
    if estado_expediente != "ALMACEN" {
        return Err(ApiError::conflicto("El expediente no está disponible en almacén"));
    }

    let expira = Utc::now()
        + Duration::from_std(app.config.reserva_ttl)
            .unwrap_or_else(|_| Duration::minutes(30));

    let reservadas = sqlx::query(
        "INSERT INTO reservas(expediente_id, pedido_item_id, reservada_por, expira_en)
         VALUES ($1,$2,$3,$4)
         ON CONFLICT (expediente_id) WHERE liberada_en IS NULL DO NOTHING",
    )
    .bind(expediente)
    .bind(id)
    .bind(user.id)
    .bind(expira)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if reservadas == 0 {
        return Err(ApiError::conflicto("El expediente ya está reservado por otro pedido"));
    }

    sqlx::query("UPDATE pedido_items SET estado = 'PREPARADO' WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    crate::sync::a_usuario_y_almacen(
        &mut tx,
        "pedido_item",
        id,
        "ACTUALIZAR",
        solicitante,
        0,
        json!({ "id": id, "pedido_id": pedido_id, "expediente_id": expediente, "estado": "PREPARADO" }),
    )
    .await?;

    sqlx::query("INSERT INTO movimientos(expediente_id, tipo, actor_id) VALUES ($1,'PREPARACION',$2)")
        .bind(expediente)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "pedidos.preparar",
        "pedido_items",
        id,
        "EXITO",
        json!({ "expediente_id": expediente }),
    )
    .await?;

    tx.commit().await?;
    Ok(Json(json!({ "item_id": id, "estado": "PREPARADO" })))
}

pub async fn descartar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<DescarteInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pedidos:descartar")?;

    let mut tx = app.db.begin().await?;

    let fila = sqlx::query_as::<_, (Uuid, String, Uuid, Uuid)>(
        "SELECT i.expediente_id, i.estado, i.pedido_id, p.solicitante_id
         FROM pedido_items i JOIN pedidos p ON p.id = i.pedido_id
         WHERE i.id = $1 FOR UPDATE OF i",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (expediente, estado, pedido_id, solicitante) =
        fila.ok_or_else(|| ApiError::no_encontrado("Renglón no encontrado"))?;

    if estado != "PENDIENTE" && estado != "PREPARADO" {
        return Err(ApiError::conflicto("El renglón no es descartable en su estado actual"));
    }

    let motivo = sqlx::query_as::<_, (String, bool)>(
        "SELECT descripcion, requiere_detalle FROM motivos_descarte WHERE clave = $1",
    )
    .bind(&input.motivo_clave)
    .fetch_optional(&mut *tx)
    .await?;

    let (descripcion, requiere_detalle) =
        motivo.ok_or_else(|| ApiError::inprocesable("Motivo de descarte no catalogado"))?;

    let detalle = input.detalle.as_deref().map(str::trim).filter(|d| !d.is_empty());
    if requiere_detalle && detalle.map(str::len).unwrap_or(0) < 5 {
        return Err(ApiError::inprocesable(
            "El motivo OTRO requiere una descripción de al menos 5 caracteres",
        ));
    }

    sqlx::query(
        "INSERT INTO descartes(pedido_item_id, motivo_clave, detalle, usuario_id)
         VALUES ($1,$2,$3,$4)",
    )
    .bind(id)
    .bind(&input.motivo_clave)
    .bind(detalle)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

    let texto = match detalle {
        Some(d) => format!("{descripcion}: {d}"),
        None => descripcion,
    };

    sqlx::query("UPDATE pedido_items SET estado = 'DESCARTADO', motivo_descarte = $2 WHERE id = $1")
        .bind(id)
        .bind(&texto)
        .execute(&mut *tx)
        .await?;

    crate::sync::a_usuario_y_almacen(
        &mut tx,
        "pedido_item",
        id,
        "ACTUALIZAR",
        solicitante,
        0,
        json!({ "id": id, "pedido_id": pedido_id, "expediente_id": expediente, "estado": "DESCARTADO" }),
    )
    .await?;

    sqlx::query(
        "UPDATE reservas SET liberada_en = now(), motivo_liberacion = 'DESCARTE'
         WHERE pedido_item_id = $1 AND liberada_en IS NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO movimientos(expediente_id, tipo, actor_id, detalle) VALUES ($1,'DESCARTE_PEDIDO',$2,$3)")
        .bind(expediente)
        .bind(user.id)
        .bind(&texto)
        .execute(&mut *tx)
        .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "pedidos.descartar",
        "pedido_items",
        id,
        "EXITO",
        json!({ "motivo_clave": input.motivo_clave, "detalle": detalle }),
    )
    .await?;

    tx.commit().await?;
    Ok(Json(json!({ "item_id": id, "estado": "DESCARTADO" })))
}





pub async fn cancelar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<CancelacionPedido>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pedidos:crear")?;

    let motivo = input.motivo.trim();
    if motivo.chars().count() < 5 {
        return Err(ApiError::inprocesable(
            "El motivo de cancelación debe tener al menos 5 caracteres",
        ));
    }

    let mut tx = app.db.begin().await?;

    let cabecera = sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
        "SELECT solicitante_id, cancelado_en FROM pedidos WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let (solicitante, cancelado_en) =
        cabecera.ok_or_else(|| ApiError::no_encontrado("Pedido no encontrado"))?;

    if solicitante != user.id {
        return Err(ApiError::no_encontrado("Pedido no encontrado"));
    }
    if cancelado_en.is_some() {
        return Err(ApiError::conflicto("El pedido ya está cancelado"));
    }

    let irresolubles: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pedido_items
         WHERE pedido_id = $1 AND estado IN ('ENTREGADO','EN_ENTREGA')",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    if irresolubles > 0 {
        return Err(ApiError::conflicto(
            "No se puede cancelar un pedido con renglones entregados o en tránsito",
        ));
    }

    sqlx::query(
        "UPDATE pedido_items SET estado = 'CANCELADO', cancelado_motivo = $2
         WHERE pedido_id = $1 AND estado IN ('PENDIENTE','RESERVADO','PREPARADO')",
    )
    .bind(id)
    .bind(motivo)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE reservas SET liberada_en = now(), motivo_liberacion = 'CANCELACION'
         WHERE liberada_en IS NULL
           AND pedido_item_id IN (SELECT id FROM pedido_items WHERE pedido_id = $1)",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    let items: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, estado FROM pedido_items WHERE pedido_id = $1")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;

    let version: i64 = sqlx::query_scalar(
        "UPDATE pedidos
         SET cancelado_en = now(), cancelado_por = $2, cancelado_motivo = $3, version = version + 1
         WHERE id = $1
         RETURNING version",
    )
    .bind(id)
    .bind(user.id)
    .bind(motivo)
    .fetch_one(&mut *tx)
    .await?;

    crate::sync::a_usuario_y_almacen(
        &mut tx,
        "pedido",
        id,
        "ACTUALIZAR",
        user.id,
        version,
        json!({ "id": id, "solicitante_id": user.id, "estado": "CANCELADO", "version": version }),
    )
    .await?;

    for (item_id, estado_item) in &items {
        crate::sync::a_usuario_y_almacen(
            &mut tx,
            "pedido_item",
            *item_id,
            "ACTUALIZAR",
            user.id,
            version,
            json!({ "id": item_id, "pedido_id": id, "estado": estado_item }),
        )
        .await?;
    }

    audit::registrar(
        &mut tx,
        Some(user.id),
        "pedidos.cancelar",
        "pedidos",
        id,
        "EXITO",
        json!({ "motivo": motivo, "renglones_irresolubles": irresolubles }),
    )
    .await?;

    tx.commit().await?;
    Ok(Json(json!({ "pedido_id": id, "estado": "CANCELADO" })))
}
