use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use chrono::{Datelike, NaiveDate, Utc};
use genpdf::{
    elements::Paragraph,
    fonts::{FontData, FontFamily},
    style::Style,
    Document, Element, SimplePageDecorator,
};
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

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CampoExtra {
    pub nombre: String,
    pub valor: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaInput {
    pub fecha: NaiveDate,
    pub curp: String,
    pub nombre: String,
    pub genero: String,
    pub municipio: String,
    pub edad: i16,
    #[serde(default)]
    pub observaciones: String,
    #[serde(default)]
    pub campos_extra: Vec<CampoExtra>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaFiltro {
    pub buscar: Option<String>,
    pub fecha_desde: Option<NaiveDate>,
    pub fecha_hasta: Option<NaiveDate>,
    pub folio: Option<String>,
    pub curp: Option<String>,
    pub nombre: Option<String>,
    pub genero: Option<String>,
    pub municipio: Option<String>,
    pub edad_min: Option<i16>,
    pub edad_max: Option<i16>,
    pub campo: Option<String>,
    pub valor: Option<String>,
    pub pagina: Option<i64>,
    pub tamano: Option<i64>,
}

struct DatosPa {
    fecha: NaiveDate,
    curp: String,
    nombre: String,
    genero: String,
    municipio: String,
    edad: i16,
    observaciones: String,
    campos_extra: Vec<CampoExtra>,
}

pub async fn listar(
    State(app): State<AppState>,
    user: AuthUser,
    ApiQuery(filtro): ApiQuery<PaFiltro>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pa:leer")?;

    let buscar = patron(&filtro.buscar);
    let folio = patron(&filtro.folio);
    let curp = patron(&filtro.curp);
    let nombre = patron(&filtro.nombre);
    let genero = patron(&filtro.genero);
    let municipio = patron(&filtro.municipio);
    let campo = filtro.campo.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let valor = patron(&filtro.valor);
    let pagina = filtro.pagina.unwrap_or(1).max(1);
    let tamano = filtro.tamano.unwrap_or(25).clamp(1, 100);
    let offset = (pagina - 1) * tamano;

    let total: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM pa_registros
         WHERE eliminado_en IS NULL
           AND ($1::text IS NULL OR (folio ILIKE $1 OR curp ILIKE $1 OR nombre ILIKE $1 OR municipio ILIKE $1 OR observaciones ILIKE $1))
           AND ($2::date IS NULL OR fecha >= $2)
           AND ($3::date IS NULL OR fecha <= $3)
           AND ($4::text IS NULL OR folio ILIKE $4)
           AND ($5::text IS NULL OR curp ILIKE $5)
           AND ($6::text IS NULL OR nombre ILIKE $6)
           AND ($7::text IS NULL OR genero ILIKE $7)
           AND ($8::text IS NULL OR municipio ILIKE $8)
           AND ($9::smallint IS NULL OR edad >= $9)
           AND ($10::smallint IS NULL OR edad <= $10)
           AND ($11::text IS NULL OR EXISTS (
               SELECT 1 FROM jsonb_array_elements(campos_extra) AS campo_extra
               WHERE campo_extra->>'nombre' ILIKE $11
                 AND ($12::text IS NULL OR campo_extra->>'valor' ILIKE $12)
           ))",
    )
    .bind(&buscar)
    .bind(filtro.fecha_desde)
    .bind(filtro.fecha_hasta)
    .bind(&folio)
    .bind(&curp)
    .bind(&nombre)
    .bind(&genero)
    .bind(&municipio)
    .bind(filtro.edad_min)
    .bind(filtro.edad_max)
    .bind(&campo)
    .bind(&valor)
    .fetch_one(&app.db)
    .await?;

    let filas: Vec<(Uuid, NaiveDate, String, String, String, String, String, i16, String, Value, String)> =
        sqlx::query_as(
            "SELECT id, fecha, folio, curp, nombre, genero, municipio, edad,
                    observaciones, campos_extra, documento_nombre
             FROM pa_registros
             WHERE eliminado_en IS NULL
               AND ($1::text IS NULL OR (folio ILIKE $1 OR curp ILIKE $1 OR nombre ILIKE $1 OR municipio ILIKE $1 OR observaciones ILIKE $1))
               AND ($2::date IS NULL OR fecha >= $2)
               AND ($3::date IS NULL OR fecha <= $3)
               AND ($4::text IS NULL OR folio ILIKE $4)
               AND ($5::text IS NULL OR curp ILIKE $5)
               AND ($6::text IS NULL OR nombre ILIKE $6)
               AND ($7::text IS NULL OR genero ILIKE $7)
               AND ($8::text IS NULL OR municipio ILIKE $8)
               AND ($9::smallint IS NULL OR edad >= $9)
               AND ($10::smallint IS NULL OR edad <= $10)
               AND ($11::text IS NULL OR EXISTS (
                   SELECT 1 FROM jsonb_array_elements(campos_extra) AS campo_extra
                   WHERE campo_extra->>'nombre' ILIKE $11
                     AND ($12::text IS NULL OR campo_extra->>'valor' ILIKE $12)
               ))
             ORDER BY fecha DESC, folio DESC
             LIMIT $13 OFFSET $14",
        )
        .bind(&buscar)
        .bind(filtro.fecha_desde)
        .bind(filtro.fecha_hasta)
        .bind(&folio)
        .bind(&curp)
        .bind(&nombre)
        .bind(&genero)
        .bind(&municipio)
        .bind(filtro.edad_min)
        .bind(filtro.edad_max)
        .bind(&campo)
        .bind(&valor)
        .bind(tamano)
        .bind(offset)
        .fetch_all(&app.db)
        .await?;

    let registros: Vec<Value> = filas
        .into_iter()
        .map(|(id, fecha, folio, curp, nombre, genero, municipio, edad, observaciones, extra, documento)| {
            json!({
                "id": id,
                "fecha": fecha,
                "folio": folio,
                "curp": curp,
                "nombre": nombre,
                "genero": genero,
                "municipio": municipio,
                "edad": edad,
                "observaciones": observaciones,
                "campos_extra": extra,
                "documento_nombre": documento,
            })
        })
        .collect();

    Ok(Json(json!({
        "registros": registros,
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
    user.exigir("pa:leer")?;
    Ok(Json(consultar(&app, id).await?.json()))
}

pub async fn crear(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<PaInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pa:crear")?;
    let datos = validar(input)?;
    let mut tx = app.db.begin().await?;

    let anio = datos.fecha.year() as i16;
    let consecutivo: i32 = sqlx::query_scalar(
        "INSERT INTO pa_folios(anio, ultimo) VALUES ($1, 1)
         ON CONFLICT (anio) DO UPDATE SET ultimo = pa_folios.ultimo + 1
         RETURNING ultimo",
    )
    .bind(anio)
    .fetch_one(&mut *tx)
    .await?;
    let folio = format!("PA-{anio}-{consecutivo:04}");
    let documento = generar_pdf(&folio, &datos, &user.nombre)?;
    let documento_nombre = format!("{folio}.pdf");
    let extra = serde_json::to_value(&datos.campos_extra).map_err(|error| ApiError::interno(error.to_string()))?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO pa_registros
            (fecha, anio, folio, curp, nombre, genero, municipio, edad, observaciones,
             campos_extra, documento, documento_nombre, creado_por, actualizado_por)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$13)
         RETURNING id",
    )
    .bind(datos.fecha)
    .bind(anio)
    .bind(&folio)
    .bind(&datos.curp)
    .bind(&datos.nombre)
    .bind(&datos.genero)
    .bind(&datos.municipio)
    .bind(datos.edad)
    .bind(&datos.observaciones)
    .bind(&extra)
    .bind(&documento)
    .bind(&documento_nombre)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "pa.crear",
        "pa_registros",
        id,
        "EXITO",
        json!({ "folio": folio, "curp": datos.curp }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(consultar(&app, id).await?.json()))
}

pub async fn actualizar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<PaInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pa:editar")?;
    let datos = validar(input)?;
    let mut tx = app.db.begin().await?;

    let existente: Option<(String, i16)> = sqlx::query_as(
        "SELECT folio, anio FROM pa_registros WHERE id = $1 AND eliminado_en IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((folio, _anio)) = existente else {
        return Err(ApiError::no_encontrado("Registro P.A. no encontrado"));
    };

    let documento = generar_pdf(&folio, &datos, &user.nombre)?;
    let documento_nombre = format!("{folio}.pdf");
    let extra = serde_json::to_value(&datos.campos_extra).map_err(|error| ApiError::interno(error.to_string()))?;

    sqlx::query(
        "UPDATE pa_registros
         SET fecha=$2, curp=$3, nombre=$4, genero=$5, municipio=$6, edad=$7,
             observaciones=$8, campos_extra=$9, documento=$10, documento_nombre=$11,
             actualizado_por=$12, actualizado_en=now()
         WHERE id=$1",
    )
    .bind(id)
    .bind(datos.fecha)
    .bind(&datos.curp)
    .bind(&datos.nombre)
    .bind(&datos.genero)
    .bind(&datos.municipio)
    .bind(datos.edad)
    .bind(&datos.observaciones)
    .bind(&extra)
    .bind(&documento)
    .bind(&documento_nombre)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "pa.editar",
        "pa_registros",
        id,
        "EXITO",
        json!({ "folio": folio }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(consultar(&app, id).await?.json()))
}

pub async fn eliminar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("pa:eliminar")?;
    let mut tx = app.db.begin().await?;
    let folio: Option<String> = sqlx::query_scalar(
        "UPDATE pa_registros
         SET eliminado_en=now(), actualizado_por=$2, actualizado_en=now()
         WHERE id=$1 AND eliminado_en IS NULL
         RETURNING folio",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(folio) = folio else {
        return Err(ApiError::no_encontrado("Registro P.A. no encontrado"));
    };

    audit::registrar(
        &mut tx,
        Some(user.id),
        "pa.eliminar",
        "pa_registros",
        id,
        "EXITO",
        json!({ "folio": folio }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn documento(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    user.exigir("pa:leer")?;
    let fila: Option<(Vec<u8>, String)> = sqlx::query_as(
        "SELECT documento, documento_nombre FROM pa_registros
         WHERE id=$1 AND eliminado_en IS NULL",
    )
    .bind(id)
    .fetch_optional(&app.db)
    .await?;
    let Some((pdf, nombre)) = fila else {
        return Err(ApiError::no_encontrado("Documento P.A. no encontrado"));
    };

    let disposicion = format!("inline; filename=\"{}\"", nombre.replace('"', ""));
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("application/pdf"))
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposicion).map_err(|error| ApiError::interno(error.to_string()))?,
        )
        .body(Body::from(pdf))
        .map_err(|error| ApiError::interno(error.to_string()))
}

fn patron(valor: &Option<String>) -> Option<String> {
    valor
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(|valor| format!("%{valor}%"))
}

fn validar(input: PaInput) -> Result<DatosPa, ApiError> {
    let curp = input.curp.trim().to_uppercase();
    if curp.chars().count() != 18 || !curp.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(ApiError::inprocesable("La CURP debe tener 18 caracteres alfanuméricos"));
    }
    let nombre = input.nombre.trim().to_string();
    if nombre.is_empty() || nombre.chars().count() > 200 {
        return Err(ApiError::inprocesable("El nombre es obligatorio y admite hasta 200 caracteres"));
    }
    let genero = input.genero.trim().to_uppercase();
    if genero.is_empty() || genero.chars().count() > 40 {
        return Err(ApiError::inprocesable("El género es obligatorio"));
    }
    let municipio = input.municipio.trim().to_string();
    if municipio.is_empty() || municipio.chars().count() > 200 {
        return Err(ApiError::inprocesable("El municipio es obligatorio y admite hasta 200 caracteres"));
    }
    if !(0..=150).contains(&input.edad) {
        return Err(ApiError::inprocesable("La edad debe estar entre 0 y 150"));
    }
    if input.observaciones.chars().count() > 2000 {
        return Err(ApiError::inprocesable("Las observaciones admiten hasta 2000 caracteres"));
    }
    if input.campos_extra.len() > 30 {
        return Err(ApiError::inprocesable("El registro admite hasta 30 campos adicionales"));
    }
    let mut campos_extra = Vec::with_capacity(input.campos_extra.len());
    for campo in input.campos_extra {
        let nombre = campo.nombre.trim().to_string();
        if nombre.is_empty() || nombre.chars().count() > 80 {
            return Err(ApiError::inprocesable("Cada campo adicional requiere un nombre de hasta 80 caracteres"));
        }
        if campo.valor.chars().count() > 500 {
            return Err(ApiError::inprocesable("Cada valor adicional admite hasta 500 caracteres"));
        }
        campos_extra.push(CampoExtra { nombre, valor: campo.valor.trim().to_string() });
    }

    Ok(DatosPa {
        fecha: input.fecha,
        curp,
        nombre,
        genero,
        municipio,
        edad: input.edad,
        observaciones: input.observaciones.trim().to_string(),
        campos_extra,
    })
}

