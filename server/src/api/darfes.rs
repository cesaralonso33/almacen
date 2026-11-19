use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Months, NaiveDate, Utc};
use pdf_extract::extract_text_from_mem;
use regex::Regex;
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

const CAMPOS_DOCUMENTO: [&str; 7] = [
    "CURP",
    "RFC",
    "REGISTRO",
    "IDENTIFICACION",
    "ESTADO_CUENTA",
    "DEPOSITO",
    "ESCRITO_LIBRE",
];
const LIMITE_DOCUMENTO: usize = 8 * 1024 * 1024;
const DESCRIPCIONES_CANASTA: [&str; 3] = ["1ra canasta 3", "2da canasta de 3", "3ra canasta de 3"];

#[derive(Clone, Deserialize, Serialize)]
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
    #[serde(default)]
    pub telefono: String,
    #[serde(default)]
    pub correo: String,
    #[serde(default)]
    pub descripcion: String,
    pub monto: f64,
    #[serde(default)]
    pub numero_cuenta: String,
    pub tipo: String,
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
    pub tipo: Option<String>,
    pub pagina: Option<i64>,
    pub tamano: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct FilaDarfe {
    id: Uuid,
    numero_darfe: String,
    fecha: NaiveDate,
    nombre: String,
    descripcion: String,
    monto: f64,
    numero_cuenta: String,
    expediente: String,
    registro: String,
    estado_cuenta: String,
    banco: String,
    identificacion: String,
    curp: String,
    rfc: String,
    telefono: String,
    correo: String,
    tipo: String,
    estatus: String,
    campos_extra: Value,
    documentos: Vec<String>,
    deposito_coincide: Option<bool>,
    actualizado_en: chrono::DateTime<Utc>,
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
    telefono: String,
    correo: String,
    descripcion: String,
    monto: f64,
    numero_cuenta: String,
    tipo: String,
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
    if datos.tipo == "CANASTA" {
        return crear_canasta(&app, &user, datos).await;
    }

    let mut tx = app.db.begin().await?;
    let id = insertar(
        &mut tx,
        &datos,
        &datos.descripcion,
        "REEMBOLSO",
        None,
        None,
        user.id,
    )
    .await?;
    audit::registrar(
        &mut tx,
        Some(user.id),
        "darfes.crear",
        "darfes",
        id,
        "EXITO",
        json!({ "numero_darfe": datos.numero_darfe, "tipo": datos.tipo, "estatus": datos.estatus }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(consultar(&app, id).await?))
}

async fn crear_canasta(
    app: &AppState,
    user: &AuthUser,
    datos: DatosDarfe,
) -> Result<Json<Value>, ApiError> {
    let grupo = Uuid::new_v4();
    let mut tx = app.db.begin().await?;
    let mut ids = Vec::with_capacity(3);

    for (indice, descripcion) in DESCRIPCIONES_CANASTA.iter().enumerate() {
        let fecha = datos
            .fecha
            .checked_add_months(Months::new(indice as u32))
            .ok_or_else(|| ApiError::inprocesable("La fecha de la canasta no es válida"))?;
        let item = DatosDarfe {
            numero_darfe: datos.numero_darfe.clone(),
            fecha,
            nombre: datos.nombre.clone(),
            expediente: datos.expediente.clone(),
            registro: datos.registro.clone(),
            estado_cuenta: datos.estado_cuenta.clone(),
            banco: datos.banco.clone(),
            identificacion: datos.identificacion.clone(),
            curp: datos.curp.clone(),
            rfc: datos.rfc.clone(),
            telefono: datos.telefono.clone(),
            correo: datos.correo.clone(),
            descripcion: (*descripcion).to_string(),
            monto: datos.monto,
            numero_cuenta: datos.numero_cuenta.clone(),
            tipo: datos.tipo.clone(),
            estatus: datos.estatus.clone(),
            campos_extra: datos.campos_extra.clone(),
        };
        ids.push(
            insertar(
                &mut tx,
                &item,
                descripcion,
                "CANASTA",
                Some(grupo),
                Some((indice + 1) as i16),
                user.id,
            )
            .await?,
        );
    }

    audit::registrar(
        &mut tx,
        Some(user.id),
        "darfes.crear_canasta",
        "darfes",
        ids[0],
        "EXITO",
        json!({ "numero_darfe": datos.numero_darfe, "grupo": grupo, "ids": ids }),
    )
    .await?;
    tx.commit().await?;

    let mut registros = Vec::with_capacity(ids.len());
    for id in &ids {
        registros.push(consultar(app, *id).await?);
    }
    Ok(Json(json!({
        "darfes": registros,
        "grupo_canasta": grupo,
        "total": ids.len(),
    })))
}

#[allow(clippy::too_many_arguments)]
async fn insertar(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    datos: &DatosDarfe,
    descripcion: &str,
    tipo: &str,
    grupo_canasta: Option<Uuid>,
    item_canasta: Option<i16>,
    usuario: Uuid,
) -> Result<Uuid, ApiError> {
    let extra = serde_json::to_value(&datos.campos_extra)
        .map_err(|error| ApiError::interno(error.to_string()))?;
    sqlx::query_scalar(
        "INSERT INTO darfes
            (numero_darfe, fecha, nombre, descripcion, monto, numero_cuenta, tipo,
             grupo_canasta, item_canasta, expediente, registro, estado_cuenta, banco,
             identificacion, curp, rfc, telefono, correo, estatus, campos_extra,
             creado_por, actualizado_por)
         VALUES ($1,$2,$3,$4,$5::numeric,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$21)
         RETURNING id",
    )
    .bind(&datos.numero_darfe)
    .bind(datos.fecha)
    .bind(&datos.nombre)
    .bind(descripcion)
    .bind(datos.monto.to_string())
    .bind(&datos.numero_cuenta)
    .bind(tipo)
    .bind(grupo_canasta)
    .bind(item_canasta)
    .bind(&datos.expediente)
    .bind(&datos.registro)
    .bind(&datos.estado_cuenta)
    .bind(&datos.banco)
    .bind(&datos.identificacion)
    .bind(&datos.curp)
    .bind(&datos.rfc)
    .bind(&datos.telefono)
    .bind(&datos.correo)
    .bind(&datos.estatus)
    .bind(&extra)
    .bind(usuario)
    .fetch_one(&mut **tx)
    .await
    .map_err(duplicado)
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

    let actual: Option<(String, Option<Uuid>, Option<i16>)> = sqlx::query_as(
        "SELECT tipo, grupo_canasta, item_canasta FROM darfes
         WHERE id=$1 AND eliminado_en IS NULL",
    )
    .bind(id)
    .fetch_optional(&app.db)
    .await?;
    let Some((tipo_actual, grupo, item)) = actual else {
        return Err(ApiError::no_encontrado("DARFE no encontrado"));
    };
    if tipo_actual != datos.tipo {
        return Err(ApiError::conflicto(
            "No se puede cambiar el tipo de un DARFE existente",
        ));
    }
    let descripcion = if tipo_actual == "CANASTA" {
        item.and_then(|valor| DESCRIPCIONES_CANASTA.get((valor - 1) as usize).copied())
            .unwrap_or(datos.descripcion.as_str())
    } else {
        datos.descripcion.as_str()
    };

    let mut tx = app.db.begin().await?;
    sqlx::query(
        "UPDATE darfes
         SET numero_darfe=$2, fecha=$3, nombre=$4, descripcion=$5, monto=$6::numeric,
             numero_cuenta=$7, tipo=$8, grupo_canasta=$9, item_canasta=$10,
             expediente=$11, registro=$12, estado_cuenta=$13, banco=$14,
             identificacion=$15, curp=$16, rfc=$17, telefono=$18, correo=$19,
             estatus=$20, campos_extra=$21, actualizado_por=$22, actualizado_en=now()
         WHERE id=$1",
    )
    .bind(id)
    .bind(&datos.numero_darfe)
    .bind(datos.fecha)
    .bind(&datos.nombre)
    .bind(descripcion)
    .bind(datos.monto.to_string())
    .bind(&datos.numero_cuenta)
    .bind(&datos.tipo)
    .bind(grupo)
    .bind(item)
    .bind(&datos.expediente)
    .bind(&datos.registro)
    .bind(&datos.estado_cuenta)
    .bind(&datos.banco)
    .bind(&datos.identificacion)
    .bind(&datos.curp)
    .bind(&datos.rfc)
    .bind(&datos.telefono)
    .bind(&datos.correo)
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
        return Err(ApiError::inprocesable(
            "El nombre del documento es obligatorio",
        ));
    }
    let contenido = STANDARD
        .decode(input.contenido_base64.as_bytes())
        .map_err(|_| {
            ApiError::solicitud_invalida("El contenido del documento no es base64 válido")
        })?;
    if contenido.is_empty() || contenido.len() > LIMITE_DOCUMENTO {
        return Err(ApiError::inprocesable(
            "El documento debe pesar entre 1 byte y 8 MiB",
        ));
    }

    let datos: Option<(String, f64, String)> = sqlx::query_as(
        "SELECT numero_cuenta, monto::float8, nombre FROM darfes
         WHERE id=$1 AND eliminado_en IS NULL",
    )
    .bind(id)
    .fetch_optional(&app.db)
    .await?;
    let Some((numero_cuenta, monto, nombre)) = datos else {
        return Err(ApiError::no_encontrado("DARFE no encontrado"));
    };

    let (datos_extraidos, coincide) = if campo == "DEPOSITO" {
        validar_deposito(&contenido, &numero_cuenta, monto, &nombre)
    } else {
        (Value::Null, None)
    };

    let mut tx = app.db.begin().await?;
    sqlx::query(
        "INSERT INTO darfe_documentos
            (darfe_id, campo, nombre, mime, tamano, contenido, datos_extraidos, coincide, creado_por)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         ON CONFLICT (darfe_id, campo) DO UPDATE SET
             nombre=excluded.nombre, mime=excluded.mime, tamano=excluded.tamano,
             contenido=excluded.contenido, datos_extraidos=excluded.datos_extraidos,
             coincide=excluded.coincide, creado_por=excluded.creado_por, creado_en=now()",
    )
    .bind(id)
    .bind(&campo)
    .bind(input.nombre.trim())
    .bind(input.mime.trim())
    .bind(contenido.len() as i32)
    .bind(&contenido)
    .bind(&datos_extraidos)
    .bind(coincide)
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
        json!({ "campo": campo, "tamano": contenido.len(), "coincide": coincide }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(json!({
        "campo": campo,
        "tamano": contenido.len(),
        "coincide": coincide,
        "datos_extraidos": datos_extraidos,
    })))
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
            HeaderValue::from_str(&mime)
                .unwrap_or(HeaderValue::from_static("application/octet-stream")),
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposicion)
                .map_err(|error| ApiError::interno(error.to_string()))?,
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
        "NO. DARFE",
        "FECHA",
        "NOMBRE",
        "DESCRIPCIÓN",
        "MONTO",
        "NO. CUENTA",
        "EXPEDIENTE",
        "REGISTRO",
        "ESTADO CUENTA",
        "BANCO",
        "IDENTIFICACIÓN",
        "TELÉFONO",
        "CORREO",
        "CURP",
        "RFC",
        "TIPO",
        "ESTATUS",
        "CAMPOS ADICIONALES",
        "DEPÓSITO COINCIDE",
    ];
    for (indice, titulo) in columnas.iter().enumerate() {
        hoja.write_string_with_format(0, indice as u16, *titulo, &encabezado)
            .map_err(error_excel)?;
        hoja.set_column_width(indice as u16, 18)
            .map_err(error_excel)?;
    }

    let posicion = |titulo: &str| -> u16 {
        columnas
            .iter()
            .position(|columna| *columna == titulo)
            .unwrap_or(0) as u16
    };
    for (fila, registro) in registros.iter().enumerate() {
        let renglon = fila as u32 + 1;
        let textos = [
            ("NO. DARFE", "numero_darfe"),
            ("FECHA", "fecha"),
            ("NOMBRE", "nombre"),
            ("DESCRIPCIÓN", "descripcion"),
            ("NO. CUENTA", "numero_cuenta"),
            ("EXPEDIENTE", "expediente"),
            ("REGISTRO", "registro"),
            ("ESTADO CUENTA", "estado_cuenta"),
            ("BANCO", "banco"),
            ("IDENTIFICACIÓN", "identificacion"),
            ("TELÉFONO", "telefono"),
            ("CORREO", "correo"),
            ("CURP", "curp"),
            ("RFC", "rfc"),
            ("TIPO", "tipo"),
            ("ESTATUS", "estatus"),
        ];
        for (titulo, clave) in textos {
            hoja.write_string(
                renglon,
                posicion(titulo),
                registro[clave].as_str().unwrap_or(""),
            )
            .map_err(error_excel)?;
        }
        hoja.write_number(
            renglon,
            posicion("MONTO"),
            registro["monto"].as_f64().unwrap_or(0.0),
        )
        .map_err(error_excel)?;
        hoja.write_string(
            renglon,
            posicion("CAMPOS ADICIONALES"),
            &registro["campos_extra"].to_string(),
        )
        .map_err(error_excel)?;
        let deposito = match registro["deposito_coincide"].as_bool() {
            Some(true) => "SÍ",
            Some(false) => "NO",
            None => "",
        };
        hoja.write_string(renglon, posicion("DEPÓSITO COINCIDE"), deposito)
            .map_err(error_excel)?;
    }

    let bytes = libro.save_to_buffer().map_err(error_excel)?;
    let nombre = format!("DARFES-{}.xlsx", Utc::now().format("%Y%m%d-%H%M%S"));
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
    let por_tipo: Vec<(String, i64)> = sqlx::query_as(
        "SELECT tipo, count(*) FROM darfes WHERE eliminado_en IS NULL GROUP BY tipo",
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
        "por_tipo": mapa(por_tipo),
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
    let estatus = filtro
        .estatus
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let tipo = filtro
        .tipo
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
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
           AND ($1::text IS NULL OR (numero_darfe ILIKE $1 OR nombre ILIKE $1 OR descripcion ILIKE $1
                OR expediente ILIKE $1 OR registro ILIKE $1 OR estado_cuenta ILIKE $1
                OR banco ILIKE $1 OR identificacion ILIKE $1 OR curp ILIKE $1 OR rfc ILIKE $1
                OR telefono ILIKE $1 OR correo ILIKE $1
                OR numero_cuenta ILIKE $1 OR tipo ILIKE $1))
           AND ($2::date IS NULL OR fecha >= $2)
           AND ($3::date IS NULL OR fecha <= $3)
           AND ($4::text IS NULL OR estatus = $4)
           AND ($5::text IS NULL OR banco ILIKE $5)
           AND ($6::text IS NULL OR tipo = $6)",
    )
    .bind(&buscar)
    .bind(filtro.fecha_desde)
    .bind(filtro.fecha_hasta)
    .bind(estatus)
    .bind(&banco)
    .bind(tipo)
    .fetch_one(&app.db)
    .await?;

    let filas: Vec<FilaDarfe> = sqlx::query_as(
        "SELECT d.id, d.numero_darfe, d.fecha, d.nombre, d.descripcion,
                d.monto::float8, d.numero_cuenta, d.expediente, d.registro,
                d.estado_cuenta, d.banco, d.identificacion, d.curp, d.rfc,
                d.telefono, d.correo, d.tipo, d.estatus, d.campos_extra,
                ARRAY(SELECT campo FROM darfe_documentos WHERE darfe_id=d.id ORDER BY campo) AS documentos,
                (SELECT coincide FROM darfe_documentos WHERE darfe_id=d.id AND campo='DEPOSITO') AS deposito_coincide,
                d.actualizado_en
         FROM darfes d
         WHERE d.eliminado_en IS NULL
           AND ($1::text IS NULL OR (d.numero_darfe ILIKE $1 OR d.nombre ILIKE $1 OR d.descripcion ILIKE $1
                OR d.expediente ILIKE $1 OR d.registro ILIKE $1 OR d.estado_cuenta ILIKE $1
                OR d.banco ILIKE $1 OR d.identificacion ILIKE $1 OR d.curp ILIKE $1 OR d.rfc ILIKE $1
                OR d.telefono ILIKE $1 OR d.correo ILIKE $1
                OR d.numero_cuenta ILIKE $1 OR d.tipo ILIKE $1))
           AND ($2::date IS NULL OR d.fecha >= $2)
           AND ($3::date IS NULL OR d.fecha <= $3)
           AND ($4::text IS NULL OR d.estatus = $4)
           AND ($5::text IS NULL OR d.banco ILIKE $5)
           AND ($6::text IS NULL OR d.tipo = $6)
         ORDER BY d.creado_en DESC, d.id DESC
         LIMIT $7 OFFSET $8",
    )
    .bind(&buscar)
    .bind(filtro.fecha_desde)
    .bind(filtro.fecha_hasta)
    .bind(estatus)
    .bind(&banco)
    .bind(tipo)
    .bind(limite)
    .bind(offset)
    .fetch_all(&app.db)
    .await?;

    let registros = filas
        .into_iter()
        .map(|fila| {
            json!({
                "id": fila.id,
                "numero_darfe": fila.numero_darfe,
                "fecha": fila.fecha,
                "nombre": fila.nombre,
                "descripcion": fila.descripcion,
                "monto": fila.monto,
                "numero_cuenta": fila.numero_cuenta,
                "expediente": fila.expediente,
                "registro": fila.registro,
                "estado_cuenta": fila.estado_cuenta,
                "banco": fila.banco,
                "identificacion": fila.identificacion,
                "curp": fila.curp,
                "rfc": fila.rfc,
                "telefono": fila.telefono,
                "correo": fila.correo,
                "tipo": fila.tipo,
                "estatus": fila.estatus,
                "campos_extra": fila.campos_extra,
                "documentos": fila.documentos,
                "deposito_coincide": fila.deposito_coincide,
            })
        })
        .collect();
    Ok((total, registros))
}

