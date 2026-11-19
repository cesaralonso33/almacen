use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{NaiveDate, Utc};
use rust_xlsxwriter::{Format, Workbook};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    api::entrada::{ApiJson, ApiQuery},
    audit,
    auth::AuthUser,
    error::ApiError,
    AppState,
};

const CAMPOS_DOCUMENTO: [&str; 5] = ["CURP", "RFC", "REGISTRO", "IDENTIFICACION", "ESTADO_CUENTA"];
const LIMITE_DOCUMENTO: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CampoExtra {
    pub nombre: String,
    pub valor: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DarfeInput {
    pub numero_darfe: String,
    pub fecha: NaiveDate,
    pub nombre: String,
    #[serde(default)]
    pub expediente: String,
    #[serde(default)]
    pub registro: String,
    #[serde(default)]
    pub estado_cuenta: String,
    #[serde(default)]
    pub banco: String,
    #[serde(default)]
    pub identificacion: String,
    #[serde(default)]
    pub curp: String,
    #[serde(default)]
    pub rfc: String,
    pub estatus: String,
    #[serde(default)]
    pub campos_extra: Vec<CampoExtra>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentoInput {
    pub campo: String,
    pub nombre: String,
    #[serde(default = "mime_predeterminado")]
    pub mime: String,
    pub contenido_base64: String,
}

fn mime_predeterminado() -> String {
    "application/octet-stream".to_string()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DarfeFiltro {
    pub buscar: Option<String>,
    pub fecha_desde: Option<NaiveDate>,
    pub fecha_hasta: Option<NaiveDate>,
    pub estatus: Option<String>,
    pub banco: Option<String>,
    pub pagina: Option<i64>,
    pub tamano: Option<i64>,
}

struct DatosDarfe {
    numero_darfe: String,
    fecha: NaiveDate,
    nombre: String,
    expediente: String,
    registro: String,
    estado_cuenta: String,
    banco: String,
    identificacion: String,
    curp: String,
    rfc: String,
    estatus: String,
    campos_extra: Vec<CampoExtra>,
}

pub async fn listar(
    State(app): State<AppState>,
    user: AuthUser,
    ApiQuery(filtro): ApiQuery<DarfeFiltro>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:leer")?;
    let (total, registros) = consultar_lista(&app, &filtro, true).await?;
    let pagina = filtro.pagina.unwrap_or(1).max(1);
    let tamano = filtro.tamano.unwrap_or(25).clamp(1, 100);
    Ok(Json(json!({
        "darfes": registros,
        "total": total,
        "pagina": pagina,
        "tamano": tamano,
    })))
}

pub async fn detalle(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:leer")?;
    Ok(Json(consultar(&app, id).await?))
}

pub async fn crear(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<DarfeInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:crear")?;
    let datos = validar(input)?;
    let extra = serde_json::to_value(&datos.campos_extra)
        .map_err(|error| ApiError::interno(error.to_string()))?;
    let mut tx = app.db.begin().await?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO darfes
            (numero_darfe, fecha, nombre, expediente, registro, estado_cuenta, banco,
             identificacion, curp, rfc, estatus, campos_extra, creado_por, actualizado_por)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$13)
         RETURNING id",
    )
    .bind(&datos.numero_darfe)
    .bind(datos.fecha)
    .bind(&datos.nombre)
    .bind(&datos.expediente)
    .bind(&datos.registro)
    .bind(&datos.estado_cuenta)
    .bind(&datos.banco)
    .bind(&datos.identificacion)
    .bind(&datos.curp)
    .bind(&datos.rfc)
    .bind(&datos.estatus)
    .bind(&extra)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(duplicado)?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "darfes.crear",
        "darfes",
        id,
        "EXITO",
        json!({ "numero_darfe": datos.numero_darfe, "estatus": datos.estatus }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(consultar(&app, id).await?))
}

pub async fn actualizar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<DarfeInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:editar")?;
    let datos = validar(input)?;
    let extra = serde_json::to_value(&datos.campos_extra)
        .map_err(|error| ApiError::interno(error.to_string()))?;

    let existe: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM darfes WHERE id=$1 AND eliminado_en IS NULL)",
    )
    .bind(id)
    .fetch_one(&app.db)
    .await?;
    if !existe {
        return Err(ApiError::no_encontrado("DARFE no encontrado"));
    }

    let mut tx = app.db.begin().await?;
    sqlx::query(
        "UPDATE darfes
         SET numero_darfe=$2, fecha=$3, nombre=$4, expediente=$5, registro=$6,
             estado_cuenta=$7, banco=$8, identificacion=$9, curp=$10, rfc=$11,
             estatus=$12, campos_extra=$13, actualizado_por=$14, actualizado_en=now()
         WHERE id=$1",
    )
    .bind(id)
    .bind(&datos.numero_darfe)
    .bind(datos.fecha)
    .bind(&datos.nombre)
    .bind(&datos.expediente)
    .bind(&datos.registro)
    .bind(&datos.estado_cuenta)
    .bind(&datos.banco)
    .bind(&datos.identificacion)
    .bind(&datos.curp)
    .bind(&datos.rfc)
    .bind(&datos.estatus)
    .bind(&extra)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .map_err(duplicado)?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "darfes.editar",
        "darfes",
        id,
        "EXITO",
        json!({ "numero_darfe": datos.numero_darfe, "estatus": datos.estatus }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(consultar(&app, id).await?))
}

