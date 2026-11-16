//! Comandos locales del escritorio: sesión, caché SQLite, outbox y sincronización.

mod api;
mod db;
mod sync;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::params;
use serde::Serialize;
use serde_json::json;
use tauri::Manager;
use uuid::Uuid;

struct Sesion {
    access_token: String,
    usuario_id: String,
    nombre: String,
}

struct EstadoApp {
    ruta_db: PathBuf,
    base_url: String,
    sesion: Mutex<Option<Sesion>>,
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

fn sesion_activa(estado: &EstadoApp) -> Result<(String, String), String> {
    let guardia = estado.sesion.lock().map_err(|_| "sesión bloqueada".to_string())?;
    let sesion = guardia
        .as_ref()
        .ok_or_else(|| "No hay una sesión iniciada".to_string())?;
    Ok((sesion.access_token.clone(), sesion.usuario_id.clone()))
}

#[tauri::command]
async fn login(
    correo: String,
    password: String,
    estado: tauri::State<'_, EstadoApp>,
) -> Result<String, String> {
    let respuesta = api::ClienteHttp::login(&estado.base_url, &correo, &password).await?;

    let access_token = respuesta
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "La respuesta no incluye access_token".to_string())?
        .to_string();
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
            let cliente = api::ClienteHttp::nuevo(&estado.base_url, access_token.clone());
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

    let mut guardia = estado.sesion.lock().map_err(|_| "sesión bloqueada".to_string())?;
    *guardia = Some(Sesion { access_token, usuario_id, nombre: nombre.clone() });
    Ok(nombre)
}

#[tauri::command]
fn logout(estado: tauri::State<'_, EstadoApp>) -> Result<(), String> {
    let mut guardia = estado.sesion.lock().map_err(|_| "sesión bloqueada".to_string())?;
    *guardia = None;
    Ok(())
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
    let (token, usuario_id) = sesion_activa(&estado)?;
    let mut conexion = abrir(&estado)?;
    let dispositivo = db::leer_meta(&conexion, &format!("dispositivo:{usuario_id}"))
        .or_else(|| db::leer_meta(&conexion, "dispositivo_actual"))
        .ok_or_else(|| "El dispositivo no está registrado".to_string())?;

    let transporte = api::ClienteHttp::nuevo(&estado.base_url, token);
    let resumen = sync::sincronizar(&mut conexion, &transporte, &dispositivo).await?;
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
    let (_, usuario_id) = sesion_activa(&estado)?;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let directorio = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directorio)?;
            let ruta_db = directorio.join("expedientes.sqlite");
            db::abrir(&ruta_db)?;

            let base_url = std::env::var("EXPEDIENTES_API_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());

            app.manage(EstadoApp {
                ruta_db,
                base_url,
                sesion: Mutex::new(None),
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
            listar_conflictos
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar Tauri");
}
