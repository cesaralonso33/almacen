use std::time::Duration;

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    api::darfes::{self, CAMPOS_DOCUMENTO, LIMITE_DOCUMENTO},
    api::entrada::ApiJson,
    audit,
    auth::AuthUser,
    error::ApiError,
    AppState,
};

const CLAVE_EXPEDIENTE: &str = "EXPEDIENTE";
const CLAVE_PAPERLESS: &str = "PAPERLESS";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegracionInput {
    pub url: String,
    #[serde(default)]
    pub token: Option<String>,
    pub activo: bool,
    #[serde(default)]
    pub config: Option<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsultaExpedienteInput {
    pub expediente: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaperlessInput {
    pub documento_id: String,
    pub campo: String,
}

struct IntegracionConfig {
    url: String,
    token: String,
}

pub async fn listar(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("admin:integraciones")?;
    if !user.es_administrador() {
        return Err(ApiError::prohibido(
            "Solo el administrador puede consultar la configuración de integraciones",
        ));
    }
    let filas = sqlx::query_as::<_, (String, String, bool, bool, Value, DateTime<Utc>)>(
        "SELECT clave, url, (token <> '') AS token_configurado, activo, config, actualizado_en
         FROM integraciones ORDER BY clave",
    )
    .fetch_all(&app.db)
    .await?;

    let integraciones: Vec<Value> = filas
        .into_iter()
        .map(|(clave, url, token_configurado, activo, config, actualizado_en)| {
            json!({
                "clave": clave,
                "url": url,
                "token_configurado": token_configurado,
                "activo": activo,
                "config": config,
                "actualizado_en": actualizado_en,
            })
        })
        .collect();

    Ok(Json(json!({ "integraciones": integraciones })))
}

pub async fn guardar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(clave): Path<String>,
    ApiJson(input): ApiJson<IntegracionInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("admin:integraciones")?;
    if !user.es_administrador() {
        return Err(ApiError::prohibido(
            "Solo el administrador puede modificar la configuración de integraciones",
        ));
    }
    let clave = clave.trim().to_uppercase();
    if clave != CLAVE_EXPEDIENTE && clave != CLAVE_PAPERLESS {
        return Err(ApiError::no_encontrado("Integración no encontrada"));
    }

    let url = input.url.trim().trim_end_matches('/').to_string();
    if !url.is_empty() {
        let analizada = url::Url::parse(&url)
            .map_err(|_| ApiError::inprocesable("La URL de la integración no es válida"))?;
        if !matches!(analizada.scheme(), "http" | "https") {
            return Err(ApiError::inprocesable(
                "La URL de la integración debe usar http o https",
            ));
        }
        if url.chars().count() > 500 {
            return Err(ApiError::inprocesable("La URL es demasiado larga"));
        }
    }

    let token = input
        .token
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty());

    let fila = sqlx::query_as::<_, (String, String, bool, bool, Value, DateTime<Utc>)>(
        "UPDATE integraciones
         SET url = $2,
             token = CASE WHEN $3::text IS NULL THEN token ELSE $3 END,
             activo = $4,
             config = COALESCE($5, config),
             actualizado_en = now(),
             actualizado_por = $6
         WHERE clave = $1
         RETURNING clave, url, (token <> '') AS token_configurado, activo, config, actualizado_en",
    )
    .bind(&clave)
    .bind(&url)
    .bind(token)
    .bind(input.activo)
    .bind(&input.config)
    .bind(user.id)
    .fetch_optional(&app.db)
    .await?;

    let Some((clave, url, token_configurado, activo, config, actualizado_en)) = fila else {
        return Err(ApiError::no_encontrado("Integración no encontrada"));
    };

    let mut tx = app.db.begin().await?;
    audit::registrar(
        &mut tx,
        Some(user.id),
        "admin.integraciones.guardar",
        "integraciones",
        Uuid::nil(),
        "EXITO",
        json!({
            "clave": clave,
            "url": url,
            "activo": activo,
            "token_configurado": token_configurado,
        }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(json!({
        "clave": clave,
        "url": url,
        "token_configurado": token_configurado,
        "activo": activo,
        "config": config,
        "actualizado_en": actualizado_en,
    })))
}

async fn configurar(app: &AppState, clave: &str) -> Result<IntegracionConfig, ApiError> {
    let fila: Option<(String, String, bool)> = sqlx::query_as(
        "SELECT url, token, activo FROM integraciones WHERE clave = $1",
    )
    .bind(clave)
    .fetch_optional(&app.db)
    .await?;

    let Some((url, token, activo)) = fila else {
        return Err(ApiError::conflicto(
            "La integración no está configurada. Contacte al administrador.",
        ));
    };
    if !activo || url.trim().is_empty() {
        return Err(ApiError::conflicto(
            "La integración no está activa. Contacte al administrador.",
        ));
    }

    Ok(IntegracionConfig {
        url: url.trim().trim_end_matches('/').to_string(),
        token,
    })
}

pub async fn consultar_expediente(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<ConsultaExpedienteInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:leer")?;
    let numero = input.expediente.trim().to_string();
    if numero.is_empty() || numero.chars().count() > 100 {
        return Err(ApiError::inprocesable(
            "Capture un número de expediente válido",
        ));
    }

    let integracion = configurar(&app, CLAVE_EXPEDIENTE).await?;
    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| ApiError::interno(format!("No se pudo preparar la consulta: {error}")))?;

    let respuesta = cliente
        .post(&integracion.url)
        .bearer_auth(&integracion.token)
        .json(&json!({ "expediente": numero }))
        .send()
        .await
        .map_err(|error| {
            tracing::error!(?error, "falló la conexión con la API de expedientes");
            ApiError::interno("No se pudo conectar con la API de expedientes")
        })?;

    let estado = respuesta.status();
    if !estado.is_success() {
        let cuerpo = respuesta.text().await.unwrap_or_default();
        tracing::warn!(%estado, respuesta = %cuerpo, "respuesta de la API de expedientes");
        return Err(ApiError::interno(format!(
            "La API de expedientes respondió con HTTP {}",
            estado.as_u16()
        )));
    }

    let datos: Value = respuesta.json().await.map_err(|error| {
        tracing::error!(?error, "respuesta inválida de la API de expedientes");
        ApiError::interno("La API de expedientes devolvió una respuesta no válida")
    })?;

    let texto = |clave: &str| {
        datos
            .get(clave)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string)
    };
    let nombre = texto("nombre");
    let correo = texto("correo").or_else(|| texto("correo2"));
    let telefono = texto("tel1").or_else(|| texto("tel2"));
    let curp = texto("curp");
    let expediente = texto("expediente").unwrap_or_else(|| numero.clone());

    Ok(Json(json!({
        "expediente": expediente,
        "nombre": nombre,
        "correo": correo,
        "telefono": telefono,
        "curp": curp,
        "curp_disponible": curp.is_some(),
        "idfud": datos.get("idfud").cloned().unwrap_or(Value::Null),
    })))
}