pub async fn eliminar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:eliminar")?;
    let mut tx = app.db.begin().await?;
    let numero: Option<String> = sqlx::query_scalar(
        "UPDATE darfes SET eliminado_en=now(), actualizado_por=$2, actualizado_en=now()
         WHERE id=$1 AND eliminado_en IS NULL RETURNING numero_darfe",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(numero) = numero else {
        return Err(ApiError::no_encontrado("DARFE no encontrado"));
    };
    audit::registrar(
        &mut tx,
        Some(user.id),
        "darfes.eliminar",
        "darfes",
        id,
        "EXITO",
        json!({ "numero_darfe": numero }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn subir_documento(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<DocumentoInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:documentos")?;
    let campo = input.campo.trim().to_uppercase();
    if !CAMPOS_DOCUMENTO.contains(&campo.as_str()) {
        return Err(ApiError::inprocesable("Campo de documento no permitido"));
    }
    if input.nombre.trim().is_empty() || input.nombre.len() > 200 {
        return Err(ApiError::inprocesable("El nombre del documento es obligatorio"));
    }
    let contenido = STANDARD
        .decode(input.contenido_base64.as_bytes())
        .map_err(|_| ApiError::solicitud_invalida("El contenido del documento no es base64 válido"))?;
    if contenido.is_empty() || contenido.len() > LIMITE_DOCUMENTO {
        return Err(ApiError::inprocesable("El documento debe pesar entre 1 byte y 8 MiB"));
    }

    let existe: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM darfes WHERE id=$1 AND eliminado_en IS NULL)",
    )
    .bind(id)
    .fetch_one(&app.db)
    .await?;
    if !existe {
        return Err(ApiError::no_encontrado("DARFE no encontrado"));
    }

    let mut tx = app.db.begin().await?;
    sqlx::query(
        "INSERT INTO darfe_documentos(darfe_id, campo, nombre, mime, tamano, contenido, creado_por)
         VALUES ($1,$2,$3,$4,$5,$6,$7)
         ON CONFLICT (darfe_id, campo) DO UPDATE SET
             nombre=excluded.nombre, mime=excluded.mime, tamano=excluded.tamano,
             contenido=excluded.contenido, creado_por=excluded.creado_por, creado_en=now()",
    )
    .bind(id)
    .bind(&campo)
    .bind(input.nombre.trim())
    .bind(input.mime.trim())
    .bind(contenido.len() as i32)
    .bind(&contenido)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    audit::registrar(
        &mut tx,
        Some(user.id),
        "darfes.documento.subir",
        "darfes",
        id,
        "EXITO",
        json!({ "campo": campo, "tamano": contenido.len() }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(json!({ "campo": campo, "tamano": contenido.len() })))
}

pub async fn documento(
    State(app): State<AppState>,
    user: AuthUser,
    Path((id, campo)): Path<(Uuid, String)>,
) -> Result<Response, ApiError> {
    user.exigir("darfes:leer")?;
    let campo = campo.trim().to_uppercase();
    let fila: Option<(Vec<u8>, String, String)> = sqlx::query_as(
        "SELECT d.contenido, d.nombre, d.mime
         FROM darfe_documentos d JOIN darfes r ON r.id=d.darfe_id
         WHERE d.darfe_id=$1 AND d.campo=$2 AND r.eliminado_en IS NULL",
    )
    .bind(id)
    .bind(&campo)
    .fetch_optional(&app.db)
    .await?;
    let Some((contenido, nombre, mime)) = fila else {
        return Err(ApiError::no_encontrado("Documento no encontrado"));
    };
    let disposicion = format!("inline; filename=\"{}\"", nombre.replace('"', ""));
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&mime).unwrap_or(HeaderValue::from_static("application/octet-stream")),
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposicion).map_err(|error| ApiError::interno(error.to_string()))?,
        )
        .body(Body::from(contenido))
        .map_err(|error| ApiError::interno(error.to_string()))
}