async fn consultar(app: &AppState, id: Uuid) -> Result<Value, ApiError> {
    let fila: Option<FilaDarfe> = sqlx::query_as(
        "SELECT d.id, d.numero_darfe, d.fecha, d.nombre, d.descripcion,
                d.monto::float8, d.numero_cuenta, d.expediente, d.registro,
                d.estado_cuenta, d.banco, d.identificacion, d.curp, d.rfc,
                d.telefono, d.correo, d.tipo, d.estatus, d.campos_extra,
                ARRAY(SELECT campo FROM darfe_documentos WHERE darfe_id=d.id ORDER BY campo) AS documentos,
                (SELECT coincide FROM darfe_documentos WHERE darfe_id=d.id AND campo='DEPOSITO') AS deposito_coincide,
                d.actualizado_en
         FROM darfes d WHERE d.id=$1 AND d.eliminado_en IS NULL",
    )
    .bind(id)
    .fetch_optional(&app.db)
    .await?;
    let Some(fila) = fila else {
        return Err(ApiError::no_encontrado("DARFE no encontrado"));
    };
    Ok(json!({
        "id": fila.id,
        "numero_darfe": fila.numero_darfe,
        "fecha": fila.fecha,
        "nombre": fila.nombre,
        "descripcion": fila.descripcion,
        "monto": fila.monto,
        "numero_cuenta": fila.numero_cuenta,
        "expediente": fila.expediente,
        "registro": fila.registro,
        "estado_cuenta": fila.estado_cuenta,
        "banco": fila.banco,
        "identificacion": fila.identificacion,
        "curp": fila.curp,
        "rfc": fila.rfc,
        "telefono": fila.telefono,
        "correo": fila.correo,
        "tipo": fila.tipo,
        "estatus": fila.estatus,
        "campos_extra": fila.campos_extra,
        "documentos": fila.documentos,
        "deposito_coincide": fila.deposito_coincide,
        "actualizado_en": fila.actualizado_en,
    }))
}

