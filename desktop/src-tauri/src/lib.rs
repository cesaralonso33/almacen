//! Comandos locales del escritorio: sesión, caché SQLite, outbox y sincronización.

mod api;
mod db;
mod seguridad;
mod sync;

use std::path::PathBuf;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::Manager;
use uuid::Uuid;

#[derive(Clone)]
struct Sesion {
    access_token: String,
    access_expira_en: DateTime<Utc>,
    refresh_token: String,
    refresh_expira_en: DateTime<Utc>,
    usuario_id: String,
    nombre: String,
}

struct EstadoApp {
    ruta_db: PathBuf,
    base_url: Mutex<String>,
    sesion: Mutex<Option<Sesion>>,
    refresco: tokio::sync::Mutex<()>,
}

#[derive(Serialize)]
struct SesionInfo {
    nombre: String,
    usuario_id: String,
    permisos: Vec<String>,
}

#[derive(Serialize)]
struct Expediente {
    id: String,
    numero: String,
    descripcion: String,
    estado: String,
    custodio_id: Option<String>,
    version: i64,
}

#[derive(Serialize)]
struct EstadoSync {
    estado: String,
    pendientes: i64,
    conflictos: i64,
    cursor: i64,
    ultima_sync: Option<String>,
    ultimo_error: Option<String>,
    usuario: Option<String>,
}

fn abrir(estado: &EstadoApp) -> Result<rusqlite::Connection, String> {
    db::abrir(&estado.ruta_db).map_err(|error| error.to_string())
}

fn base_url(estado: &EstadoApp) -> Result<String, String> {
    estado
        .base_url
        .lock()
        .map(|valor| valor.clone())
        .map_err(|_| "configuración bloqueada".to_string())
}

fn sesion_actual(estado: &EstadoApp) -> Result<Sesion, String> {
    let guardia = estado.sesion.lock().map_err(|_| "sesión bloqueada".to_string())?;
    guardia
        .as_ref()
        .cloned()
        .ok_or_else(|| "No hay una sesión iniciada".to_string())
}

fn guardar_sesion(estado: &EstadoApp, sesion: Sesion) -> Result<(), String> {
    let mut guardia = estado.sesion.lock().map_err(|_| "sesión bloqueada".to_string())?;
    *guardia = Some(sesion);
    Ok(())
}

fn limpiar_sesion(estado: &EstadoApp) -> Result<(), String> {
    let mut guardia = estado.sesion.lock().map_err(|_| "sesión bloqueada".to_string())?;
    *guardia = None;
    Ok(())
}

fn fecha_campo(valor: &Value, campo: &str) -> Result<DateTime<Utc>, String> {
    valor
        .get(campo)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("La respuesta no incluye {campo}"))?
        .parse::<DateTime<Utc>>()
        .map_err(|_| format!("{campo} no tiene un formato de fecha válido"))
}

