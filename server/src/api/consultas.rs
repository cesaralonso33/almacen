use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{auth::AuthUser, error::ApiError, AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusquedaExpedientes {
    pub buscar: Option<String>,
    pub estado: Option<String>,
    pub limite: Option<i64>,
}

pub async fn dashboard(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    let todas = user.puede("pedidos:preparar");

    let pedidos_pendientes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pedidos p
         WHERE p.cancelado_en IS NULL
           AND ($1::bool OR p.solicitante_id = $2)
           AND NOT EXISTS (
               SELECT 1 FROM pedido_items i
               WHERE i.pedido_id = p.id AND i.estado <> 'PENDIENTE'
           )",
    )
    .bind(todas)
    .bind(user.id)
    .fetch_one(&app.db)
    .await?;

    let pedidos_por_preparar: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pedido_items i
         JOIN pedidos p ON p.id = i.pedido_id
         WHERE i.estado = 'PENDIENTE' AND p.cancelado_en IS NULL
           AND ($1::bool OR p.solicitante_id = $2)",
    )
    .bind(todas)
    .bind(user.id)
    .fetch_one(&app.db)
    .await?;

    let en_entrega: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pedido_items i
         JOIN pedidos p ON p.id = i.pedido_id
         WHERE i.estado = 'EN_ENTREGA'
           AND ($1::bool OR p.solicitante_id = $2)",
    )
    .bind(todas)
    .bind(user.id)
    .fetch_one(&app.db)
    .await?;

    let custodias_activas: i64 = if todas {
        sqlx::query_scalar("SELECT count(*) FROM custodias WHERE fin IS NULL")
            .fetch_one(&app.db)
            .await?
    } else {
        sqlx::query_scalar("SELECT count(*) FROM custodias WHERE fin IS NULL AND usuario_id = $1")
            .bind(user.id)
            .fetch_one(&app.db)
            .await?
    };

    let incidencias: i64 =
        sqlx::query_scalar("SELECT count(*) FROM incidencias WHERE estado = 'ABIERTA'")
            .fetch_one(&app.db)
            .await?;

    let no_localizados: i64 =
        sqlx::query_scalar("SELECT count(*) FROM expedientes WHERE estado = 'NO_LOCALIZADO'")
            .fetch_one(&app.db)
            .await?;

    let devoluciones_pendientes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM devoluciones WHERE estado = 'SOLICITADA'")
            .fetch_one(&app.db)
            .await?;

    Ok(Json(json!({
        "pedidos_pendientes": pedidos_pendientes,
        "pedidos_por_preparar": pedidos_por_preparar,
        "en_entrega": en_entrega,
        "custodias_activas": custodias_activas,
        "incidencias": incidencias,
        "no_localizados": no_localizados,
        "devoluciones_pendientes": devoluciones_pendientes,
    })))
}

pub async fn listar_pedidos(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pedidos:leer")?;
    let todas = user.puede("pedidos:preparar");

    let filas = sqlx::query_as::<_, (Uuid, Uuid, String, DateTime<Utc>, Option<DateTime<Utc>>, i64, i64, i64, i64, i64)>(
        "SELECT p.id, p.solicitante_id, u.nombre, p.creado_en, p.cancelado_en,
                count(i.id),
                count(i.id) FILTER (WHERE i.estado = 'ENTREGADO'),
                count(i.id) FILTER (WHERE i.estado = 'DESCARTADO'),
                count(i.id) FILTER (WHERE i.estado = 'CANCELADO'),
                count(i.id) FILTER (WHERE i.estado IN ('RESERVADO','PREPARADO','EN_ENTREGA'))
         FROM pedidos p
         JOIN usuarios u ON u.id = p.solicitante_id
         LEFT JOIN pedido_items i ON i.pedido_id = p.id
         WHERE ($1::bool OR p.solicitante_id = $2)
         GROUP BY p.id, u.nombre
         ORDER BY p.creado_en DESC
         LIMIT 200",
    )
    .bind(todas)
    .bind(user.id)
    .fetch_all(&app.db)
    .await?;

    let pedidos: Vec<Value> = filas
        .into_iter()
        .map(|(id, solicitante_id, solicitante_nombre, creado_en, cancelado_en, total, entregados, descartados, cancelados, en_proceso)| {
            let resueltos = entregados + descartados + cancelados;
            let estado = if cancelado_en.is_some() {
                "CANCELADO"
            } else if total > 0 && resueltos == total {
                "COMPLETADO"
            } else if resueltos > 0 {
                "PARCIAL"
            } else if en_proceso > 0 {
                "EN_PROCESO"
            } else {
                "PENDIENTE"
            };
            json!({
                "id": id,
                "solicitante_id": solicitante_id,
                "solicitante_nombre": solicitante_nombre,
                "creado_en": creado_en,
                "cancelado_en": cancelado_en,
                "estado": estado,
                "total": total,
                "entregados": entregados,
                "descartados": descartados,
                "cancelados": cancelados,
            })
        })
        .collect();

    Ok(Json(json!({ "pedidos": pedidos })))
}