fn validar(input: DarfeInput) -> Result<DatosDarfe, ApiError> {
    let numero_darfe = input.numero_darfe.trim().to_string();
    if numero_darfe.is_empty() || numero_darfe.chars().count() > 100 {
        return Err(ApiError::inprocesable(
            "El número de DARFE es obligatorio y admite hasta 100 caracteres",
        ));
    }
    let nombre = input.nombre.trim().to_string();
    if nombre.is_empty() || nombre.chars().count() > 200 {
        return Err(ApiError::inprocesable(
            "El nombre es obligatorio y admite hasta 200 caracteres",
        ));
    }
    let tipo = input.tipo.trim().to_uppercase();
    if !["CANASTA", "REEMBOLSO"].contains(&tipo.as_str()) {
        return Err(ApiError::inprocesable(
            "El tipo debe ser CANASTA o REEMBOLSO",
        ));
    }
    let estatus = input.estatus.trim().to_uppercase();
    if !["PENDIENTE", "INCOMPLETO", "COMPLETO"].contains(&estatus.as_str()) {
        return Err(ApiError::inprocesable(
            "El estatus debe ser PENDIENTE, INCOMPLETO o COMPLETO",
        ));
    }
    if !input.monto.is_finite() || input.monto < 0.0 || input.monto > 1_000_000_000_000.0 {
        return Err(ApiError::inprocesable("El monto no es válido"));
    }
    let monto = (input.monto * 100.0).round() / 100.0;
    let campo = |valor: String, max: usize, nombre_campo: &str| -> Result<String, ApiError> {
        let valor = valor.trim().to_string();
        if valor.chars().count() > max {
            return Err(ApiError::inprocesable(format!(
                "{nombre_campo} admite hasta {max} caracteres"
            )));
        }
        Ok(valor)
    };
    if input.campos_extra.len() > 30 {
        return Err(ApiError::inprocesable(
            "El registro admite hasta 30 campos adicionales",
        ));
    }
    let mut campos_extra = Vec::with_capacity(input.campos_extra.len());
    for extra in input.campos_extra {
        let nombre_extra = extra.nombre.trim().to_string();
        if nombre_extra.is_empty() || nombre_extra.chars().count() > 80 {
            return Err(ApiError::inprocesable(
                "Cada campo adicional requiere un nombre de hasta 80 caracteres",
            ));
        }
        if extra.valor.chars().count() > 500 {
            return Err(ApiError::inprocesable(
                "Cada valor adicional admite hasta 500 caracteres",
            ));
        }
        campos_extra.push(CampoExtra {
            nombre: nombre_extra,
            valor: extra.valor.trim().to_string(),
        });
    }

    let correo = campo(input.correo, 200, "El correo")?;
    if !correo.is_empty()
        && (correo.contains(char::is_whitespace)
            || !correo.contains('@')
            || correo.starts_with('@')
            || correo.ends_with('@'))
    {
        return Err(ApiError::inprocesable(
            "El correo no tiene un formato válido",
        ));
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
        telefono: campo(input.telefono, 30, "El teléfono")?,
        correo: correo,
        descripcion: campo(input.descripcion, 500, "La descripción")?,
        monto,
        numero_cuenta: campo(input.numero_cuenta, 50, "El número de cuenta")?,
        tipo,
        estatus,
        campos_extra,
    })
}