pub async fn eliminar_documento(
    State(app): State<AppState>,
    user: AuthUser,
    Path((id, campo)): Path<(Uuid, String)>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:documentos")?;
    let campo = campo.trim().to_uppercase();
    let mut tx = app.db.begin().await?;
    let eliminado: Option<String> = sqlx::query_scalar(
        "DELETE FROM darfe_documentos WHERE darfe_id=$1 AND campo=$2 RETURNING campo",
    )
    .bind(id)
    .bind(&campo)
    .fetch_optional(&mut *tx)
    .await?;
    if eliminado.is_none() {
        return Err(ApiError::no_encontrado("Documento no encontrado"));
    }
    audit::registrar(
        &mut tx,
        Some(user.id),
        "darfes.documento.eliminar",
        "darfes",
        id,
        "EXITO",
        json!({ "campo": campo }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn exportar(
    State(app): State<AppState>,
    user: AuthUser,
    ApiQuery(filtro): ApiQuery<DarfeFiltro>,
) -> Result<Response, ApiError> {
    user.exigir("darfes:exportar")?;
    let (_, registros) = consultar_lista(&app, &filtro, false).await?;

    let mut libro = Workbook::new();
    let hoja = libro.add_worksheet();
    hoja.set_name("DARFES").map_err(error_excel)?;
    let encabezado = Format::new().set_bold();
    let columnas = [
        "No. DARFE", "Fecha", "Nombre", "Expediente", "Registro", "Estado de cuenta",
        "Banco", "Identificación", "CURP", "RFC", "Estatus", "Campos adicionales",
    ];
    for (indice, titulo) in columnas.iter().enumerate() {
        hoja.write_string_with_format(0, indice as u16, *titulo, &encabezado)
            .map_err(error_excel)?;
        hoja.set_column_width(indice as u16, 18).map_err(error_excel)?;
    }

    for (fila, registro) in registros.iter().enumerate() {
        let renglon = fila as u32 + 1;
        hoja.write_string(renglon, 0, registro["numero_darfe"].as_str().unwrap_or(""))
            .map_err(error_excel)?;
        hoja.write_string(renglon, 1, registro["fecha"].as_str().unwrap_or(""))
            .map_err(error_excel)?;
        for (indice, clave) in [
            "nombre", "expediente", "registro", "estado_cuenta", "banco",
            "identificacion", "curp", "rfc", "estatus",
        ]
        .iter()
        .enumerate()
        {
            hoja.write_string(
                renglon,
                indice as u16 + 2,
                registro[*clave].as_str().unwrap_or(""),
            )
            .map_err(error_excel)?;
        }
        let extra = registro["campos_extra"].to_string();
        hoja.write_string(renglon, 11, extra).map_err(error_excel)?;
    }

    let bytes = libro.save_to_buffer().map_err(error_excel)?;
    let nombre = format!("DARFES-{}.xlsx", Utc::now().format("%Y%m%d-%H%M%S"));
    let disposicion = format!("attachment; filename=\"{nombre}\"");
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposicion).map_err(|error| ApiError::interno(error.to_string()))?,
        )
        .body(Body::from(bytes))
        .map_err(|error| ApiError::interno(error.to_string()))
}

pub async fn reportes(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:reportes")?;

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM darfes WHERE eliminado_en IS NULL")
        .fetch_one(&app.db)
        .await?;
    let por_estatus: Vec<(String, i64)> = sqlx::query_as(
        "SELECT estatus, count(*) FROM darfes WHERE eliminado_en IS NULL GROUP BY estatus",
    )
    .fetch_all(&app.db)
    .await?;
    let por_banco: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(NULLIF(trim(banco), ''), 'Sin banco'), count(*)
         FROM darfes WHERE eliminado_en IS NULL
         GROUP BY 1 ORDER BY count(*) DESC LIMIT 10",
    )
    .fetch_all(&app.db)
    .await?;
    let por_mes: Vec<(String, i64)> = sqlx::query_as(
        "SELECT to_char(date_trunc('month', fecha), 'YYYY-MM') AS periodo, count(*)
         FROM darfes WHERE eliminado_en IS NULL
         GROUP BY 1 ORDER BY 1 DESC LIMIT 12",
    )
    .fetch_all(&app.db)
    .await?;
    let documentos: Vec<(String, i64)> = sqlx::query_as(
        "SELECT d.campo, count(*) FROM darfe_documentos d
         JOIN darfes r ON r.id=d.darfe_id
         WHERE r.eliminado_en IS NULL
         GROUP BY d.campo ORDER BY d.campo",
    )
    .fetch_all(&app.db)
    .await?;
    let recientes: Vec<(Uuid, String, NaiveDate, String, String)> = sqlx::query_as(
        "SELECT id, numero_darfe, fecha, nombre, estatus
         FROM darfes WHERE eliminado_en IS NULL ORDER BY creado_en DESC LIMIT 5",
    )
    .fetch_all(&app.db)
    .await?;

    let mapa = |filas: Vec<(String, i64)>| {
        let mut mapa = serde_json::Map::new();
        for (clave, valor) in filas {
            mapa.insert(clave, json!(valor));
        }
        Value::Object(mapa)
    };
    let lista = |filas: Vec<(String, i64)>| {
        filas
            .into_iter()
            .map(|(clave, total)| json!({ "clave": clave, "total": total }))
            .collect::<Vec<_>>()
    };

    Ok(Json(json!({
        "total": total,
        "por_estatus": mapa(por_estatus),
        "por_banco": lista(por_banco),
        "por_mes": lista(por_mes),
        "documentos": mapa(documentos),
        "recientes": recientes.into_iter().map(|(id, numero, fecha, nombre, estatus)| json!({
            "id": id, "numero_darfe": numero, "fecha": fecha, "nombre": nombre, "estatus": estatus
        })).collect::<Vec<_>>(),
    })))
}

