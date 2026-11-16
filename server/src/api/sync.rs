use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{api::entrada::ApiJson, audit, auth::AuthUser, error::ApiError, sync, AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NuevoDispositivo {
    pub nombre: String,
    pub plataforma: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperacionEntrante {
    pub clave: Uuid,
    pub tipo: String,
    pub payload: Value,
    pub version_base: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushInput {
    pub dispositivo_id: Uuid,
    pub operaciones: Vec<OperacionEntrante>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullQuery {
    pub dispositivo_id: Uuid,
    pub cursor: Option<i64>,
    pub limite: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotQuery {
    pub dispositivo_id: Uuid,
}

pub async fn registrar_dispositivo(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<NuevoDispositivo>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("sync:push")?;

    let nombre = input.nombre.trim();
    if nombre.is_empty() {
        return Err(ApiError::inprocesable("El nombre del dispositivo es obligatorio"));
    }

    let mut tx = app.db.begin().await?;
    let dispositivo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO dispositivos(usuario_id, nombre, plataforma) VALUES ($1,$2,$3) RETURNING id",
    )
    .bind(user.id)
    .bind(nombre)
    .bind(input.plataforma.as_deref())
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO sync_devices(dispositivo_id) VALUES ($1)")
        .bind(dispositivo_id)
        .execute(&mut *tx)
        .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "sync.dispositivo.registrar",
        "dispositivos",
        dispositivo_id,
        "EXITO",
        json!({ "nombre": nombre }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({ "dispositivo_id": dispositivo_id })))
}

pub async fn push(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<PushInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("sync:push")?;

    if input.operaciones.is_empty() || input.operaciones.len() > 50 {
        return Err(ApiError::solicitud_invalida(
            "Envíe entre 1 y 50 operaciones por lote",
        ));
    }

    validar_dispositivo(&app.db, &user, input.dispositivo_id).await?;

    let mut resultados = Vec::with_capacity(input.operaciones.len());
    for operacion in input.operaciones {
        resultados.push(procesar(&app, &user, input.dispositivo_id, operacion).await?);
    }

    Ok(Json(json!({ "resultados": resultados })))
}

pub async fn pull(
    State(app): State<AppState>,
    user: AuthUser,
    Query(input): Query<PullQuery>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("sync:pull")?;
    validar_dispositivo(&app.db, &user, input.dispositivo_id).await?;

    let maximo: i64 = sqlx::query_scalar("SELECT COALESCE(max(secuencia),0) FROM sync_events")
        .fetch_one(&app.db)
        .await?;

    let cursor = input.cursor.unwrap_or(0);
    if cursor < 0 || cursor > maximo {
        return Ok(Json(json!({
            "eventos": [],
            "siguiente_cursor": maximo,
            "hay_mas": false,
            "reiniciar": true,
        })));
    }

    let limite = input.limite.unwrap_or(200).clamp(1, 500);

    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT r.clave FROM usuario_roles ur JOIN roles r ON r.id = ur.rol_id
         WHERE ur.usuario_id = $1 AND ur.revocado_en IS NULL",
    )
    .bind(user.id)
    .fetch_all(&app.db)
    .await?;

    let filas = sqlx::query_as::<_, (i64, String, Uuid, String, i64, bool, Value)>(
        "SELECT secuencia, entidad, entidad_id, operacion, version, tombstone, payload
         FROM sync_events
         WHERE secuencia > $1
           AND (visibilidad = 'GLOBAL'
                OR (visibilidad = 'USUARIO' AND usuario_id = $2)
                OR (visibilidad = 'ROL' AND rol_clave = ANY($3)))
         ORDER BY secuencia
         LIMIT $4",
    )
    .bind(cursor)
    .bind(user.id)
    .bind(&roles)
    .bind(limite)
    .fetch_all(&app.db)
    .await?;

    let hay_mas = filas.len() as i64 == limite;
    let siguiente_cursor = filas.last().map(|f| f.0).unwrap_or(cursor);

    let eventos: Vec<Value> = filas
        .into_iter()
        .map(|(secuencia, entidad, entidad_id, operacion, version, tombstone, payload)| {
            json!({
                "secuencia": secuencia,
                "entidad": entidad,
                "entidad_id": entidad_id,
                "operacion": operacion,
                "version": version,
                "tombstone": tombstone,
                "payload": payload,
            })
        })
        .collect();

    sqlx::query(
        "UPDATE sync_devices
         SET ultima_secuencia = GREATEST(ultima_secuencia, $2), ultima_sync = now()
         WHERE dispositivo_id = $1",
    )
    .bind(input.dispositivo_id)
    .bind(siguiente_cursor)
    .execute(&app.db)
    .await?;

    Ok(Json(json!({
        "eventos": eventos,
        "siguiente_cursor": siguiente_cursor,
        "hay_mas": hay_mas,
    })))
}

pub async fn snapshot(
    State(app): State<AppState>,
    user: AuthUser,
    Query(input): Query<SnapshotQuery>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("sync:pull")?;
    validar_dispositivo(&app.db, &user, input.dispositivo_id).await?;

    let expedientes = sqlx::query_as::<_, (Uuid, String, String, String, Option<Uuid>, Option<Uuid>, i64, DateTime<Utc>)>(
        "SELECT id, numero, descripcion, estado, ubicacion_id, custodio_id, version, actualizado_en
         FROM expedientes
         ORDER BY numero",
    )
    .fetch_all(&app.db)
    .await?;

    let expedientes: Vec<Value> = expedientes
        .into_iter()
        .map(|(id, numero, descripcion, estado, ubicacion_id, custodio_id, version, actualizado_en)| {
            json!({
                "id": id,
                "numero": numero,
                "descripcion": descripcion,
                "estado": estado,
                "ubicacion_id": ubicacion_id,
                "custodio_id": custodio_id,
                "version": version,
                "actualizado_en": actualizado_en,
            })
        })
        .collect();

    let pedidos_propios: Vec<(Uuid, Option<DateTime<Utc>>, i64)> = sqlx::query_as(
        "SELECT id, cancelado_en, version FROM pedidos WHERE solicitante_id = $1 ORDER BY creado_en",
    )
    .bind(user.id)
    .fetch_all(&app.db)
    .await?;

    let mut pedidos = Vec::with_capacity(pedidos_propios.len());
    for (id, cancelado_en, version) in pedidos_propios {
        let items = items_del_pedido(&app.db, id).await?;
        let estado = estado_derivado(cancelado_en.is_some(), &items);
        pedidos.push(json!({
            "id": id,
            "solicitante_id": user.id,
            "estado": estado,
            "cancelado_en": cancelado_en,
            "version": version,
            "items": items,
        }));
    }

    let cursor: i64 = sqlx::query_scalar("SELECT COALESCE(max(secuencia),0) FROM sync_events")
        .fetch_one(&app.db)
        .await?;

    Ok(Json(json!({
        "expedientes": expedientes,
        "pedidos": pedidos,
        "cursor": cursor,
    })))
}

// ---------------------------------------------------------------------------
// Procesamiento de operaciones
// ---------------------------------------------------------------------------

async fn validar_dispositivo(
    db: &PgPool,
    user: &AuthUser,
    dispositivo_id: Uuid,
) -> Result<(), ApiError> {
    let propio: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM dispositivos
         WHERE id = $1 AND usuario_id = $2 AND revocado_en IS NULL)",
    )
    .bind(dispositivo_id)
    .bind(user.id)
    .fetch_one(db)
    .await?;

    if !propio {
        return Err(ApiError::prohibido("El dispositivo no pertenece al usuario autenticado"));
    }
    Ok(())
}