fn patron(valor: &Option<String>) -> Option<String> {
    valor
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| format!("%{v}%"))
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

fn validar_deposito(
    contenido: &[u8],
    cuenta_capturada: &str,
    monto_capturado: f64,
    nombre_capturado: &str,
) -> (Value, Option<bool>) {
    let texto = match extract_text_from_mem(contenido) {
        Ok(texto) => texto,
        Err(error) => {
            return (
                json!({ "error": format!("No se pudo leer el PDF: {error}") }),
                Some(false),
            )
        }
    };
    comparar_deposito(&texto, cuenta_capturada, monto_capturado, nombre_capturado)
}

fn comparar_deposito(
    texto: &str,
    cuenta_capturada: &str,
    monto_capturado: f64,
    nombre_capturado: &str,
) -> (Value, Option<bool>) {
    let extraido = extraer_deposito(texto);

    let cuenta_normalizada = solo_digitos(cuenta_capturada);
    let cuenta_ok = !cuenta_normalizada.is_empty()
        && extraido
            .numero_cuenta
            .as_deref()
            .map(|cuenta| solo_digitos(cuenta) == cuenta_normalizada)
            .unwrap_or(false);
    let monto_ok = extraido
        .monto
        .map(|valor| (valor - monto_capturado).abs() < 0.01)
        .unwrap_or(false);
    let nombre_ok = extraido
        .nombre
        .as_deref()
        .map(|valor| {
            let capturado = normalizar_nombre(nombre_capturado);
            let encontrado = normalizar_nombre(valor);
            !capturado.is_empty()
                && (encontrado.contains(&capturado) || capturado.contains(&encontrado))
        })
        .unwrap_or(false);

    let mut diferencias = Vec::new();
    if !cuenta_ok {
        diferencias.push("El número de cuenta no coincide");
    }
    if !monto_ok {
        diferencias.push("El monto no coincide");
    }
    if !nombre_ok {
        diferencias.push("El nombre no coincide");
    }
    let coincide = diferencias.is_empty();
    (
        json!({
            "numero_cuenta": extraido.numero_cuenta,
            "monto": extraido.monto,
            "nombre": extraido.nombre,
            "diferencias": diferencias,
        }),
        Some(coincide),
    )
}

