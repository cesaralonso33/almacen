use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use chrono::{DateTime, Utc};
use rust_xlsxwriter::{Format, Workbook};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::{api::entrada::ApiQuery, auth::AuthUser, error::ApiError, AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusquedaExpedientes {
    pub buscar: Option<String>,
    pub estado: Option<String>,
    pub unidad: Option<String>,
    pub anio: Option<i16>,
    pub custodio: Option<String>,
    pub pagina: Option<i64>,
    pub tamano: Option<i64>,
}

pub async fn dashboard(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    let todas = user.puede("pedidos:preparar");

    let (
        pedidos_pendientes,
        pedidos_por_preparar,
        en_entrega,
        custodias_activas,
        incidencias,
        no_localizados,
        devoluciones_pendientes,
    ): (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM pedidos p
             WHERE p.cancelado_en IS NULL
               AND ($1::bool OR p.solicitante_id = $2)
               AND NOT EXISTS (
                   SELECT 1 FROM pedido_items i
                   WHERE i.pedido_id = p.id AND i.estado <> 'PENDIENTE'
               )),
            (SELECT count(*) FROM pedido_items i
             JOIN pedidos p ON p.id = i.pedido_id
             WHERE i.estado = 'PENDIENTE' AND p.cancelado_en IS NULL
               AND ($1::bool OR p.solicitante_id = $2)),
            (SELECT count(*) FROM pedido_items i
             JOIN pedidos p ON p.id = i.pedido_id
             WHERE i.estado = 'EN_ENTREGA'
               AND ($1::bool OR p.solicitante_id = $2)),
            (SELECT count(*) FROM custodias
             WHERE fin IS NULL AND ($1::bool OR usuario_id = $2)),
            (SELECT count(*) FROM incidencias WHERE estado = 'ABIERTA'),
            (SELECT count(*) FROM expedientes WHERE estado = 'NO_LOCALIZADO'),
            (SELECT count(*) FROM devoluciones WHERE estado = 'SOLICITADA')",
    )
    .bind(todas)
    .bind(user.id)
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
    ApiQuery(input): ApiQuery<BusquedaExpedientes>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("expedientes:leer")?;

    let patron = format!("%{}%", input.buscar.as_deref().unwrap_or_default().trim());
    let estado = input
        .estado
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string);
    let unidad = input
        .unidad
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string);
    let custodio = input
        .custodio
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(|valor| format!("%{valor}%"));

    let pagina = input.pagina.unwrap_or(1).max(1);
    let tamano = input.tamano.unwrap_or(25).clamp(1, 100);
    let offset = (pagina - 1) * tamano;

    let total: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM expedientes e
         LEFT JOIN usuarios u ON u.id = e.custodio_id
         WHERE (e.numero ILIKE $1 OR e.descripcion ILIKE $1)
           AND ($2::text IS NULL OR e.estado = $2)
           AND ($3::text IS NULL OR e.unidad = $3)
           AND ($4::smallint IS NULL OR e.anio = $4)
           AND ($5::text IS NULL OR u.nombre ILIKE $5)",
    )
    .bind(&patron)
    .bind(&estado)
    .bind(&unidad)
    .bind(input.anio)
    .bind(&custodio)
    .fetch_one(&app.db)
    .await?;

    let filas = sqlx::query_as::<_, (Uuid, String, String, String, Option<String>, i64, Option<String>, Option<i16>)>(
        "SELECT e.id, e.numero, e.descripcion, e.estado, u.nombre, e.version, e.unidad, e.anio
         FROM expedientes e
         LEFT JOIN usuarios u ON u.id = e.custodio_id
         WHERE (e.numero ILIKE $1 OR e.descripcion ILIKE $1)
           AND ($2::text IS NULL OR e.estado = $2)
           AND ($3::text IS NULL OR e.unidad = $3)
           AND ($4::smallint IS NULL OR e.anio = $4)
           AND ($5::text IS NULL OR u.nombre ILIKE $5)
         ORDER BY e.numero
         LIMIT $6 OFFSET $7",
    )
    .bind(&patron)
    .bind(&estado)
    .bind(&unidad)
    .bind(input.anio)
    .bind(&custodio)
    .bind(tamano)
    .bind(offset)
    .fetch_all(&app.db)
    .await?;

    // Resumen por estado conservando los demás filtros (sin el de estado).
    let resumen = sqlx::query_as::<_, (String, i64)>(
        "SELECT e.estado, count(*)
         FROM expedientes e
         LEFT JOIN usuarios u ON u.id = e.custodio_id
         WHERE (e.numero ILIKE $1 OR e.descripcion ILIKE $1)
           AND ($2::text IS NULL OR e.unidad = $2)
           AND ($3::smallint IS NULL OR e.anio = $3)
           AND ($4::text IS NULL OR u.nombre ILIKE $4)
         GROUP BY e.estado",
    )
    .bind(&patron)
    .bind(&unidad)
    .bind(input.anio)
    .bind(&custodio)
    .fetch_all(&app.db)
    .await?;

    let mut por_estado = Map::new();
    for (estado_fila, cantidad) in resumen {
        por_estado.insert(estado_fila, json!(cantidad));
    }

    let unidades: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT unidad FROM expedientes WHERE unidad IS NOT NULL ORDER BY unidad LIMIT 200",
    )
    .fetch_all(&app.db)
    .await?;

    let anios: Vec<i16> = sqlx::query_scalar(
        "SELECT DISTINCT anio FROM expedientes WHERE anio IS NOT NULL ORDER BY anio DESC",
    )
    .fetch_all(&app.db)
    .await?;

    let expedientes: Vec<Value> = filas
        .into_iter()
        .map(|(id, numero, descripcion, estado, custodio, version, unidad, anio)| {
            json!({
                "id": id,
                "numero": numero,
                "descripcion": descripcion,
                "estado": estado,
                "custodio": custodio,
                "version": version,
                "unidad": unidad,
                "anio": anio,
            })
        })
        .collect();

    Ok(Json(json!({
        "expedientes": expedientes,
        "total": total,
        "pagina": pagina,
        "tamano": tamano,
        "por_estado": Value::Object(por_estado),
        "unidades": unidades,
        "anios": anios,
    })))
}