async fn consultar_lista(
    app: &AppState,
    filtro: &DarfeFiltro,
    paginar: bool,
) -> Result<(i64, Vec<Value>), ApiError> {
    let buscar = patron(&filtro.buscar);
    let banco = patron(&filtro.banco);
    let estatus = filtro.estatus.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let pagina = filtro.pagina.unwrap_or(1).max(1);
    let tamano = filtro.tamano.unwrap_or(25).clamp(1, 100);
    let (limite, offset) = if paginar {
        (tamano, (pagina - 1) * tamano)
    } else {
        (100_000_i64, 0_i64)
    };

    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM darfes
         WHERE eliminado_en IS NULL
           AND ($1::text IS NULL OR (numero_darfe ILIKE $1 OR nombre ILIKE $1 OR expediente ILIKE $1
                OR registro ILIKE $1 OR estado_cuenta ILIKE $1 OR banco ILIKE $1
                OR identificacion ILIKE $1 OR curp ILIKE $1 OR rfc ILIKE $1))
           AND ($2::date IS NULL OR fecha >= $2)
           AND ($3::date IS NULL OR fecha <= $3)
           AND ($4::text IS NULL OR estatus = $4)
           AND ($5::text IS NULL OR banco ILIKE $5)",
    )
    .bind(&buscar)
    .bind(filtro.fecha_desde)
    .bind(filtro.fecha_hasta)
    .bind(estatus)
    .bind(&banco)
    .fetch_one(&app.db)
    .await?;

    let filas: Vec<(Uuid, String, NaiveDate, String, String, String, String, String, String, String, String, String, Value, Vec<String>)> =
        sqlx::query_as(
            "SELECT d.id, d.numero_darfe, d.fecha, d.nombre, d.expediente, d.registro,
                    d.estado_cuenta, d.banco, d.identificacion, d.curp, d.rfc, d.estatus,
                    d.campos_extra,
                    ARRAY(SELECT campo FROM darfe_documentos WHERE darfe_id=d.id ORDER BY campo)
             FROM darfes d
             WHERE d.eliminado_en IS NULL
               AND ($1::text IS NULL OR (d.numero_darfe ILIKE $1 OR d.nombre ILIKE $1 OR d.expediente ILIKE $1
                    OR d.registro ILIKE $1 OR d.estado_cuenta ILIKE $1 OR d.banco ILIKE $1
                    OR d.identificacion ILIKE $1 OR d.curp ILIKE $1 OR d.rfc ILIKE $1))
               AND ($2::date IS NULL OR d.fecha >= $2)
               AND ($3::date IS NULL OR d.fecha <= $3)
               AND ($4::text IS NULL OR d.estatus = $4)
               AND ($5::text IS NULL OR d.banco ILIKE $5)
             ORDER BY d.fecha DESC, d.numero_darfe DESC
             LIMIT $6 OFFSET $7",
        )
        .bind(&buscar)
        .bind(filtro.fecha_desde)
        .bind(filtro.fecha_hasta)
        .bind(estatus)
        .bind(&banco)
        .bind(limite)
        .bind(offset)
        .fetch_all(&app.db)
        .await?;

    let registros = filas
        .into_iter()
        .map(|(id, numero_darfe, fecha, nombre, expediente, registro, estado_cuenta, banco, identificacion, curp, rfc, estatus, extra, documentos)| {
            json!({
                "id": id,
                "numero_darfe": numero_darfe,
                "fecha": fecha,
                "nombre": nombre,
                "expediente": expediente,
                "registro": registro,
                "estado_cuenta": estado_cuenta,
                "banco": banco,
                "identificacion": identificacion,
                "curp": curp,
                "rfc": rfc,
                "estatus": estatus,
                "campos_extra": extra,
                "documentos": documentos,
            })
        })
        .collect();
    Ok((total, registros))
}