struct DepositoExtraido {
    numero_cuenta: Option<String>,
    monto: Option<f64>,
    nombre: Option<String>,
}

fn extraer_deposito(texto: &str) -> DepositoExtraido {
    // En los reportes SPEI la CLABE del beneficiario aparece como
    // `Cuenta/CLABE/Celular`. La cuenta del ordenante (la institución) no debe
    // compararse contra el DARFE.
    let cuenta = valor_etiqueta(
        texto,
        &[
            "cuenta/clabe/celular",
            "cuenta / clabe / celular",
            "clabe interbancaria",
            "clabe",
            "numero de cuenta",
            "no. de cuenta",
            "cuenta",
        ],
    )
    .map(|valor| solo_digitos(&valor))
    .filter(|valor| valor.len() >= 8);

    // `Importe a Transferir` es el monto en los reportes SPEI; se aceptan
    // además las etiquetas habituales de los comprobantes de depósito.
    let monto = valor_etiqueta(
        texto,
        &[
            "importe a transferir",
            "monto",
            "importe",
            "total",
            "deposito",
        ],
    )
    .and_then(|valor| primer_numero(&valor))
    .or_else(|| {
        capturar(
            texto,
            r"(?i)(?:monto|importe|total)[^\d]{0,40}([0-9][0-9,]*(?:\.[0-9]{1,2})?)",
        )
        .and_then(|valor| monto_numero(&valor))
    });

    // El nombre relevante es el del beneficiario, no el del ordenante.
    let nombre = valor_etiqueta(
        texto,
        &[
            "nombre del beneficiario",
            "beneficiario",
            "nombre del titular",
            "titular",
            "nombre",
        ],
    )
    .filter(|valor| valor.chars().count() >= 3);

    DepositoExtraido {
        numero_cuenta: cuenta,
        monto,
        nombre,
    }
}