pub async fn probar_paperless(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("admin:integraciones")?;
    if !user.es_administrador() {
        return Err(ApiError::prohibido(
            "Solo el administrador puede probar la configuración de Paperless",
        ));
    }
    let integracion = configurar(&app, CLAVE_PAPERLESS).await?;
    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| ApiError::interno(format!("No se pudo preparar la prueba: {error}")))?;

    let respuesta = cliente
        .get(format!("{}/api/documents/?page_size=1", integracion.url))
        .header(
            "Authorization",
            format!("Token {}", integracion.token),
        )
        .send()
        .await
        .map_err(|error| {
            tracing::error!(?error, "falló la conexión con Paperless");
            ApiError::interno("No se pudo conectar con Paperless-ngx")
        })?;

    if !respuesta.status().is_success() {
        tracing::warn!(estado = %respuesta.status(), "Paperless rechazó la prueba");
        return Err(ApiError::interno(format!(
            "Paperless-ngx respondió con HTTP {}",
            respuesta.status().as_u16()
        )));
    }

    Ok(Json(json!({
        "ok": true,
        "mensaje": "Conexión con Paperless-ngx correcta"
    })))
}

pub async fn importar_paperless(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<PaperlessInput>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("darfes:documentos")?;

    let campo = input.campo.trim().to_uppercase();
    if !CAMPOS_DOCUMENTO.contains(&campo.as_str()) {
        return Err(ApiError::inprocesable("Campo de documento no permitido"));
    }

    let documento_id = input.documento_id.trim();
    if documento_id.is_empty()
        || documento_id.len() > 20
        || !documento_id.chars().all(|caracter| caracter.is_ascii_digit())
    {
        return Err(ApiError::inprocesable(
            "El ID del documento de Paperless debe ser numérico",
        ));
    }

    let datos_darfe: Option<(String, f64, String)> = sqlx::query_as(
        "SELECT numero_cuenta, monto::float8, nombre FROM darfes
         WHERE id = $1 AND eliminado_en IS NULL",
    )
    .bind(id)
    .fetch_optional(&app.db)
    .await?;
    let Some((numero_cuenta, monto, nombre_darfe)) = datos_darfe else {
        return Err(ApiError::no_encontrado("DARFE no encontrado"));
    };

    let integracion = configurar(&app, CLAVE_PAPERLESS).await?;
    let cliente = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| ApiError::interno(format!("No se pudo preparar la importación: {error}")))?;

    let respuesta = cliente
        .get(format!(
            "{}/api/documents/{documento_id}/download/",
            integracion.url
        ))
        .header(
            "Authorization",
            format!("Token {}", integracion.token),
        )
        .send()
        .await
        .map_err(|error| {
            tracing::error!(?error, "falló la conexión con Paperless");
            ApiError::interno("No se pudo conectar con Paperless-ngx")
        })?;

    if !respuesta.status().is_success() {
        tracing::warn!(
            estado = %respuesta.status(),
            "Paperless rechazó la descarga del documento"
        );
        return Err(ApiError::interno(format!(
            "Paperless-ngx respondió con HTTP {}",
            respuesta.status().as_u16()
        )));
    }

    if let Some(tamano) = respuesta.content_length() {
        if tamano > LIMITE_DOCUMENTO as u64 {
            return Err(ApiError::inprocesable(
                "El documento de Paperless excede el límite de 8 MiB",
            ));
        }
    }

    let mime = respuesta
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let nombre = respuesta
        .headers()
        .get("content-disposition")
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.split("filename=\"").nth(1))
        .and_then(|valor| valor.split('"').next())
        .unwrap_or("documento-paperless.pdf")
        .replace('/', "_")
        .replace('\\', "_");

    let bytes = respuesta.bytes().await.map_err(|error| {
        tracing::error!(?error, "no se pudo leer el documento de Paperless");
        ApiError::interno("No se pudo leer el documento de Paperless")
    })?;
    if bytes.is_empty() || bytes.len() > LIMITE_DOCUMENTO {
        return Err(ApiError::inprocesable(
            "El documento de Paperless está vacío o excede el límite de 8 MiB",
        ));
    }

    let (datos_extraidos, coincide) = if campo == "DEPOSITO" {
        darfes::validar_deposito(&bytes, &numero_cuenta, monto, &nombre_darfe)
    } else {
        (Value::Null, None)
    };
    let contenido = bytes.to_vec();

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
    .bind(&nombre)
    .bind(&mime)
    .bind(bytes.len() as i32)
    .bind(&contenido)
    .bind(&datos_extraidos)
    .bind(coincide)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "darfes.documento.paperless",
        "darfes",
        id,
        "EXITO",
        json!({
            "campo": campo,
            "documento_id": documento_id,
            "tamano": bytes.len(),
            "coincide": coincide,
        }),
    )
    .await?;
    tx.commit().await?;

    Ok(Json(json!({
        "campo": campo,
        "nombre": nombre,
        "tamano": bytes.len(),
        "coincide": coincide,
    })))
}