#[tauri::command]
async fn login(
    correo: String,
    password: String,
    estado: tauri::State<'_, EstadoApp>,
) -> Result<SesionInfo, String> {
    let base = base_url(&estado)?;
    let respuesta = api::ClienteHttp::login(&base, &correo, &password).await?;

    let access_token = respuesta
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "La respuesta no incluye access_token".to_string())?
        .to_string();
    // Compatibilidad con servidores anteriores que no informaban la expiración.
    let access_expira_en = fecha_campo(&respuesta, "access_expira_en")
        .unwrap_or_else(|_| Utc::now() + chrono::Duration::minutes(18 * 60 - 5));
    let refresh_token = respuesta
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "La respuesta no incluye refresh_token".to_string())?
        .to_string();
    let refresh_expira_en = fecha_campo(&respuesta, "refresh_expira_en")
        .unwrap_or_else(|_| Utc::now() + chrono::Duration::hours(23));
    let usuario_id = respuesta
        .pointer("/usuario/id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "La respuesta no incluye el usuario".to_string())?
        .to_string();
    let nombre = respuesta
        .pointer("/usuario/nombre")
        .and_then(|v| v.as_str())
        .unwrap_or("Usuario")
        .to_string();

    let conexion = abrir(&estado)?;
    let clave_meta = format!("dispositivo:{usuario_id}");
    let dispositivo = match db::leer_meta(&conexion, &clave_meta) {
        Some(dispositivo) => dispositivo,
        None => {
            let cliente = api::ClienteHttp::nuevo(&base, access_token.clone());
            let registro = cliente.registrar_dispositivo("Escritorio institucional").await?;
            let dispositivo = registro
                .get("dispositivo_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "No se pudo registrar el dispositivo".to_string())?
                .to_string();
            db::escribir_meta(&conexion, &clave_meta, &dispositivo)
                .map_err(|error| error.to_string())?;
            dispositivo
        }
    };
    db::escribir_meta(&conexion, "dispositivo_actual", &dispositivo)
        .map_err(|error| error.to_string())?;

    let permisos: Vec<String> = respuesta
        .get("permisos")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();

    guardar_sesion(
        &estado,
        Sesion {
            access_token,
            access_expira_en,
            refresh_token,
            refresh_expira_en,
            usuario_id: usuario_id.clone(),
            nombre: nombre.clone(),
        },
    )?;
    Ok(SesionInfo { nombre, usuario_id, permisos })
}

#[tauri::command]
async fn logout(estado: tauri::State<'_, EstadoApp>) -> Result<(), String> {
    if let (Ok(base), Ok(sesion)) = (base_url(&estado), sesion_actual(&estado)) {
        let token = token_actual(&estado, &base)
            .await
            .unwrap_or_else(|_| sesion.access_token.clone());
        let _ = reqwest::Client::new()
            .post(format!("{}/api/v1/auth/logout", base.trim_end_matches('/')))
            .bearer_auth(token)
            .send()
            .await;
    }
    limpiar_sesion(&estado)
}

#[tauri::command]
fn estado_sync(estado: tauri::State<'_, EstadoApp>) -> Result<EstadoSync, String> {
    let conexion = abrir(&estado)?;
    let (sync_estado, ultima_sync, ultimo_error): (String, Option<String>, Option<String>) = conexion
        .query_row(
            "SELECT estado, ultima_sync, ultimo_error FROM sync_status WHERE id = 1",
            [],
            |fila| Ok((fila.get(0)?, fila.get(1)?, fila.get(2)?)),
        )
        .map_err(|error| error.to_string())?;

    let pendientes: i64 = conexion
        .query_row("SELECT count(*) FROM outbox WHERE estado = 'PENDIENTE'", [], |fila| fila.get(0))
        .map_err(|error| error.to_string())?;
    let conflictos: i64 = conexion
        .query_row("SELECT count(*) FROM sync_conflicts WHERE resuelto_en IS NULL", [], |fila| fila.get(0))
        .map_err(|error| error.to_string())?;

    let usuario = estado
        .sesion
        .lock()
        .map_err(|_| "sesión bloqueada".to_string())?
        .as_ref()
        .map(|sesion| sesion.nombre.clone());

    Ok(EstadoSync {
        estado: sync_estado,
        pendientes,
        conflictos,
        cursor: db::cursor(&conexion),
        ultima_sync,
        ultimo_error,
        usuario,
    })
}

#[tauri::command]
async fn sincronizar(estado: tauri::State<'_, EstadoApp>) -> Result<serde_json::Value, String> {
    let base = base_url(&estado)?;
    let usuario_id = sesion_actual(&estado)?.usuario_id;
    let mut conexion = abrir(&estado)?;
    let dispositivo = db::leer_meta(&conexion, &format!("dispositivo:{usuario_id}"))
        .or_else(|| db::leer_meta(&conexion, "dispositivo_actual"))
        .ok_or_else(|| "El dispositivo no está registrado".to_string())?;

    let token = token_actual(&estado, &base).await?;
    let transporte = api::ClienteHttp::nuevo(&base, token.clone());
    let resumen = match sync::sincronizar(&mut conexion, &transporte, &dispositivo).await {
        Ok(resumen) => resumen,
        Err(error) if error.contains("401") => {
            let token_nuevo = renovar_sesion(&estado, &base, Some(&token)).await?;
            let transporte = api::ClienteHttp::nuevo(&base, token_nuevo);
            sync::sincronizar(&mut conexion, &transporte, &dispositivo).await?
        }
        Err(error) => return Err(error),
    };
    serde_json::to_value(resumen).map_err(|error| error.to_string())
}

#[tauri::command]
fn listar_expedientes(
    filtro: String,
    estado: tauri::State<'_, EstadoApp>,
) -> Result<Vec<Expediente>, String> {
    let conexion = abrir(&estado)?;
    let mut sentencia = conexion
        .prepare(
            "SELECT id, numero, descripcion, estado, custodio_id, version
             FROM expedientes
             WHERE numero LIKE ?1 OR descripcion LIKE ?1
             ORDER BY numero
             LIMIT 300",
        )
        .map_err(|error| error.to_string())?;

    let filas = sentencia
        .query_map(params![format!("%{}%", filtro)], |fila| {
            Ok(Expediente {
                id: fila.get(0)?,
                numero: fila.get(1)?,
                descripcion: fila.get(2)?,
                estado: fila.get(3)?,
                custodio_id: fila.get(4)?,
                version: fila.get(5)?,
            })
        })
        .map_err(|error| error.to_string())?;

    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

/// Crea un pedido local y lo deja en el outbox.
///
/// La UI debe mostrarlo como `PENDIENTE_DE_SINCRONIZAR`; no es un pedido oficial
/// hasta que el servidor lo acepta.
#[tauri::command]
fn crear_pedido_local(
    expedientes: Vec<String>,
    estado: tauri::State<'_, EstadoApp>,
) -> Result<String, String> {
    if expedientes.is_empty() {
        return Err("Seleccione al menos un expediente".to_string());
    }
    let usuario_id = sesion_actual(&estado)?.usuario_id;
    let mut conexion = abrir(&estado)?;
    let tx = conexion.transaction().map_err(|error| error.to_string())?;

    let pedido_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO pedidos (id, solicitante_id, estado, version)
         VALUES (?1, ?2, 'PENDIENTE_DE_SINCRONIZAR', 0)",
        params![pedido_id, usuario_id],
    )
    .map_err(|error| error.to_string())?;

    for expediente in &expedientes {
        tx.execute(
            "INSERT INTO pedido_items (id, pedido_id, expediente_id, estado)
             VALUES (?1, ?2, ?3, 'PENDIENTE_DE_SINCRONIZAR')",
            params![Uuid::new_v4().to_string(), pedido_id, expediente],
        )
        .map_err(|error| error.to_string())?;
    }

    sync::encolar(&tx, "CREAR_PEDIDO", &json!({ "expedientes": expedientes }))?;
    tx.commit().map_err(|error| error.to_string())?;

    Ok(pedido_id)
}

#[tauri::command]
fn listar_conflictos(estado: tauri::State<'_, EstadoApp>) -> Result<Vec<serde_json::Value>, String> {
    let conexion = abrir(&estado)?;
    let mut sentencia = conexion
        .prepare(
            "SELECT id, clave, tipo, motivo, payload, creado_en
             FROM sync_conflicts WHERE resuelto_en IS NULL ORDER BY creado_en DESC",
        )
        .map_err(|error| error.to_string())?;

    let filas = sentencia
        .query_map([], |fila| {
            Ok(json!({
                "id": fila.get::<_, String>(0)?,
                "clave": fila.get::<_, Option<String>>(1)?,
                "tipo": fila.get::<_, Option<String>>(2)?,
                "motivo": fila.get::<_, String>(3)?,
                "payload": fila.get::<_, Option<String>>(4)?,
                "creado_en": fila.get::<_, String>(5)?,
            }))
        })
        .map_err(|error| error.to_string())?;

    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

async fn token_actual(estado: &EstadoApp, base: &str) -> Result<String, String> {
    let sesion = sesion_actual(estado)?;
    if sesion.access_expira_en > Utc::now() + chrono::Duration::minutes(5) {
        return Ok(sesion.access_token);
    }
    renovar_sesion(estado, base, None).await
}

/// Renueva el refresh token de forma serializada. Varias peticiones que reciben
/// 401 al mismo tiempo comparten una sola rotación.
async fn renovar_sesion(
    estado: &EstadoApp,
    base: &str,
    token_fallido: Option<&str>,
) -> Result<String, String> {
    let _guardia = estado.refresco.lock().await;
    let sesion = sesion_actual(estado)?;

    if let Some(fallido) = token_fallido {
        if sesion.access_token != fallido && sesion.access_expira_en > Utc::now() {
            return Ok(sesion.access_token);
        }
    }

    if sesion.refresh_expira_en <= Utc::now() {
        limpiar_sesion(estado)?;
        return Err("SESION_EXPIRADA: la sesión expiró".to_string());
    }

    let respuesta = reqwest::Client::new()
        .post(format!("{}/api/v1/auth/refresh", base.trim_end_matches('/')))
        .json(&json!({ "refresh_token": sesion.refresh_token }))
        .send()
        .await
        .map_err(api::describir)?;

    let estado_http = respuesta.status();
    if !estado_http.is_success() {
        limpiar_sesion(estado)?;
        return Err(format!(
            "SESION_EXPIRADA: no fue posible renovar la sesión (HTTP {estado_http})"
        ));
    }

    let cuerpo: Value = respuesta.json().await.map_err(|error| error.to_string())?;
    let access_token = cuerpo
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "La renovación no incluye access_token".to_string())?
        .to_string();
    let refresh_token = cuerpo
        .get("refresh_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "La renovación no incluye refresh_token".to_string())?
        .to_string();
    let access_expira_en = fecha_campo(&cuerpo, "access_expira_en")
        .unwrap_or_else(|_| Utc::now() + chrono::Duration::minutes(18 * 60 - 5));
    let refresh_expira_en = fecha_campo(&cuerpo, "refresh_expira_en")
        .unwrap_or_else(|_| Utc::now() + chrono::Duration::hours(23));

    guardar_sesion(
        estado,
        Sesion {
            access_token: access_token.clone(),
            access_expira_en,
            refresh_token,
            refresh_expira_en,
            usuario_id: sesion.usuario_id,
            nombre: sesion.nombre,
        },
    )?;
    Ok(access_token)
}

async fn enviar_api(
    base: &str,
    metodo: &str,
    ruta: &str,
    cuerpo: Option<Value>,
    token: &str,
) -> Result<(reqwest::StatusCode, String), String> {
    let cliente = reqwest::Client::new();
    let url = format!("{}{}", base.trim_end_matches('/'), ruta);
    let constructor = match metodo.to_ascii_uppercase().as_str() {
        "GET" => cliente.get(&url),
        "POST" => cliente.post(&url),
        "PUT" => cliente.put(&url),
        "DELETE" => cliente.delete(&url),
        otro => return Err(format!("Método no permitido: {otro}")),
    };

    let mut solicitud = constructor.bearer_auth(token);
    if let Some(cuerpo) = cuerpo {
        solicitud = solicitud.json(&cuerpo);
    }

    let respuesta = solicitud.send().await.map_err(api::describir)?;
    let estado_http = respuesta.status();
    let texto = respuesta.text().await.map_err(|error| error.to_string())?;
    Ok((estado_http, texto))
}

/// Proxy autenticado hacia la API. La interfaz nunca maneja el token directamente.
#[tauri::command]
async fn peticion(
    metodo: String,
    ruta: String,
    cuerpo: Option<serde_json::Value>,
    estado: tauri::State<'_, EstadoApp>,
) -> Result<serde_json::Value, String> {
    if !ruta.starts_with("/api/v1/") || ruta.contains("..") {
        return Err("Ruta no permitida".to_string());
    }

    let base = base_url(&estado)?;
    let token = token_actual(&estado, &base).await?;
    let (mut estado_http, mut texto) =
        enviar_api(&base, &metodo, &ruta, cuerpo.clone(), &token).await?;

    if estado_http == reqwest::StatusCode::UNAUTHORIZED {
        let token_nuevo = renovar_sesion(&estado, &base, Some(&token)).await?;
        (estado_http, texto) =
            enviar_api(&base, &metodo, &ruta, cuerpo, &token_nuevo).await?;
    }

    if !estado_http.is_success() {
        let mensaje = serde_json::from_str::<Value>(&texto)
            .ok()
            .and_then(|valor| {
                valor
                    .pointer("/error/message")
                    .and_then(|m| m.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("HTTP {estado_http}"));
        if estado_http == reqwest::StatusCode::UNAUTHORIZED {
            limpiar_sesion(&estado)?;
            return Err(format!("SESION_EXPIRADA: {mensaje}"));
        }
        return Err(mensaje);
    }

    serde_json::from_str(&texto).map_err(|error| error.to_string())
}

#[tauri::command]
async fn guardar_documento_pa(
    id: String,
    app: tauri::AppHandle,
    estado: tauri::State<'_, EstadoApp>,
) -> Result<String, String> {
    Uuid::parse_str(&id).map_err(|_| "Identificador P.A. inválido".to_string())?;
    let base = base_url(&estado)?;
    let token = token_actual(&estado, &base).await?;
    let respuesta = reqwest::Client::new()
        .get(format!("{}/api/v1/pa/{id}/documento", base.trim_end_matches('/')))
        .bearer_auth(token)
        .send()
        .await
        .map_err(api::describir)?;

    if !respuesta.status().is_success() {
        return Err(format!("No se pudo obtener el documento (HTTP {})", respuesta.status()));
    }

    let nombre = respuesta
        .headers()
        .get("content-disposition")
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.split("filename=\"").nth(1))
        .and_then(|valor| valor.split('"').next())
        .unwrap_or("documento-pa.pdf")
        .replace('/', "_")
        .replace('\\', "_");
    let bytes = respuesta.bytes().await.map_err(|error| error.to_string())?;
    let directorio = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&directorio).map_err(|error| error.to_string())?;
    let ruta = directorio.join(if nombre.ends_with(".pdf") {
        nombre
    } else {
        format!("{nombre}.pdf")
    });
    std::fs::write(&ruta, &bytes).map_err(|error| error.to_string())?;
    Ok(ruta.to_string_lossy().to_string())
}

async fn guardar_descarga(
    respuesta: reqwest::Response,
    app: &tauri::AppHandle,
    nombre_predeterminado: &str,
) -> Result<String, String> {
    if !respuesta.status().is_success() {
        return Err(format!("No se pudo obtener el archivo (HTTP {})", respuesta.status()));
    }

    let nombre = respuesta
        .headers()
        .get("content-disposition")
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.split("filename=\"").nth(1))
        .and_then(|valor| valor.split('"').next())
        .unwrap_or(nombre_predeterminado)
        .replace('/', "_")
        .replace('\\', "_");
    let bytes = respuesta.bytes().await.map_err(|error| error.to_string())?;
    let directorio = app
        .path()
        .download_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&directorio).map_err(|error| error.to_string())?;
    let ruta = directorio.join(nombre);
    std::fs::write(&ruta, &bytes).map_err(|error| error.to_string())?;
    Ok(ruta.to_string_lossy().to_string())
}