/// Devuelve el texto que sigue a la primera etiqueta encontrada, comparando
/// sin distinguir mayúsculas ni acentos. Las etiquetas se evalúan en orden de
/// prioridad, de modo que una etiqueta específica gana sobre una genérica.
fn valor_etiqueta(texto: &str, etiquetas: &[&str]) -> Option<String> {
    for etiqueta in etiquetas {
        let buscada = plegar(etiqueta);
        for linea in texto.lines() {
            let limpia = linea.trim();
            let plegada = plegar(limpia);
            if !plegada.starts_with(&buscada) {
                continue;
            }
            let generica = matches!(*etiqueta, "cuenta" | "nombre");
            if generica && plegada.contains("ordenante") {
                continue;
            }
            let caracteres = buscada.chars().count();
            let resto: String = limpia
                .chars()
                .skip(caracteres)
                .collect::<String>()
                .trim_start_matches([':', ' ', '\t'])
                .trim()
                .to_string();
            if !resto.is_empty() {
                return Some(resto);
            }
        }
    }
    None
}

fn primer_numero(texto: &str) -> Option<f64> {
    let capturado = capturar(texto, r"([0-9][0-9,]*(?:\.[0-9]{1,2})?)")?;
    monto_numero(&capturado)
}

fn capturar(texto: &str, patron: &str) -> Option<String> {
    Regex::new(patron)
        .ok()?
        .captures(texto)?
        .get(1)
        .map(|coincidencia| coincidencia.as_str().trim().to_string())
}

