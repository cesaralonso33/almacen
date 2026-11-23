//! Motor de sincronización del escritorio.
//!
//! Reglas que se respetan en todo el archivo:
//! - El cursor local solo avanza dentro de la misma transacción que aplica los cambios.
//! - El outbox nunca envía dos veces la misma clave; el servidor es idempotente por (clave, dispositivo).
//! - Un rechazo del servidor se registra como conflicto visible para resolución humana.
//! - Ninguna confirmación de custodia, entrega, devolución o permiso se acepta sin servidor.

use std::collections::HashMap;

use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db;

/// Transporte HTTP hacia el servidor. Se abstrae para poder probar el motor sin red.
#[async_trait::async_trait]
pub trait Transporte: Send + Sync {
    async fn pull(&self, dispositivo: &str, cursor: i64, limite: i64) -> Result<Value, String>;
    async fn push(&self, dispositivo: &str, operaciones: Vec<Value>) -> Result<Value, String>;
    async fn snapshot(&self, dispositivo: &str) -> Result<Value, String>;
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize)]
pub struct Resumen {
    pub eventos_aplicados: usize,
    pub operaciones_enviadas: usize,
    pub operaciones_aceptadas: usize,
    pub operaciones_rechazadas: usize,
    pub operaciones_con_error: usize,
    pub reconstruyo_snapshot: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OperacionPendiente {
    pub id: String,
    pub clave: String,
    pub tipo: String,
    pub payload: Value,
    pub intentos: i64,
}

/// Aplica un snapshot completo y fija el cursor en una sola transacción.
pub fn aplicar_snapshot(conexion: &mut Connection, snapshot: &Value) -> Result<(), String> {
    let cursor = snapshot
        .get("cursor")
        .and_then(Value::as_i64)
        .ok_or_else(|| "el snapshot no incluye cursor".to_string())?;

    let tx = conexion.transaction().map_err(error_texto)?;

    tx.execute("DELETE FROM pedido_items", []).map_err(error_texto)?;
    tx.execute("DELETE FROM pedidos", []).map_err(error_texto)?;
    tx.execute("DELETE FROM entregas", []).map_err(error_texto)?;
    tx.execute("DELETE FROM expedientes", []).map_err(error_texto)?;

    if let Some(expedientes) = snapshot.get("expedientes").and_then(Value::as_array) {
        for expediente in expedientes {
            tx.execute(
                "INSERT INTO expedientes
                    (id, numero, descripcion, estado, ubicacion_id, custodio_id, version, actualizado_en)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    texto(expediente, "id")?,
                    texto(expediente, "numero")?,
                    expediente.get("descripcion").and_then(Value::as_str).unwrap_or(""),
                    texto(expediente, "estado")?,
                    opcional(expediente, "ubicacion_id"),
                    opcional(expediente, "custodio_id"),
                    entero(expediente, "version").unwrap_or(0),
                    opcional(expediente, "actualizado_en"),
                ],
            )
            .map_err(error_texto)?;
        }
    }

    if let Some(pedidos) = snapshot.get("pedidos").and_then(Value::as_array) {
        for pedido in pedidos {
            let pedido_id = texto(pedido, "id")?;
            tx.execute(
                "INSERT INTO pedidos (id, solicitante_id, estado, cancelado_en, version)
                 VALUES (?1,?2,?3,?4,?5)",
                params![
                    pedido_id,
                    opcional(pedido, "solicitante_id"),
                    texto(pedido, "estado")?,
                    opcional(pedido, "cancelado_en"),
                    entero(pedido, "version").unwrap_or(0),
                ],
            )
            .map_err(error_texto)?;

            if let Some(items) = pedido.get("items").and_then(Value::as_array) {
                for item in items {
                    tx.execute(
                        "INSERT INTO pedido_items (id, pedido_id, expediente_id, estado, version)
                         VALUES (?1,?2,?3,?4,?5)",
                        params![
                            texto(item, "id")?,
                            pedido_id,
                            opcional(item, "expediente_id"),
                            texto(item, "estado")?,
                            entero(item, "version").unwrap_or(0),
                        ],
                    )
                    .map_err(error_texto)?;
                }
            }
        }
    }

    db::escribir_meta(&tx, "cursor", &cursor.to_string()).map_err(error_texto)?;
    tx.commit().map_err(error_texto)?;
    Ok(())
}