pub async fn exportar_expedientes(
    State(app): State<AppState>,
    user: AuthUser,
    ApiQuery(input): ApiQuery<BusquedaExpedientes>,
) -> Result<Response, ApiError> {
    user.exigir("expedientes:exportar")?;

    let patron = format!("%{}%", input.buscar.as_deref().unwrap_or_default().trim());
    let estado = input
        .estado
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    let unidad = input
        .unidad
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    let custodio = input
        .custodio
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(|valor| format!("%{valor}%"));

    let filas = sqlx::query_as::<
        _,
        (
            String,
            Option<i16>,
            Option<String>,
            String,
            String,
            Option<String>,
            i64,
        ),
    >(
        "SELECT e.numero, e.anio, e.unidad, e.descripcion, e.estado, u.nombre, e.version
         FROM expedientes e
         LEFT JOIN usuarios u ON u.id = e.custodio_id
         WHERE (e.numero ILIKE $1 OR e.descripcion ILIKE $1)
           AND ($2::text IS NULL OR e.estado = $2)
           AND ($3::text IS NULL OR e.unidad = $3)
           AND ($4::smallint IS NULL OR e.anio = $4)
           AND ($5::text IS NULL OR u.nombre ILIKE $5)
         ORDER BY e.numero
         LIMIT 100000",
    )
    .bind(&patron)
    .bind(estado)
    .bind(unidad)
    .bind(input.anio)
    .bind(&custodio)
    .fetch_all(&app.db)
    .await?;

    let mut libro = Workbook::new();
    let hoja = libro.add_worksheet();
    hoja.set_name("Expedientes").map_err(error_excel)?;
    let encabezado = Format::new().set_bold();
    let columnas = [
        "NÚMERO",
        "AÑO",
        "UNIDAD / ÓRGANO",
        "DESCRIPCIÓN",
        "ESTADO",
        "CUSTODIO",
        "VERSIÓN",
    ];
    for (indice, titulo) in columnas.iter().enumerate() {
        hoja.write_string_with_format(0, indice as u16, *titulo, &encabezado)
            .map_err(error_excel)?;
        hoja.set_column_width(indice as u16, if indice == 3 { 48 } else { 18 })
            .map_err(error_excel)?;
    }

    for (fila, (numero, anio, unidad, descripcion, estado, custodio, version)) in
        filas.iter().enumerate()
    {
        let renglon = fila as u32 + 1;
        hoja.write_string(renglon, 0, numero.as_str())
            .map_err(error_excel)?;
        if let Some(anio) = anio {
            hoja.write_number(renglon, 1, *anio as f64)
                .map_err(error_excel)?;
        }
        hoja.write_string(renglon, 2, unidad.as_deref().unwrap_or(""))
            .map_err(error_excel)?;
        hoja.write_string(renglon, 3, descripcion.as_str())
            .map_err(error_excel)?;
        hoja.write_string(renglon, 4, estado.as_str())
            .map_err(error_excel)?;
        hoja.write_string(renglon, 5, custodio.as_deref().unwrap_or(""))
            .map_err(error_excel)?;
        hoja.write_number(renglon, 6, *version as f64)
            .map_err(error_excel)?;
    }

    let bytes = libro.save_to_buffer().map_err(error_excel)?;
    let nombre = format!("expedientes-{}.xlsx", Utc::now().format("%Y%m%d-%H%M%S"));
    let disposicion = format!("attachment; filename=\"{nombre}\"");
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static(
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ),
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposicion)
                .map_err(|error| ApiError::interno(error.to_string()))?,
        )
        .body(Body::from(bytes))
        .map_err(|error| ApiError::interno(error.to_string()))
}

fn error_excel(error: rust_xlsxwriter::XlsxError) -> ApiError {
    ApiError::interno(format!("No se pudo generar el Excel: {error}"))
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