fn solo_digitos(texto: &str) -> String {
    texto.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn monto_numero(texto: &str) -> Option<f64> {
    texto.replace(',', "").trim().parse::<f64>().ok()
}

/// Minúsculas y sin acentos. Conserva un carácter por cada carácter de
/// entrada para poder recortar la línea original por posición.
fn plegar(texto: &str) -> String {
    texto
        .chars()
        .map(|c| match c {
            'Á' | 'À' | 'Ä' | 'Â' | 'á' | 'à' | 'ä' | 'â' => 'a',
            'É' | 'È' | 'Ë' | 'Ê' | 'é' | 'è' | 'ë' | 'ê' => 'e',
            'Í' | 'Ì' | 'Ï' | 'Î' | 'í' | 'ì' | 'ï' | 'î' => 'i',
            'Ó' | 'Ò' | 'Ö' | 'Ô' | 'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'Ú' | 'Ù' | 'Ü' | 'Û' | 'ú' | 'ù' | 'ü' | 'û' => 'u',
            'Ñ' | 'ñ' => 'n',
            otro => otro.to_ascii_lowercase(),
        })
        .collect()
}

fn normalizar_nombre(texto: &str) -> String {
    plegar(texto)
        .to_uppercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use genpdf::{
        elements::Paragraph,
        fonts::{FontData, FontFamily},
        Document,
    };

    fn pdf_con(lineas: &[&str]) -> Vec<u8> {
        let fuente = FontData::new(
            include_bytes!("../../assets/NotoSans-Regular.ttf").to_vec(),
            None,
        )
        .expect("fuente del PDF");
        let familia = FontFamily {
            regular: fuente.clone(),
            bold: fuente.clone(),
            italic: fuente.clone(),
            bold_italic: fuente,
        };
        let mut documento = Document::new(familia);
        documento.set_font_size(11);
        for linea in lineas {
            documento.push(Paragraph::new(*linea));
        }
        let mut pdf = Vec::new();
        documento.render(&mut pdf).expect("PDF en memoria");
        pdf
    }

    #[test]
    fn el_deposito_coincide_con_cuenta_monto_y_nombre() {
        let pdf = pdf_con(&[
            "Comprobante de deposito",
            "Numero de cuenta: 1234567890",
            "Monto: $1,500.50",
            "Nombre: JUAN PEREZ LOPEZ",
        ]);
        let (extraidos, coincide) =
            validar_deposito(&pdf, "1234567890", 1500.50, "Juan Pérez López");
        assert_eq!(coincide, Some(true), "{extraidos}");
        assert_eq!(extraidos["numero_cuenta"], "1234567890");
        assert_eq!(extraidos["monto"], 1500.50);
    }

    #[test]
    fn el_deposito_reporta_las_diferencias() {
        let pdf = pdf_con(&[
            "Numero de cuenta: 9999999999",
            "Monto: $1,500.50",
            "Nombre: JUAN PEREZ LOPEZ",
        ]);
        let (extraidos, coincide) =
            validar_deposito(&pdf, "1234567890", 1500.50, "Juan Pérez López");
        assert_eq!(coincide, Some(false), "{extraidos}");
        let diferencias = extraidos["diferencias"].as_array().expect("diferencias");
        assert!(
            diferencias
                .iter()
                .any(|valor| valor.as_str().unwrap_or("").contains("cuenta")),
            "{extraidos}"
        );
    }

    #[test]
    fn interpreta_el_reporte_spei_del_banco() {
        // Estructura real de un "Reporte de Transferencia a Nacionales SPEI",
        // con datos ficticios.
        let texto = "\
Reporte de Transferencia a Nacionales SPEI BANCO MERCANTIL DEL NORTE S.A.
RFC: BMN-930209-927
18/11/2026 10:42
Cuenta/ CLABE Ordenante 1257199978
Nombre del Ordenante DEPENDENCIA ORDENANTE
RFC o CURP del Ordenante CEE200101A29
Moneda MXP
Nombre del Beneficiario MARIA LOPEZ HERNANDEZ
Cuenta/CLABE/Celular 012345678901234567
RFC Beneficiario LOPM900101ABC
Banco Destino AZTECA
Importe a Transferir $1,234.56
IVA $0.00
Concepto de la Transferencia APOYO CANASTA ALIMENTOS 1RA MARIA LOPEZ
Clave de Rastreo 8846APR1202611186031515573";
        let (extraidos, coincide) = comparar_deposito(
            texto,
            "012345678901234567",
            1234.56,
            "María López Hernández",
        );
        assert_eq!(coincide, Some(true), "{extraidos}");
        // La cuenta del ordenante no debe tomarse como la del beneficiario.
        assert_eq!(extraidos["numero_cuenta"], "012345678901234567");
        assert_eq!(extraidos["monto"], 1234.56);
        assert_eq!(extraidos["nombre"], "MARIA LOPEZ HERNANDEZ");
    }

    #[test]
    fn el_reporte_spei_detecta_un_importe_distinto() {
        let texto = "\
Cuenta/CLABE/Celular 012345678901234567
Nombre del Beneficiario MARIA LOPEZ HERNANDEZ
Importe a Transferir $1,234.56";
        let (extraidos, coincide) = comparar_deposito(
            texto,
            "012345678901234567",
            1200.00,
            "María López Hernández",
        );
        assert_eq!(coincide, Some(false), "{extraidos}");
        assert_eq!(extraidos["diferencias"][0], "El monto no coincide");
    }
}