/// Aplica un lote y avanza el cursor en la misma transacción.
///
/// Si cualquier evento es inválido, se revierte todo y el cursor queda donde estaba.
pub fn aplicar_eventos(
    conexion: &mut Connection,
    eventos: &[Value],
    siguiente_cursor: i64,
) -> Result<(), String> {
    let tx = conexion.transaction().map_err(error_texto)?;

    for evento in eventos {
        aplicar_evento(&tx, evento)?;
    }

    db::escribir_meta(&tx, "cursor", &siguiente_cursor.to_string()).map_err(error_texto)?;
    tx.commit().map_err(error_texto)?;
    Ok(())
}

fn aplicar_evento(tx: &Connection, evento: &Value) -> Result<(), String> {
    let entidad = evento
        .get("entidad")
        .and_then(Value::as_str)
        .ok_or_else(|| "evento sin entidad".to_string())?;
    let entidad_id = evento
        .get("entidad_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "evento sin entidad_id".to_string())?;
    let operacion = evento
        .get("operacion")
        .and_then(Value::as_str)
        .ok_or_else(|| "evento sin operación".to_string())?;
    let payload = evento.get("payload").cloned().unwrap_or(Value::Null);

    if operacion == "ELIMINAR" || evento.get("tombstone").and_then(Value::as_bool) == Some(true) {
        return borrar_entidad(tx, entidad, entidad_id);
    }

    match entidad {
        "expediente" => {
            tx.execute(
                "INSERT OR IGNORE INTO expedientes (id, numero, estado) VALUES (?1, ?2, ?3)",
                params![
                    entidad_id,
                    payload.get("numero").and_then(Value::as_str).unwrap_or(""),
                    payload.get("estado").and_then(Value::as_str).unwrap_or("DESCONOCIDO"),
                ],
            )
            .map_err(error_texto)?;
            tx.execute(
                "UPDATE expedientes
                 SET estado = COALESCE(?2, estado),
                     version = MAX(version, ?3),
                     sincronizado_en = datetime('now')
                 WHERE id = ?1",
                params![
                    entidad_id,
                    payload.get("estado").and_then(Value::as_str),
                    payload.get("version").and_then(Value::as_i64).unwrap_or(0),
                ],
            )
            .map_err(error_texto)?;
            if payload.get("custodio_id").is_some() {
                tx.execute(
                    "UPDATE expedientes SET custodio_id = ?2 WHERE id = ?1",
                    params![entidad_id, payload.get("custodio_id").and_then(Value::as_str)],
                )
                .map_err(error_texto)?;
            }
            if let Some(numero) = payload.get("numero").and_then(Value::as_str) {
                tx.execute(
                    "UPDATE expedientes SET numero = ?2 WHERE id = ?1",
                    params![entidad_id, numero],
                )
                .map_err(error_texto)?;
            }
            if let Some(descripcion) = payload.get("descripcion").and_then(Value::as_str) {
                tx.execute(
                    "UPDATE expedientes SET descripcion = ?2 WHERE id = ?1",
                    params![entidad_id, descripcion],
                )
                .map_err(error_texto)?;
            }
            if let Some(ubicacion) = payload.get("ubicacion_id").and_then(Value::as_str) {
                tx.execute(
                    "UPDATE expedientes SET ubicacion_id = ?2 WHERE id = ?1",
                    params![entidad_id, ubicacion],
                )
                .map_err(error_texto)?;
            }
        }
        "pedido" => {
            tx.execute(
                "INSERT OR IGNORE INTO pedidos (id, solicitante_id, estado, version)
                 VALUES (?1,?2,?3,?4)",
                params![
                    entidad_id,
                    payload.get("solicitante_id").and_then(Value::as_str),
                    payload.get("estado").and_then(Value::as_str).unwrap_or("PENDIENTE"),
                    payload.get("version").and_then(Value::as_i64).unwrap_or(0),
                ],
            )
            .map_err(error_texto)?;
            tx.execute(
                "UPDATE pedidos
                 SET estado = COALESCE(?2, estado),
                     version = MAX(version, ?3),
                     sincronizado_en = datetime('now')
                 WHERE id = ?1",
                params![
                    entidad_id,
                    payload.get("estado").and_then(Value::as_str),
                    payload.get("version").and_then(Value::as_i64).unwrap_or(0),
                ],
            )
            .map_err(error_texto)?;
            if payload.get("cancelado_en").is_some() {
                tx.execute(
                    "UPDATE pedidos SET cancelado_en = ?2 WHERE id = ?1",
                    params![entidad_id, payload.get("cancelado_en").and_then(Value::as_str)],
                )
                .map_err(error_texto)?;
            }
        }
        "pedido_item" => {
            tx.execute(
                "INSERT OR IGNORE INTO pedido_items (id, pedido_id, expediente_id, estado, version)
                 VALUES (?1,?2,?3,?4,?5)",
                params![
                    entidad_id,
                    payload.get("pedido_id").and_then(Value::as_str),
                    payload.get("expediente_id").and_then(Value::as_str),
                    payload.get("estado").and_then(Value::as_str).unwrap_or("PENDIENTE"),
                    payload.get("version").and_then(Value::as_i64).unwrap_or(0),
                ],
            )
            .map_err(error_texto)?;
            tx.execute(
                "UPDATE pedido_items
                 SET estado = COALESCE(?2, estado), sincronizado_en = datetime('now')
                 WHERE id = ?1",
                params![entidad_id, payload.get("estado").and_then(Value::as_str)],
            )
            .map_err(error_texto)?;
            if let Some(pedido_id) = payload.get("pedido_id").and_then(Value::as_str) {
                tx.execute(
                    "UPDATE pedido_items SET pedido_id = ?2 WHERE id = ?1",
                    params![entidad_id, pedido_id],
                )
                .map_err(error_texto)?;
            }
        }
        "entrega" => {
            tx.execute(
                "INSERT OR IGNORE INTO entregas (id, pedido_item_id, estado) VALUES (?1,?2,?3)",
                params![
                    entidad_id,
                    payload.get("pedido_item_id").and_then(Value::as_str),
                    payload.get("estado").and_then(Value::as_str).unwrap_or("DESPACHADA"),
                ],
            )
            .map_err(error_texto)?;
            tx.execute(
                "UPDATE entregas SET estado = COALESCE(?2, estado), sincronizado_en = datetime('now')
                 WHERE id = ?1",
                params![entidad_id, payload.get("estado").and_then(Value::as_str)],
            )
            .map_err(error_texto)?;
        }
        // Entidades no proyectadas se ignoran para permitir que el servidor evolucione.
        _ => {}
    }

    Ok(())
}

fn borrar_entidad(tx: &Connection, entidad: &str, id: &str) -> Result<(), String> {
    let tabla = match entidad {
        "expediente" => "expedientes",
        "pedido" => "pedidos",
        "pedido_item" => "pedido_items",
        "entrega" => "entregas",
        _ => return Ok(()),
    };
    tx.execute(&format!("DELETE FROM {tabla} WHERE id = ?1"), [id])
        .map_err(error_texto)?;
    Ok(())
}

/// Encola una operación local. La clave es única: reintentar no duplica.
pub fn encolar(conexion: &Connection, tipo: &str, payload: &Value) -> Result<String, String> {
    let clave = Uuid::new_v4().to_string();
    conexion
        .execute(
            "INSERT INTO outbox (id, clave, tipo, payload) VALUES (?1,?2,?3,?4)",
            params![Uuid::new_v4().to_string(), clave, tipo, payload.to_string()],
        )
        .map_err(error_texto)?;
    Ok(clave)
}

pub fn pendientes(conexion: &Connection, limite: i64) -> Result<Vec<OperacionPendiente>, String> {
    let mut sentencia = conexion
        .prepare(
            "SELECT id, clave, tipo, payload, intentos
             FROM outbox
             WHERE estado = 'PENDIENTE'
               AND proximo_intento <= strftime('%Y-%m-%d %H:%M:%S','now')
             ORDER BY creado_en
             LIMIT ?1",
        )
        .map_err(error_texto)?;

    let filas = sentencia
        .query_map([limite], |fila| {
            let payload: String = fila.get(3)?;
            Ok(OperacionPendiente {
                id: fila.get(0)?,
                clave: fila.get(1)?,
                tipo: fila.get(2)?,
                payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
                intentos: fila.get(4)?,
            })
        })
        .map_err(error_texto)?;

    filas.collect::<rusqlite::Result<Vec<_>>>().map_err(error_texto)
}

pub fn completar(conexion: &Connection, id: &str) -> Result<(), String> {
    conexion
        .execute(
            "UPDATE outbox SET estado = 'COMPLETADA', completado_en = datetime('now')
             WHERE id = ?1",
            [id],
        )
        .map_err(error_texto)?;
    Ok(())
}

pub fn marcar_rechazada(conexion: &Connection, id: &str, motivo: &str) -> Result<(), String> {
    let (clave, tipo, payload): (String, String, String) = conexion
        .query_row(
            "SELECT clave, tipo, payload FROM outbox WHERE id = ?1",
            [id],
            |fila| Ok((fila.get(0)?, fila.get(1)?, fila.get(2)?)),
        )
        .map_err(error_texto)?;

    conexion
        .execute(
            "UPDATE outbox SET estado = 'RECHAZADA', ultimo_error = ?2, completado_en = datetime('now')
             WHERE id = ?1",
            params![id, motivo],
        )
        .map_err(error_texto)?;

    conexion
        .execute(
            "INSERT INTO sync_conflicts (id, clave, tipo, motivo, payload)
             VALUES (?1,?2,?3,?4,?5)",
            params![Uuid::new_v4().to_string(), clave, tipo, motivo, payload],
        )
        .map_err(error_texto)?;

    Ok(())
}

pub fn marcar_error(conexion: &Connection, id: &str, error: &str) -> Result<(), String> {
    let intentos: i64 = conexion
        .query_row("SELECT intentos FROM outbox WHERE id = ?1", [id], |fila| fila.get(0))
        .map_err(error_texto)?;

    let intentos = intentos + 1;
    let proximo = proximo_intento(intentos);

    conexion
        .execute(
            "UPDATE outbox SET intentos = ?2, ultimo_error = ?3, proximo_intento = ?4
             WHERE id = ?1",
            params![id, intentos, error, proximo],
        )
        .map_err(error_texto)?;
    Ok(())
}

/// Retroceso exponencial con jitter acotado: 5 s, 10 s, 20 s… hasta 10 minutos.
pub fn proximo_intento(intentos: i64) -> String {
    let exponente = intentos.clamp(1, 7) as u32;
    let base = 5_i64 * 2_i64.pow(exponente - 1);
    let jitter = intentos % 4;
    let total = (base + jitter).min(600);
    (Utc::now() + chrono::Duration::seconds(total))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// Sincroniza una vez: pull incremental (o snapshot si el cursor ya no sirve) y push del outbox.
pub async fn sincronizar<T: Transporte>(
    conexion: &mut Connection,
    transporte: &T,
    dispositivo: &str,
) -> Result<Resumen, String> {
    let mut resumen = Resumen::default();

    match sincronizar_pull(conexion, transporte, dispositivo, &mut resumen).await {
        Ok(()) => {}
        Err(error) => {
            marcar_estado(conexion, "ERROR", Some(&error))?;
            return Err(error);
        }
    }

    if let Err(error) = sincronizar_push(conexion, transporte, dispositivo, &mut resumen).await {
        marcar_estado(conexion, "ERROR", Some(&error))?;
        return Err(error);
    }

    marcar_estado(conexion, "EN_LINEA", None)?;
    Ok(resumen)
}

async fn sincronizar_pull<T: Transporte>(
    conexion: &mut Connection,
    transporte: &T,
    dispositivo: &str,
    resumen: &mut Resumen,
) -> Result<(), String> {
    let mut cursor_actual = db::cursor(conexion);

    for _ in 0..50 {
        let respuesta = transporte.pull(dispositivo, cursor_actual, 200).await?;

        if respuesta.get("reiniciar").and_then(Value::as_bool) == Some(true) {
            let snapshot = transporte.snapshot(dispositivo).await?;
            aplicar_snapshot(conexion, &snapshot)?;
            resumen.reconstruyo_snapshot = true;
            break;
        }

        let eventos = respuesta
            .get("eventos")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let siguiente = respuesta
            .get("siguiente_cursor")
            .and_then(Value::as_i64)
            .unwrap_or(cursor_actual);

        if eventos.is_empty() {
            break;
        }

        aplicar_eventos(conexion, &eventos, siguiente)?;
        resumen.eventos_aplicados += eventos.len();
        cursor_actual = siguiente;

        if respuesta.get("hay_mas").and_then(Value::as_bool) != Some(true) {
            break;
        }
    }

    Ok(())
}

async fn sincronizar_push<T: Transporte>(
    conexion: &mut Connection,
    transporte: &T,
    dispositivo: &str,
    resumen: &mut Resumen,
) -> Result<(), String> {
    let pendientes = pendientes(conexion, 20)?;
    if pendientes.is_empty() {
        return Ok(());
    }

    let operaciones: Vec<Value> = pendientes
        .iter()
        .map(|operacion| {
            json!({
                "clave": operacion.clave,
                "tipo": operacion.tipo,
                "payload": operacion.payload,
            })
        })
        .collect();

    let respuesta = match transporte.push(dispositivo, operaciones).await {
        Ok(valor) => valor,
        Err(error) => {
            for operacion in &pendientes {
                marcar_error(conexion, &operacion.id, &error)?;
                resumen.operaciones_con_error += 1;
            }
            return Err(error);
        }
    };

    let mut resultados: HashMap<String, Value> = HashMap::new();
    if let Some(lista) = respuesta.get("resultados").and_then(Value::as_array) {
        for resultado in lista {
            if let Some(clave) = resultado.get("clave").and_then(Value::as_str) {
                resultados.insert(clave.to_string(), resultado.clone());
            }
        }
    }

    for operacion in &pendientes {
        resumen.operaciones_enviadas += 1;

        match resultados.get(&operacion.clave) {
            Some(resultado) if resultado.get("estado").and_then(Value::as_str) == Some("ACEPTADA") => {
                completar(conexion, &operacion.id)?;
                resumen.operaciones_aceptadas += 1;
            }
            Some(resultado) => {
                let motivo = resultado
                    .get("resultado")
                    .map(|valor| valor.to_string())
                    .unwrap_or_else(|| "operación rechazada por el servidor".to_string());
                marcar_rechazada(conexion, &operacion.id, &motivo)?;
                resumen.operaciones_rechazadas += 1;
            }
            None => {
                marcar_error(conexion, &operacion.id, "el servidor no devolvió resultado para la operación")?;
                resumen.operaciones_con_error += 1;
            }
        }
    }

    Ok(())
}

fn marcar_estado(conexion: &Connection, estado: &str, error: Option<&str>) -> Result<(), String> {
    let pendientes: i64 = conexion
        .query_row("SELECT count(*) FROM outbox WHERE estado = 'PENDIENTE'", [], |fila| fila.get(0))
        .map_err(error_texto)?;

    conexion
        .execute(
            "UPDATE sync_status
             SET estado = ?1, ultimo_error = ?2,
                 ultima_sync = CASE WHEN ?1 = 'EN_LINEA' THEN datetime('now') ELSE ultima_sync END,
                 pendientes = ?3
             WHERE id = 1",
            params![estado, error, pendientes],
        )
        .map_err(error_texto)?;
    Ok(())
}

fn texto(valor: &Value, campo: &str) -> Result<String, String> {
    valor
        .get(campo)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("el objeto no incluye {campo}"))
}

fn opcional(valor: &Value, campo: &str) -> Option<String> {
    valor.get(campo).and_then(Value::as_str).map(str::to_string)
}

fn entero(valor: &Value, campo: &str) -> Option<i64> {
    valor.get(campo).and_then(Value::as_i64)
}

fn error_texto(error: rusqlite::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct TransporteFalso {
        pulls: Mutex<VecDeque<Value>>,
        push: Mutex<Value>,
        snapshots: Mutex<VecDeque<Value>>,
        falla_push: Mutex<bool>,
    }

    impl TransporteFalso {
        fn nuevo() -> Self {
            Self {
                pulls: Mutex::new(VecDeque::new()),
                push: Mutex::new(json!({ "resultados": [] })),
                snapshots: Mutex::new(VecDeque::new()),
                falla_push: Mutex::new(false),
            }
        }

        fn con_pull(self, valor: Value) -> Self {
            self.pulls.lock().unwrap().push_back(valor);
            self
        }

        fn con_push(self, valor: Value) -> Self {
            *self.push.lock().unwrap() = valor;
            self
        }

        fn con_snapshot(self, valor: Value) -> Self {
            self.snapshots.lock().unwrap().push_back(valor);
            self
        }

        fn con_falla_de_push(self) -> Self {
            *self.falla_push.lock().unwrap() = true;
            self
        }
    }

    #[async_trait::async_trait]
    impl Transporte for TransporteFalso {
        async fn pull(&self, _: &str, _: i64, _: i64) -> Result<Value, String> {
            Ok(self
                .pulls
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| json!({ "eventos": [], "siguiente_cursor": 0, "hay_mas": false })))
        }

        async fn push(&self, _: &str, _: Vec<Value>) -> Result<Value, String> {
            if *self.falla_push.lock().unwrap() {
                return Err("sin conexión".to_string());
            }
            Ok(self.push.lock().unwrap().clone())
        }

        async fn snapshot(&self, _: &str) -> Result<Value, String> {
            Ok(self.snapshots.lock().unwrap().pop_front().unwrap_or_else(|| json!({ "cursor": 0 })))
        }
    }

    fn evento(entidad: &str, id: &str, operacion: &str, payload: Value) -> Value {
        json!({
            "secuencia": 1,
            "entidad": entidad,
            "entidad_id": id,
            "operacion": operacion,
            "version": 1,
            "tombstone": false,
            "payload": payload,
        })
    }

    #[test]
    fn snapshot_reemplaza_proyecciones_y_fija_cursor() {
        let mut conexion = db::abrir_en_memoria().unwrap();
        let snapshot = json!({
            "cursor": 42,
            "expedientes": [{
                "id": "e1", "numero": "EXP-1", "descripcion": "Demanda",
                "estado": "ALMACEN", "version": 3
            }],
            "pedidos": [{
                "id": "p1", "solicitante_id": "u1", "estado": "PENDIENTE", "version": 1,
                "items": [{ "id": "i1", "expediente_id": "e1", "estado": "PENDIENTE" }]
            }]
        });

        aplicar_snapshot(&mut conexion, &snapshot).unwrap();

        assert_eq!(db::cursor(&conexion), 42);
        let numero: String = conexion
            .query_row("SELECT numero FROM expedientes WHERE id = 'e1'", [], |f| f.get(0))
            .unwrap();
        assert_eq!(numero, "EXP-1");
        let items: i64 = conexion
            .query_row("SELECT count(*) FROM pedido_items", [], |f| f.get(0))
            .unwrap();
        assert_eq!(items, 1);
    }

    #[test]
    fn el_cursor_no_avanza_si_un_evento_es_invalido() {
        let mut conexion = db::abrir_en_memoria().unwrap();
        db::escribir_meta(&conexion, "cursor", "10").unwrap();

        let eventos = vec![
            evento("expediente", "e1", "CREAR", json!({ "estado": "ALMACEN" })),
            json!({ "entidad_id": "x", "operacion": "CREAR" }),
        ];

        let resultado = aplicar_eventos(&mut conexion, &eventos, 20);
        assert!(resultado.is_err());
        assert_eq!(db::cursor(&conexion), 10, "el cursor no debe adelantarse");
        let filas: i64 = conexion
            .query_row("SELECT count(*) FROM expedientes", [], |f| f.get(0))
            .unwrap();
        assert_eq!(filas, 0, "el lote completo debe revertirse");
    }

    #[test]
    fn un_evento_de_custodia_actualiza_el_expediente() {
        let mut conexion = db::abrir_en_memoria().unwrap();
        let eventos = vec![evento(
            "expediente",
            "e1",
            "ACTUALIZAR",
            json!({ "estado": "EN_CUSTODIA", "custodio_id": "u9", "version": 5 }),
        )];

        aplicar_eventos(&mut conexion, &eventos, 1).unwrap();

        let (estado, custodio, version): (String, Option<String>, i64) = conexion
            .query_row(
                "SELECT estado, custodio_id, version FROM expedientes WHERE id = 'e1'",
                [],
                |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?)),
            )
            .unwrap();
        assert_eq!(estado, "EN_CUSTODIA");
        assert_eq!(custodio.as_deref(), Some("u9"));
        assert_eq!(version, 5);
        assert_eq!(db::cursor(&conexion), 1);
    }

    #[test]
    fn un_rechazo_del_servidor_crea_conflicto_y_no_reenvia() {
        let conexion = db::abrir_en_memoria().unwrap();
        let clave = encolar(&conexion, "CREAR_PEDIDO", &json!({ "expedientes": ["e1"] })).unwrap();
        let pendiente = pendientes(&conexion, 10).unwrap().remove(0);
        assert_eq!(pendiente.clave, clave);

        marcar_rechazada(&conexion, &pendiente.id, "{\"code\":\"DATOS_INVALIDOS\"}").unwrap();

        assert!(pendientes(&conexion, 10).unwrap().is_empty());
        let conflictos: i64 = conexion
            .query_row("SELECT count(*) FROM sync_conflicts", [], |f| f.get(0))
            .unwrap();
        assert_eq!(conflictos, 1);
    }

    #[test]
    fn el_error_de_red_programa_reintento_con_intentos() {
        let conexion = db::abrir_en_memoria().unwrap();
        encolar(&conexion, "CREAR_PEDIDO", &json!({ "expedientes": ["e1"] })).unwrap();
        let pendiente = pendientes(&conexion, 10).unwrap().remove(0);

        marcar_error(&conexion, &pendiente.id, "sin conexión").unwrap();

        let (estado, intentos, proximo): (String, i64, String) = conexion
            .query_row(
                "SELECT estado, intentos, proximo_intento FROM outbox WHERE id = ?1",
                [&pendiente.id],
                |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?)),
            )
            .unwrap();
        assert_eq!(estado, "PENDIENTE");
        assert_eq!(intentos, 1);
        assert!(proximo.as_str() > Utc::now().format("%Y-%m-%d %H:%M:%S").to_string().as_str());
    }

    #[tokio::test]
    async fn pull_y_push_completan_el_ciclo() {
        let mut conexion = db::abrir_en_memoria().unwrap();
        encolar(&conexion, "CREAR_PEDIDO", &json!({ "expedientes": ["e1"] })).unwrap();
        let clave: String = conexion
            .query_row("SELECT clave FROM outbox", [], |f| f.get(0))
            .unwrap();

        let transporte = TransporteFalso::nuevo()
            .con_pull(json!({
                "eventos": [evento("expediente", "e1", "CREAR", json!({ "estado": "ALMACEN", "version": 1 }))],
                "siguiente_cursor": 1,
                "hay_mas": false
            }))
            .con_push(json!({ "resultados": [{ "clave": clave, "estado": "ACEPTADA", "resultado": { "pedido_id": "p1" } }] }));

        let resumen = sincronizar(&mut conexion, &transporte, "d1").await.unwrap();

        assert_eq!(resumen.eventos_aplicados, 1);
        assert_eq!(resumen.operaciones_aceptadas, 1);
        assert_eq!(db::cursor(&conexion), 1);
        let estado: String = conexion
            .query_row("SELECT estado FROM outbox", [], |f| f.get(0))
            .unwrap();
        assert_eq!(estado, "COMPLETADA");
    }

    #[tokio::test]
    async fn cursor_invalido_reconstruye_con_snapshot() {
        let mut conexion = db::abrir_en_memoria().unwrap();
        let transporte = TransporteFalso::nuevo()
            .con_pull(json!({ "eventos": [], "reiniciar": true, "siguiente_cursor": 7 }))
            .con_snapshot(json!({
                "cursor": 7,
                "expedientes": [{ "id": "e2", "numero": "EXP-2", "estado": "ALMACEN", "version": 1 }],
                "pedidos": []
            }));

        let resumen = sincronizar(&mut conexion, &transporte, "d1").await.unwrap();

        assert!(resumen.reconstruyo_snapshot);
        assert_eq!(db::cursor(&conexion), 7);
        let numero: String = conexion
            .query_row("SELECT numero FROM expedientes WHERE id = 'e2'", [], |f| f.get(0))
            .unwrap();
        assert_eq!(numero, "EXP-2");
    }

    #[tokio::test]
    async fn sin_conexion_el_outbox_sobrevive_y_queda_para_reintento() {
        let mut conexion = db::abrir_en_memoria().unwrap();
        encolar(&conexion, "CREAR_PEDIDO", &json!({ "expedientes": ["e1"] })).unwrap();

        let transporte = TransporteFalso::nuevo()
            .con_pull(json!({ "eventos": [], "siguiente_cursor": 0, "hay_mas": false }))
            .con_falla_de_push();

        let resultado = sincronizar(&mut conexion, &transporte, "d1").await;
        assert!(resultado.is_err());

        let (estado_outbox, intentos): (String, i64) = conexion
            .query_row("SELECT estado, intentos FROM outbox LIMIT 1", [], |f| {
                Ok((f.get(0)?, f.get(1)?))
            })
            .unwrap();
        assert_eq!(estado_outbox, "PENDIENTE", "la operación sigue en el outbox");
        assert_eq!(intentos, 1);
        assert!(
            pendientes(&conexion, 10).unwrap().is_empty(),
            "el reintento se difiere con retroceso exponencial"
        );
    }
}