fn generar_pdf(folio: &str, datos: &DatosPa, actor: &str) -> Result<Vec<u8>, ApiError> {
    let fuente = FontData::new(include_bytes!("../../assets/NotoSans-Regular.ttf").to_vec(), None)
        .map_err(|error| ApiError::interno(format!("No se pudo cargar la fuente del PDF: {error}")))?;
    let familia = FontFamily {
        regular: fuente.clone(),
        bold: fuente.clone(),
        italic: fuente.clone(),
        bold_italic: fuente,
    };
    let mut documento = Document::new(familia);
    documento.set_title(format!("P.A. {folio}"));
    documento.set_font_size(11);
    documento.set_line_spacing(1.35);
    let mut decorador = SimplePageDecorator::new();
    decorador.set_margins(18);
    documento.set_page_decorator(decorador);

    documento.push(Paragraph::new("P.A. — CONSTANCIA DE REGISTRO").styled(Style::new().bold()));
    documento.push(Paragraph::new(format!("Folio: {folio}")));
    documento.push(Paragraph::new(""));
    documento.push(Paragraph::new(format!("Fecha: {}", datos.fecha.format("%d/%m/%Y"))));
    documento.push(Paragraph::new(format!("CURP: {}", datos.curp)));
    documento.push(Paragraph::new(format!("Nombre: {}", datos.nombre)));
    documento.push(Paragraph::new(format!("Género: {}", datos.genero)));
    documento.push(Paragraph::new(format!("Municipio donde radica: {}", datos.municipio)));
    documento.push(Paragraph::new(format!("Edad: {}", datos.edad)));
    documento.push(Paragraph::new(format!("Observaciones: {}", datos.observaciones)));
    if !datos.campos_extra.is_empty() {
        documento.push(Paragraph::new("Campos adicionales:").styled(Style::new().bold()));
        for campo in &datos.campos_extra {
            documento.push(Paragraph::new(format!("{}: {}", campo.nombre, campo.valor)));
        }
    }
    documento.push(Paragraph::new(""));
    documento.push(Paragraph::new(format!(
        "Generado por {} el {}",
        actor,
        Utc::now().format("%d/%m/%Y %H:%M UTC")
    )));

    let mut pdf = Vec::new();
    documento
        .render(&mut pdf)
        .map_err(|error| ApiError::interno(format!("No se pudo generar el PDF: {error}")))?;
    Ok(pdf)
}

struct RegistroPa {
    valor: Value,
}

impl RegistroPa {
    fn json(self) -> Value {
        self.valor
    }
}

async fn consultar(app: &AppState, id: Uuid) -> Result<RegistroPa, ApiError> {
    let fila: Option<(Uuid, NaiveDate, String, String, String, String, String, i16, String, Value, String, chrono::DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT id, fecha, folio, curp, nombre, genero, municipio, edad,
                    observaciones, campos_extra, documento_nombre, actualizado_en
             FROM pa_registros WHERE id=$1 AND eliminado_en IS NULL",
        )
        .bind(id)
        .fetch_optional(&app.db)
        .await?;
    let Some((id, fecha, folio, curp, nombre, genero, municipio, edad, observaciones, extra, documento, actualizado)) = fila else {
        return Err(ApiError::no_encontrado("Registro P.A. no encontrado"));
    };
    Ok(RegistroPa {
        valor: json!({
            "id": id,
            "fecha": fecha,
            "folio": folio,
            "curp": curp,
            "nombre": nombre,
            "genero": genero,
            "municipio": municipio,
            "edad": edad,
            "observaciones": observaciones,
            "campos_extra": extra,
            "documento_nombre": documento,
            "actualizado_en": actualizado,
        }),
    })
}