fn hash_payload(payload: &Value) -> String {
    hex::encode(Sha256::digest(payload.to_string().as_bytes()))
}

async fn procesar(
    app: &AppState,
    user: &AuthUser,
    dispositivo_id: Uuid,
    operacion: OperacionEntrante,
) -> Result<Value, ApiError> {
    let existente = sqlx::query_as::<_, (String, Option<Value>)>(
        "SELECT estado, respuesta FROM operaciones_idempotentes
         WHERE clave = $1 AND dispositivo_id = $2",
    )
    .bind(operacion.clave)
    .bind(dispositivo_id)
    .fetch_optional(&app.db)
    .await?;

    if let Some((estado, respuesta)) = existente {
        let respuesta = respuesta.unwrap_or_else(|| json!({}));
        return Ok(resultado_json(&operacion.clave, &estado, respuesta));
    }

    let insertadas = sqlx::query(
        "INSERT INTO operaciones_idempotentes(clave, dispositivo_id, tipo, payload_hash)
         VALUES ($1,$2,$3,$4)
         ON CONFLICT DO NOTHING",
    )
    .bind(operacion.clave)
    .bind(dispositivo_id)
    .bind(&operacion.tipo)
    .bind(hash_payload(&operacion.payload))
    .execute(&app.db)
    .await?
    .rows_affected();

    if insertadas == 0 {
        // Otro proceso tomó la misma clave: no se ejecuta dos veces.
        return Ok(json!({
            "clave": operacion.clave,
            "estado": "RECHAZADA",
            "resultado": {
                "code": "EN_PROCESO",
                "message": "La operación ya se está procesando; reintente más tarde"
            }
        }));
    }

    let ejecucion = ejecutar(app, user, &operacion).await;

    let (estado, respuesta) = match ejecucion {
        Ok(valor) => ("COMPLETADA", valor),
        Err(error) => (
            "RECHAZADA",
            json!({ "code": error.codigo, "message": error.mensaje }),
        ),
    };

    sqlx::query(
        "UPDATE operaciones_idempotentes
         SET estado = $3, respuesta = $4, completada_en = now()
         WHERE clave = $1 AND dispositivo_id = $2",
    )
    .bind(operacion.clave)
    .bind(dispositivo_id)
    .bind(estado)
    .bind(&respuesta)
    .execute(&app.db)
    .await?;

    Ok(resultado_json(&operacion.clave, estado, respuesta))
}