pub async fn custodias(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("expedientes:leer")?;

    let filas = sqlx::query_as::<_, (Uuid, String, String, String, DateTime<Utc>)>(
        "SELECT e.id, e.numero, e.descripcion, e.estado, c.inicio
         FROM custodias c
         JOIN expedientes e ON e.id = c.expediente_id
         WHERE c.usuario_id = $1 AND c.fin IS NULL
         ORDER BY c.inicio DESC",
    )
    .bind(user.id)
    .fetch_all(&app.db)
    .await?;

    let custodias: Vec<Value> = filas
        .into_iter()
        .map(|(expediente_id, numero, descripcion, estado, desde)| {
            json!({
                "expediente_id": expediente_id,
                "numero": numero,
                "descripcion": descripcion,
                "estado": estado,
                "desde": desde,
            })
        })
        .collect();

    Ok(Json(json!({ "custodias": custodias })))
}

pub async fn buscar_expedientes(
    State(app): State<AppState>,
    user: AuthUser,
    Query(input): Query<BusquedaExpedientes>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("expedientes:leer")?;

    let patron = format!("%{}%", input.buscar.unwrap_or_default().trim());
    let limite = input.limite.unwrap_or(100).clamp(1, 500);
    let estado = input.estado;

    let filas = sqlx::query_as::<_, (Uuid, String, String, String, Option<String>, i64)>(
        "SELECT e.id, e.numero, e.descripcion, e.estado, u.nombre, e.version
         FROM expedientes e
         LEFT JOIN usuarios u ON u.id = e.custodio_id
         WHERE (e.numero ILIKE $1 OR e.descripcion ILIKE $1)
           AND ($2::text IS NULL OR e.estado = $2)
         ORDER BY e.numero
         LIMIT $3",
    )
    .bind(&patron)
    .bind(&estado)
    .bind(limite)
    .fetch_all(&app.db)
    .await?;

    let expedientes: Vec<Value> = filas
        .into_iter()
        .map(|(id, numero, descripcion, estado, custodio, version)| {
            json!({
                "id": id,
                "numero": numero,
                "descripcion": descripcion,
                "estado": estado,
                "custodio": custodio,
                "version": version,
            })
        })
        .collect();

    Ok(Json(json!({ "expedientes": expedientes })))
}

pub async fn historial(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("expedientes:leer")?;

    let cabecera = sqlx::query_as::<_, (String, String, Option<String>, i64)>(
        "SELECT e.numero, e.estado, u.nombre, e.version
         FROM expedientes e
         LEFT JOIN usuarios u ON u.id = e.custodio_id
         WHERE e.id = $1",
    )
    .bind(id)
    .fetch_optional(&app.db)
    .await?;

    let Some((numero, estado, custodio, version)) = cabecera else {
        return Err(ApiError::no_encontrado("Expediente no encontrado"));
    };

    let filas = sqlx::query_as::<_, (String, String, Option<String>, DateTime<Utc>)>(
        "SELECT m.tipo, u.nombre, m.detalle, m.creado_en
         FROM movimientos m
         JOIN usuarios u ON u.id = m.actor_id
         WHERE m.expediente_id = $1
         ORDER BY m.creado_en DESC
         LIMIT 200",
    )
    .bind(id)
    .fetch_all(&app.db)
    .await?;

    let movimientos: Vec<Value> = filas
        .into_iter()
        .map(|(tipo, actor, detalle, creado_en)| {
            json!({ "tipo": tipo, "actor": actor, "detalle": detalle, "creado_en": creado_en })
        })
        .collect();

    Ok(Json(json!({
        "expediente": {
            "id": id,
            "numero": numero,
            "estado": estado,
            "custodio": custodio,
            "version": version,
        },
        "movimientos": movimientos,
    })))
}

pub async fn entregas_pendientes(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("entregas:confirmar")?;

    let filas = sqlx::query_as::<_, (Uuid, String, String, DateTime<Utc>, String)>(
        "SELECT t.id, e.numero, e.descripcion, t.creada_en, u.nombre
         FROM entregas t
         JOIN pedido_items i ON i.id = t.pedido_item_id
         JOIN expedientes e ON e.id = i.expediente_id
         JOIN usuarios u ON u.id = t.entregador_id
         WHERE t.estado = 'DESPACHADA' AND t.receptor_id = $1
         ORDER BY t.creada_en",
    )
    .bind(user.id)
    .fetch_all(&app.db)
    .await?;

    let entregas: Vec<Value> = filas
        .into_iter()
        .map(|(id, numero, descripcion, creada_en, entregador)| {
            json!({
                "id": id,
                "numero": numero,
                "descripcion": descripcion,
                "creada_en": creada_en,
                "entregador": entregador,
            })
        })
        .collect();

    Ok(Json(json!({ "entregas": entregas })))
}