#[tauri::command]
async fn guardar_documento_darfe(
    id: String,
    campo: String,
    app: tauri::AppHandle,
    estado: tauri::State<'_, EstadoApp>,
) -> Result<String, String> {
    Uuid::parse_str(&id).map_err(|_| "Identificador DARFE inválido".to_string())?;
    let campo = campo.trim().to_uppercase();
    if ![
        "CURP",
        "RFC",
        "REGISTRO",
        "IDENTIFICACION",
        "ESTADO_CUENTA",
        "DEPOSITO",
        "ESCRITO_LIBRE",
    ]
    .contains(&campo.as_str())
    {
        return Err("Campo de documento no permitido".to_string());
    }
    let base = base_url(&estado)?;
    let token = token_actual(&estado, &base).await?;
    let respuesta = reqwest::Client::new()
        .get(format!(
            "{}/api/v1/darfes/{id}/documentos/{campo}",
            base.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(api::describir)?;
    guardar_descarga(respuesta, &app, &format!("DARFE-{id}-{campo}.pdf")).await
}

#[tauri::command]
async fn guardar_exportacion_darfes(
    consulta: String,
    app: tauri::AppHandle,
    estado: tauri::State<'_, EstadoApp>,
) -> Result<String, String> {
    if (!consulta.is_empty() && !consulta.starts_with('?')) || consulta.contains("..") {
        return Err("Filtros de exportación no válidos".to_string());
    }
    let base = base_url(&estado)?;
    let token = token_actual(&estado, &base).await?;
    let respuesta = reqwest::Client::new()
        .get(format!(
            "{}/api/v1/darfes/exportar{consulta}",
            base.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(api::describir)?;
    guardar_descarga(respuesta, &app, "DARFES.xlsx").await
}

#[tauri::command]
async fn guardar_formato_darfe(
    id: String,
    app: tauri::AppHandle,
    estado: tauri::State<'_, EstadoApp>,
) -> Result<String, String> {
    Uuid::parse_str(&id).map_err(|_| "Identificador DARFE inválido".to_string())?;
    let base = base_url(&estado)?;
    let token = token_actual(&estado, &base).await?;
    let respuesta = reqwest::Client::new()
        .post(format!(
            "{}/api/v1/darfes/{id}/formato",
            base.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(api::describir)?;
    guardar_descarga(respuesta, &app, &format!("Recibo-{id}.pdf")).await
}

#[tauri::command]
fn servidor_actual(estado: tauri::State<'_, EstadoApp>) -> Result<String, String> {
    base_url(&estado)
}

/// Permite configurar el servidor en la instalación sin recompilar el paquete.
#[tauri::command]
fn configurar_servidor(url: String, estado: tauri::State<'_, EstadoApp>) -> Result<(), String> {
    let limpio = url.trim().trim_end_matches('/').to_string();
    if !(limpio.starts_with("https://") || limpio.starts_with("http://")) {
        return Err("La dirección debe iniciar con https:// o http://".to_string());
    }
    if limpio.len() < 12 {
        return Err("La dirección del servidor no es válida".to_string());
    }

    let conexion = abrir(&estado)?;
    db::escribir_meta(&conexion, "base_url", &limpio).map_err(|error| error.to_string())?;

    let mut guardia = estado.base_url.lock().map_err(|_| "configuración bloqueada".to_string())?;
    *guardia = limpio;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let directorio = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directorio)?;
            let ruta_db = directorio.join("expedientes.sqlite");
            db::abrir(&ruta_db)?;

            let base_inicial = db::leer_meta(&db::abrir(&ruta_db)?, "base_url")
                .or_else(|| std::env::var("EXPEDIENTES_API_URL").ok())
                .unwrap_or_else(|| "https://129-121-117-177.sslip.io".to_string());

            app.manage(EstadoApp {
                ruta_db,
                base_url: Mutex::new(base_inicial),
                sesion: Mutex::new(None),
                refresco: tokio::sync::Mutex::new(()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            login,
            logout,
            estado_sync,
            sincronizar,
            listar_expedientes,
            crear_pedido_local,
            listar_conflictos,
            peticion,
            guardar_documento_pa,
            guardar_documento_darfe,
            guardar_exportacion_darfes,
            guardar_formato_darfe,
            servidor_actual,
            configurar_servidor
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar Tauri");
}