async fn consultar(app: &AppState, id: Uuid) -> Result<Value, ApiError> {
    let fila: Option<(Uuid, String, NaiveDate, String, String, String, String, String, String, String, String, String, Value, Vec<String>, chrono::DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT d.id, d.numero_darfe, d.fecha, d.nombre, d.expediente, d.registro,
                    d.estado_cuenta, d.banco, d.identificacion, d.curp, d.rfc, d.estatus,
                    d.campos_extra,
                    ARRAY(SELECT campo FROM darfe_documentos WHERE darfe_id=d.id ORDER BY campo),
                    d.actualizado_en
             FROM darfes d WHERE d.id=$1 AND d.eliminado_en IS NULL",
        )
        .bind(id)
        .fetch_optional(&app.db)
        .await?;
    let Some((id, numero_darfe, fecha, nombre, expediente, registro, estado_cuenta, banco, identificacion, curp, rfc, estatus, extra, documentos, actualizado)) = fila else {
        return Err(ApiError::no_encontrado("DARFE no encontrado"));
    };
    Ok(json!({
        "id": id,
        "numero_darfe": numero_darfe,
        "fecha": fecha,
        "nombre": nombre,
        "expediente": expediente,
        "registro": registro,
        "estado_cuenta": estado_cuenta,
        "banco": banco,
        "identificacion": identificacion,
        "curp": curp,
        "rfc": rfc,
        "estatus": estatus,
        "campos_extra": extra,
        "documentos": documentos,
        "actualizado_en": actualizado,
    }))
}