fn resultado_json(clave: &Uuid, estado: &str, respuesta: Value) -> Value {
    let estado_cliente = match estado {
        "COMPLETADA" => "ACEPTADA",
        _ => "RECHAZADA",
    };
    json!({ "clave": clave, "estado": estado_cliente, "resultado": respuesta })
}

async fn ejecutar(
    app: &AppState,
    user: &AuthUser,
    operacion: &OperacionEntrante,
) -> Result<Value, ApiError> {
    match operacion.tipo.as_str() {
        "CREAR_PEDIDO" => crear_pedido_offline(app, user, operacion).await,
        otro => Err(ApiError::inprocesable(format!(
            "La operación {otro} no se permite sin conexión al servidor"
        ))),
    }
}

async fn crear_pedido_offline(
    app: &AppState,
    user: &AuthUser,
    operacion: &OperacionEntrante,
) -> Result<Value, ApiError> {
    user.exigir("pedidos:crear")?;

    let expedientes: Vec<Uuid> = operacion
        .payload
        .get("expedientes")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| ApiError::solicitud_invalida("payload.expedientes debe ser una lista de UUID"))?
        .ok_or_else(|| ApiError::solicitud_invalida("payload.expedientes es obligatorio"))?;

    if expedientes.is_empty() || expedientes.len() > 100 {
        return Err(ApiError::solicitud_invalida(
            "Debe solicitar entre 1 y 100 expedientes",
        ));
    }

    let mut ids = expedientes.clone();
    ids.sort();
    ids.dedup();
    if ids.len() != expedientes.len() {
        return Err(ApiError::solicitud_invalida("Expedientes duplicados en la operación"));
    }

    let mut tx = app.db.begin().await?;

    let existentes: i64 = sqlx::query_scalar("SELECT count(*) FROM expedientes WHERE id = ANY($1)")
        .bind(&ids)
        .fetch_one(&mut *tx)
        .await?;
    if existentes as usize != ids.len() {
        return Err(ApiError::inprocesable("Uno o más expedientes no existen"));
    }

    let pedido_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pedidos(solicitante_id, version) VALUES ($1,1) RETURNING id",
    )
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;

    for expediente in &ids {
        sqlx::query("INSERT INTO pedido_items(pedido_id, expediente_id) VALUES ($1,$2)")
            .bind(pedido_id)
            .bind(expediente)
            .execute(&mut *tx)
            .await?;
    }

    sync::a_usuario_y_almacen(
        &mut tx,
        "pedido",
        pedido_id,
        "CREAR",
        user.id,
        1,
        json!({
            "id": pedido_id,
            "solicitante_id": user.id,
            "estado": "PENDIENTE",
            "version": 1,
        }),
    )
    .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "sync.pedidos.crear",
        "pedidos",
        pedido_id,
        "EXITO",
        json!({ "expedientes": ids, "clave": operacion.clave }),
    )
    .await?;

    tx.commit().await?;

    Ok(json!({ "pedido_id": pedido_id, "version": 1 }))
}

async fn items_del_pedido(db: &PgPool, pedido_id: Uuid) -> Result<Vec<Value>, ApiError> {
    let filas: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT id, expediente_id, estado FROM pedido_items WHERE pedido_id = $1 ORDER BY expediente_id",
    )
    .bind(pedido_id)
    .fetch_all(db)
    .await?;

    Ok(filas
        .into_iter()
        .map(|(id, expediente_id, estado)| {
            json!({ "id": id, "pedido_id": pedido_id, "expediente_id": expediente_id, "estado": estado })
        })
        .collect())
}

fn estado_derivado(cancelado: bool, items: &[Value]) -> &'static str {
    if cancelado {
        return "CANCELADO";
    }
    let total = items.len();
    let resueltos = items
        .iter()
        .filter(|i| matches!(i["estado"].as_str(), Some("ENTREGADO" | "DESCARTADO" | "CANCELADO")))
        .count();
    if total > 0 && resueltos == total {
        "COMPLETADO"
    } else if resueltos > 0 {
        "PARCIAL"
    } else if items.iter().any(|i| {
        matches!(
            i["estado"].as_str(),
            Some("RESERVADO" | "PREPARADO" | "EN_ENTREGA")
        )
    }) {
        "EN_PROCESO"
    } else {
        "PENDIENTE"
    }
}
