//! Base local SQLite del escritorio: proyecciones autorizadas, outbox y estado de sincronización.
//!
//! Todo el esquema se versiona con `PRAGMA user_version` para poder migrar instalaciones
//! existentes sin perder datos ni operaciones pendientes.

use std::fs;
use std::path::Path;

use rusqlite::Connection;

pub const VERSION_ESQUEMA: i64 = 1;

pub fn abrir(ruta: &Path) -> Result<Connection, String> {
    let clave = crate::seguridad::clave_sqlite()?;
    abrir_con_clave(ruta, &clave)
}

pub fn abrir_con_clave(ruta: &Path, clave: &str) -> Result<Connection, String> {
    if ruta.exists() && es_sqlite_plano(ruta)? {
        migrar_plano_a_cifrado(ruta, clave)?;
    }

    let mut conexion = Connection::open(ruta).map_err(|error| error.to_string())?;
    aplicar_clave(&conexion, clave)?;
    validar_clave(&conexion)?;
    migrar(&mut conexion).map_err(|error| error.to_string())?;
    Ok(conexion)
}

fn es_sqlite_plano(ruta: &Path) -> Result<bool, String> {
    let bytes = fs::read(ruta).map_err(|error| error.to_string())?;
    Ok(bytes.starts_with(b"SQLite format 3\0"))
}

fn aplicar_clave(conexion: &Connection, clave: &str) -> Result<(), String> {
    let escapada = clave.replace('\'', "''");
    conexion
        .execute_batch(&format!("PRAGMA key = '{escapada}';"))
        .map_err(|error| format!("No se pudo aplicar la clave local: {error}"))
}

fn validar_clave(conexion: &Connection) -> Result<(), String> {
    conexion
        .query_row("SELECT count(*) FROM sqlite_master", [], |fila| fila.get::<_, i64>(0))
        .map(|_| ())
        .map_err(|_| "La base local está cifrada con otra clave o está dañada".to_string())
}

/// Convierte una base SQLite antigua, sin cifrar, a SQLCipher.
/// La copia plana se elimina al verificar que la nueva base abre correctamente.
fn migrar_plano_a_cifrado(ruta: &Path, clave: &str) -> Result<(), String> {
    let temporal = ruta.with_extension("sqlite.tmp");
    let respaldo = ruta.with_extension("sqlite.plain.bak");
    let _ = fs::remove_file(&temporal);
    let _ = fs::remove_file(&respaldo);

    let plano = Connection::open(ruta).map_err(|error| error.to_string())?;
    let ruta_temporal = temporal.to_string_lossy().replace('\'', "''");
    let clave_sql = clave.replace('\'', "''");
    plano
        .execute_batch(&format!(
            "ATTACH DATABASE '{ruta_temporal}' AS cifrada KEY '{clave_sql}';"
        ))
        .map_err(|error| format!("No se pudo crear la base cifrada: {error}"))?;
    plano
        .execute_batch("SELECT sqlcipher_export('cifrada');")
        .map_err(|error| format!("No se pudo exportar la base local: {error}"))?;

    let version: i64 = plano
        .query_row("PRAGMA user_version", [], |fila| fila.get(0))
        .map_err(|error| error.to_string())?;
    plano
        .execute_batch(&format!("PRAGMA cifrada.user_version = {version};"))
        .map_err(|error| error.to_string())?;
    plano
        .execute_batch("DETACH DATABASE cifrada;")
        .map_err(|error| error.to_string())?;
    drop(plano);

    let verificacion = Connection::open(&temporal).map_err(|error| error.to_string())?;
    aplicar_clave(&verificacion, clave)?;
    validar_clave(&verificacion)?;
    drop(verificacion);

    fs::rename(ruta, &respaldo).map_err(|error| error.to_string())?;
    if let Err(error) = fs::rename(&temporal, ruta) {
        let _ = fs::rename(&respaldo, ruta);
        return Err(format!("No se pudo activar la base cifrada: {error}"));
    }
    let _ = fs::remove_file(&respaldo);
    Ok(())
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

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    const CLAVE: &str = "clave-de-pruebas-sqlcipher-con-mas-de-32-caracteres";

    #[test]
    fn crea_reabre_y_rechaza_clave_incorrecta() {
        let directorio = tempfile::tempdir().unwrap();
        let ruta = directorio.path().join("expedientes.sqlite");

        let conexion = abrir_con_clave(&ruta, CLAVE).unwrap();
        conexion
            .execute(
                "INSERT INTO sync_metadata(clave, valor) VALUES ('prueba', 'cifrada')",
                [],
            )
            .unwrap();
        drop(conexion);

        let bytes = fs::read(&ruta).unwrap();
        assert!(
            !bytes.starts_with(b"SQLite format 3\0"),
            "el archivo no debe conservar la cabecera SQLite plana"
        );

        let conexion = abrir_con_clave(&ruta, CLAVE).unwrap();
        let valor: String = conexion
            .query_row("SELECT valor FROM sync_metadata WHERE clave = 'prueba'", [], |fila| {
                fila.get(0)
            })
            .unwrap();
        assert_eq!(valor, "cifrada");

        assert!(abrir_con_clave(&ruta, "otra-clave-suficientemente-larga-para-sqlcipher").is_err());
    }

    #[test]
    fn migra_una_base_plana_sin_perder_datos() {
        let directorio = tempfile::tempdir().unwrap();
        let ruta = directorio.path().join("expedientes.sqlite");

        let plana = Connection::open(&ruta).unwrap();
        plana
            .execute_batch(
                "CREATE TABLE legado(valor TEXT NOT NULL);
                 INSERT INTO legado(valor) VALUES ('conservado');
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        drop(plana);

        let conexion = abrir_con_clave(&ruta, CLAVE).unwrap();
        let valor: String = conexion
            .query_row("SELECT valor FROM legado", [], |fila| fila.get(0))
            .unwrap();
        assert_eq!(valor, "conservado");
        assert!(!ruta.with_extension("sqlite.plain.bak").exists());

        let bytes = fs::read(&ruta).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3\0"));
    }
}