fn validar(input: DarfeInput) -> Result<DatosDarfe, ApiError> {
    let numero_darfe = input.numero_darfe.trim().to_string();
    if numero_darfe.is_empty() || numero_darfe.chars().count() > 100 {
        return Err(ApiError::inprocesable("El número de DARFE es obligatorio y admite hasta 100 caracteres"));
    }
    let nombre = input.nombre.trim().to_string();
    if nombre.is_empty() || nombre.chars().count() > 200 {
        return Err(ApiError::inprocesable("El nombre es obligatorio y admite hasta 200 caracteres"));
    }
    let estatus = input.estatus.trim().to_uppercase();
    if !["PENDIENTE", "INCOMPLETO", "COMPLETO"].contains(&estatus.as_str()) {
        return Err(ApiError::inprocesable("El estatus debe ser PENDIENTE, INCOMPLETO o COMPLETO"));
    }
    let campo = |valor: String, max: usize, nombre_campo: &str| -> Result<String, ApiError> {
        let valor = valor.trim().to_string();
        if valor.chars().count() > max {
            return Err(ApiError::inprocesable(format!("{nombre_campo} admite hasta {max} caracteres")));
        }
        Ok(valor)
    };
    if input.campos_extra.len() > 30 {
        return Err(ApiError::inprocesable("El registro admite hasta 30 campos adicionales"));
    }
    let mut campos_extra = Vec::with_capacity(input.campos_extra.len());
    for extra in input.campos_extra {
        let nombre_extra = extra.nombre.trim().to_string();
        if nombre_extra.is_empty() || nombre_extra.chars().count() > 80 {
            return Err(ApiError::inprocesable("Cada campo adicional requiere un nombre de hasta 80 caracteres"));
        }
        if extra.valor.chars().count() > 500 {
            return Err(ApiError::inprocesable("Cada valor adicional admite hasta 500 caracteres"));
        }
        campos_extra.push(CampoExtra {
            nombre: nombre_extra,
            valor: extra.valor.trim().to_string(),
        });
    }

    Ok(DatosDarfe {
        numero_darfe,
        fecha: input.fecha,
        nombre,
        expediente: campo(input.expediente, 200, "El expediente")?,
        registro: campo(input.registro, 100, "El registro")?,
        estado_cuenta: campo(input.estado_cuenta, 100, "El estado de cuenta")?,
        banco: campo(input.banco, 120, "El banco")?,
        identificacion: campo(input.identificacion, 100, "La identificación")?,
        curp: campo(input.curp.to_uppercase(), 18, "La CURP")?,
        rfc: campo(input.rfc.to_uppercase(), 13, "El RFC")?,
        estatus,
        campos_extra,
    })
}

fn patron(valor: &Option<String>) -> Option<String> {
    valor.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(|v| format!("%{v}%"))
}

fn duplicado(error: sqlx::Error) -> ApiError {
    if error
        .as_database_error()
        .map(|base| base.is_unique_violation())
        .unwrap_or(false)
    {
        return ApiError::conflicto("Ya existe un DARFE con ese número");
    }
    ApiError::from(error)
}

fn error_excel(error: rust_xlsxwriter::XlsxError) -> ApiError {
    ApiError::interno(format!("No se pudo generar el Excel: {error}"))
}
