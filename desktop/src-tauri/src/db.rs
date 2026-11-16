//! Base local SQLite del escritorio: proyecciones autorizadas, outbox y estado de sincronización.
//!
//! Todo el esquema se versiona con `PRAGMA user_version` para poder migrar instalaciones
//! existentes sin perder datos ni operaciones pendientes.

use std::path::Path;

use rusqlite::Connection;

pub const VERSION_ESQUEMA: i64 = 1;

pub fn abrir(ruta: &Path) -> rusqlite::Result<Connection> {
    let mut conexion = Connection::open(ruta)?;
    migrar(&mut conexion)?;
    Ok(conexion)
}

#[cfg(test)]
pub fn abrir_en_memoria() -> rusqlite::Result<Connection> {
    let mut conexion = Connection::open_in_memory()?;
    migrar(&mut conexion)?;
    Ok(conexion)
}

pub fn migrar(conexion: &mut Connection) -> rusqlite::Result<()> {
    // En bases en memoria WAL no aplica; se ignora el resultado.
    let _ = conexion.pragma_update(None, "journal_mode", "WAL");
    let _ = conexion.pragma_update(None, "foreign_keys", "ON");

    let version: i64 = conexion.query_row("PRAGMA user_version", [], |fila| fila.get(0))?;

    if version < 1 {
        aplicar_v1(conexion)?;
        conexion.pragma_update(None, "user_version", VERSION_ESQUEMA)?;
    }

    Ok(())
}

fn aplicar_v1(conexion: &mut Connection) -> rusqlite::Result<()> {
    conexion.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS expedientes (
            id                TEXT PRIMARY KEY,
            numero            TEXT NOT NULL,
            descripcion       TEXT NOT NULL DEFAULT '',
            estado            TEXT NOT NULL,
            ubicacion_id      TEXT,
            custodio_id       TEXT,
            version           INTEGER NOT NULL DEFAULT 0,
            actualizado_en    TEXT,
            sincronizado_en   TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS pedidos (
            id              TEXT PRIMARY KEY,
            solicitante_id  TEXT,
            estado          TEXT NOT NULL,
            cancelado_en    TEXT,
            version         INTEGER NOT NULL DEFAULT 0,
            sincronizado_en TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS pedido_items (
            id              TEXT PRIMARY KEY,
            pedido_id       TEXT NOT NULL,
            expediente_id   TEXT,
            estado          TEXT NOT NULL,
            version         INTEGER NOT NULL DEFAULT 0,
            sincronizado_en TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS pedido_items_por_pedido ON pedido_items (pedido_id);

        CREATE TABLE IF NOT EXISTS entregas (
            id              TEXT PRIMARY KEY,
            pedido_item_id  TEXT,
            estado          TEXT NOT NULL,
            sincronizado_en TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS outbox (
            id              TEXT PRIMARY KEY,
            clave           TEXT NOT NULL UNIQUE,
            tipo            TEXT NOT NULL,
            payload         TEXT NOT NULL,
            version_base    INTEGER,
            estado          TEXT NOT NULL DEFAULT 'PENDIENTE'
                            CHECK (estado IN ('PENDIENTE', 'ENVIANDO', 'COMPLETADA', 'RECHAZADA', 'CONFLICTO')),
            intentos        INTEGER NOT NULL DEFAULT 0,
            ultimo_error    TEXT,
            proximo_intento TEXT NOT NULL DEFAULT (datetime('now')),
            creado_en       TEXT NOT NULL DEFAULT (datetime('now')),
            completado_en   TEXT
        );
        CREATE INDEX IF NOT EXISTS outbox_pendientes ON outbox (estado, proximo_intento);

        CREATE TABLE IF NOT EXISTS sync_metadata (
            clave TEXT PRIMARY KEY,
            valor TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_conflicts (
            id           TEXT PRIMARY KEY,
            clave        TEXT,
            tipo         TEXT,
            motivo       TEXT NOT NULL,
            payload      TEXT,
            creado_en    TEXT NOT NULL DEFAULT (datetime('now')),
            resuelto_en  TEXT
        );

        CREATE TABLE IF NOT EXISTS sync_status (
            id           INTEGER PRIMARY KEY CHECK (id = 1),
            estado       TEXT NOT NULL,
            ultimo_error TEXT,
            ultima_sync  TEXT,
            pendientes   INTEGER NOT NULL DEFAULT 0
        );
        INSERT OR IGNORE INTO sync_status(id, estado) VALUES (1, 'SIN_CONEXION');
        "#,
    )
}

pub fn leer_meta(conexion: &Connection, clave: &str) -> Option<String> {
    conexion
        .query_row("SELECT valor FROM sync_metadata WHERE clave = ?1", [clave], |fila| {
            fila.get(0)
        })
        .ok()
}

pub fn escribir_meta(conexion: &Connection, clave: &str, valor: &str) -> rusqlite::Result<()> {
    conexion.execute(
        "INSERT INTO sync_metadata(clave, valor) VALUES (?1, ?2)
         ON CONFLICT(clave) DO UPDATE SET valor = excluded.valor",
        rusqlite::params![clave, valor],
    )?;
    Ok(())
}

pub fn cursor(conexion: &Connection) -> i64 {
    leer_meta(conexion, "cursor")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

