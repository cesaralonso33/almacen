//! Pruebas de invariantes del modelo de datos.
//!
//! Cada prueba crea una base PostgreSQL efímera, aplica todas las migraciones y la elimina al final.
//! Requiere PostgreSQL local accesible; se configura con `TEST_ADMIN_DATABASE_URL`
//! (por omisión `postgres://postgres:postgres@127.0.0.1:5432/postgres`).

use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

struct TestDb {
    admin: PgPool,
    pool: PgPool,
    nombre: String,
}

async fn setup() -> TestDb {
    let admin_url = std::env::var("TEST_ADMIN_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/postgres".to_string());
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("conectar al servidor PostgreSQL de pruebas");
    let nombre = format!("expedientes_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE \"{nombre}\""))
        .execute(&admin)
        .await
        .expect("crear base de prueba");
    let base = admin_url.rsplit_once('/').expect("URL con base").0;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&format!("{base}/{nombre}"))
        .await
        .expect("conectar a la base de prueba");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("aplicar migraciones");
    TestDb { admin, pool, nombre }
}

impl TestDb {
    async fn cerrar(self) {
        self.pool.close().await;
        sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)", self.nombre))
            .execute(&self.admin)
            .await
            .expect("eliminar base de prueba");
        self.admin.close().await;
    }
}

async fn usuario(pool: &PgPool, nombre: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO usuarios(nombre) VALUES ($1) RETURNING id")
        .bind(nombre)
        .fetch_one(pool)
        .await
        .expect("crear usuario")
}

async fn expediente(pool: &PgPool, numero: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO expedientes(numero) VALUES ($1) RETURNING id")
        .bind(numero)
        .fetch_one(pool)
        .await
        .expect("crear expediente")
}

#[tokio::test]
async fn migraciones_aplican_y_siembran_rbac() {
    let db = setup().await;

    let roles: i64 = sqlx::query_scalar("SELECT count(*) FROM roles")
        .fetch_one(&db.pool)
        .await
        .expect("tabla roles");
    let permisos: i64 = sqlx::query_scalar("SELECT count(*) FROM permisos")
        .fetch_one(&db.pool)
        .await
        .expect("tabla permisos");
    let asignaciones: i64 = sqlx::query_scalar("SELECT count(*) FROM rol_permisos")
        .fetch_one(&db.pool)
        .await
        .expect("tabla rol_permisos");
    let admin: i64 = sqlx::query_scalar("SELECT count(*) FROM roles WHERE clave = 'ADMINISTRADOR'")
        .fetch_one(&db.pool)
        .await
        .expect("rol administrador");

    assert_eq!(admin, 1, "debe existir el rol ADMINISTRADOR");
    assert!(roles >= 4, "se esperaban al menos 4 roles, hay {roles}");
    assert!(permisos >= 10, "se esperaban al menos 10 permisos, hay {permisos}");
    assert!(asignaciones > 0, "los roles deben tener permisos asignados");

    db.cerrar().await;
}

#[tokio::test]
async fn usuario_no_puede_tener_dos_asignaciones_activas_del_mismo_rol() {
    let db = setup().await;
    let u = usuario(&db.pool, "Ana").await;
    let rol: Uuid = sqlx::query_scalar("SELECT id FROM roles WHERE clave = 'USUARIO'")
        .fetch_one(&db.pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO usuario_roles(usuario_id, rol_id) VALUES ($1,$2)")
        .bind(u)
        .bind(rol)
        .execute(&db.pool)
        .await
        .expect("primera asignación");

    let segundo = sqlx::query("INSERT INTO usuario_roles(usuario_id, rol_id) VALUES ($1,$2)")
        .bind(u)
        .bind(rol)
        .execute(&db.pool)
        .await;
    assert!(segundo.is_err(), "no debe permitirse un rol activo duplicado");

    db.cerrar().await;
}

#[tokio::test]
async fn refresh_token_es_unico_y_la_sesion_se_puede_revocar() {
    let db = setup().await;
    let u = usuario(&db.pool, "Beto").await;

    sqlx::query(
        "INSERT INTO sesiones(usuario_id, refresh_token_hash, expira_en)
         VALUES ($1,'hash-1', now() + interval '1 hour')",
    )
    .bind(u)
    .execute(&db.pool)
    .await
    .expect("crear sesión");

    let duplicada = sqlx::query(
        "INSERT INTO sesiones(usuario_id, refresh_token_hash, expira_en)
         VALUES ($1,'hash-1', now() + interval '1 hour')",
    )
    .bind(u)
    .execute(&db.pool)
    .await;
    assert!(duplicada.is_err(), "el hash de refresh debe ser único");

    sqlx::query("UPDATE sesiones SET revocada_en = now() WHERE refresh_token_hash = 'hash-1'")
        .execute(&db.pool)
        .await
        .expect("revocar");
    let revocada: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT revocada_en FROM sesiones WHERE refresh_token_hash = 'hash-1'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(revocada.is_some(), "la sesión debe quedar revocada");

    db.cerrar().await;
}

#[tokio::test]
async fn jerarquia_de_ubicaciones_respeta_los_niveles() {
    let db = setup().await;
    let edificio: Uuid = sqlx::query_scalar(
        "INSERT INTO ubicaciones(tipo, codigo, nombre) VALUES ('EDIFICIO','E1','Edificio principal') RETURNING id",
    )
    .fetch_one(&db.pool)
    .await
    .expect("edificio raíz");

    let salto = sqlx::query(
        "INSERT INTO ubicaciones(padre_id, tipo, codigo, nombre) VALUES ($1,'CAJA','C1','Caja inválida')",
    )
    .bind(edificio)
    .execute(&db.pool)
    .await;
    assert!(salto.is_err(), "una CAJA no puede colgar de un EDIFICIO");

    let area: Uuid = sqlx::query_scalar(
        "INSERT INTO ubicaciones(padre_id, tipo, codigo, nombre) VALUES ($1,'AREA','A1','Archivo') RETURNING id",
    )
    .bind(edificio)
    .fetch_one(&db.pool)
    .await
    .expect("área bajo edificio");

    let estante: Uuid = sqlx::query_scalar(
        "INSERT INTO ubicaciones(padre_id, tipo, codigo, nombre) VALUES ($1,'ESTANTE','S1','Estante 1') RETURNING id",
    )
    .bind(area)
    .fetch_one(&db.pool)
    .await
    .expect("estante bajo área");

    sqlx::query(
        "INSERT INTO ubicaciones(padre_id, tipo, codigo, nombre) VALUES ($1,'CAJA','C1','Caja 1')",
    )
    .bind(estante)
    .execute(&db.pool)
    .await
    .expect("caja bajo estante");

    db.cerrar().await;
}

#[tokio::test]
async fn codigo_de_ubicacion_es_unico_por_padre_incluido_el_nivel_raiz() {
    let db = setup().await;

    sqlx::query("INSERT INTO ubicaciones(tipo, codigo, nombre) VALUES ('EDIFICIO','E1','Uno')")
        .execute(&db.pool)
        .await
        .expect("primer edificio");
    let duplicado = sqlx::query(
        "INSERT INTO ubicaciones(tipo, codigo, nombre) VALUES ('EDIFICIO','E1','Otro')",
    )
    .execute(&db.pool)
    .await;
    assert!(duplicado.is_err(), "dos raíces no pueden compartir código");

    let e2: Uuid = sqlx::query_scalar(
        "INSERT INTO ubicaciones(tipo, codigo, nombre) VALUES ('EDIFICIO','E2','Dos') RETURNING id",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO ubicaciones(padre_id, tipo, codigo, nombre) VALUES ($1,'AREA','E1','Hijo')")
        .bind(e2)
        .execute(&db.pool)
        .await
        .expect("el código puede repetirse en otro padre");

    db.cerrar().await;
}

#[tokio::test]
async fn solo_hay_una_reserva_activa_por_expediente() {
    let db = setup().await;
    let u = usuario(&db.pool, "Carla").await;
    let exp = expediente(&db.pool, "EXP-R-1").await;
    let pedido: Uuid = sqlx::query_scalar("INSERT INTO pedidos(solicitante_id) VALUES ($1) RETURNING id")
        .bind(u)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let item: Uuid = sqlx::query_scalar(
        "INSERT INTO pedido_items(pedido_id, expediente_id) VALUES ($1,$2) RETURNING id",
    )
    .bind(pedido)
    .bind(exp)
    .fetch_one(&db.pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO reservas(expediente_id, pedido_item_id, reservada_por, expira_en)
         VALUES ($1,$2,$3, now() + interval '30 minutes')",
    )
    .bind(exp)
    .bind(item)
    .bind(u)
    .execute(&db.pool)
    .await
    .expect("primera reserva");

    let segunda = sqlx::query(
        "INSERT INTO reservas(expediente_id, pedido_item_id, reservada_por, expira_en)
         VALUES ($1,$2,$3, now() + interval '30 minutes')",
    )
    .bind(exp)
    .bind(item)
    .bind(u)
    .execute(&db.pool)
    .await;
    assert!(segunda.is_err(), "no puede haber dos reservas activas del mismo expediente");

    sqlx::query("UPDATE reservas SET liberada_en = now() WHERE expediente_id = $1")
        .bind(exp)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO reservas(expediente_id, pedido_item_id, reservada_por, expira_en)
         VALUES ($1,$2,$3, now() + interval '30 minutes')",
    )
    .bind(exp)
    .bind(item)
    .bind(u)
    .execute(&db.pool)
    .await
    .expect("tras liberar, se puede reservar de nuevo");

    db.cerrar().await;
}

#[tokio::test]
async fn descarte_con_motivo_otro_exige_detalle() {
    let db = setup().await;
    let u = usuario(&db.pool, "Dario").await;
    let exp = expediente(&db.pool, "EXP-D-1").await;
    let pedido: Uuid = sqlx::query_scalar("INSERT INTO pedidos(solicitante_id) VALUES ($1) RETURNING id")
        .bind(u)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let item: Uuid = sqlx::query_scalar(
        "INSERT INTO pedido_items(pedido_id, expediente_id) VALUES ($1,$2) RETURNING id",
    )
    .bind(pedido)
    .bind(exp)
    .fetch_one(&db.pool)
    .await
    .unwrap();

    let sin_detalle = sqlx::query(
        "INSERT INTO descartes(pedido_item_id, motivo_clave, usuario_id) VALUES ($1,'OTRO',$2)",
    )
    .bind(item)
    .bind(u)
    .execute(&db.pool)
    .await;
    assert!(sin_detalle.is_err(), "'OTRO' exige descripción");

    let con_detalle = sqlx::query(
        "INSERT INTO descartes(pedido_item_id, motivo_clave, detalle, usuario_id)
         VALUES ($1,'OTRO','El expediente está en otro edificio',$2)",
    )
    .bind(item)
    .bind(u)
    .execute(&db.pool)
    .await;
    assert!(con_detalle.is_ok(), "con descripción debe aceptarse");

    db.cerrar().await;
}

#[tokio::test]
async fn auditoria_es_append_only() {
    let db = setup().await;
    let u = usuario(&db.pool, "Elena").await;
    sqlx::query(
        "INSERT INTO auditoria(actor_id, accion, entidad, resultado) VALUES ($1,'pedidos.crear','pedidos','EXITO')",
    )
    .bind(u)
    .execute(&db.pool)
    .await
    .expect("registrar auditoría");

    sqlx::query("UPDATE auditoria SET accion = 'alterado'")
        .execute(&db.pool)
        .await
        .expect("el UPDATE se ignora, no falla");
    let accion: String = sqlx::query_scalar("SELECT accion FROM auditoria")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(accion, "pedidos.crear", "la auditoría no debe poder modificarse");

    sqlx::query("DELETE FROM auditoria")
        .execute(&db.pool)
        .await
        .expect("el DELETE se ignora, no falla");
    let filas: i64 = sqlx::query_scalar("SELECT count(*) FROM auditoria")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(filas, 1, "la auditoría no debe poder borrarse");

    db.cerrar().await;
}

#[tokio::test]
async fn sync_events_tiene_secuencia_global_monotonica() {
    let db = setup().await;
    let exp = expediente(&db.pool, "EXP-S-1").await;

    let s1: i64 = sqlx::query_scalar(
        "INSERT INTO sync_events(entidad, entidad_id, operacion) VALUES ('expedientes',$1,'CREAR') RETURNING secuencia",
    )
    .bind(exp)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let s2: i64 = sqlx::query_scalar(
        "INSERT INTO sync_events(entidad, entidad_id, operacion, tombstone)
         VALUES ('expedientes',$1,'ELIMINAR',true) RETURNING secuencia",
    )
    .bind(exp)
    .fetch_one(&db.pool)
    .await
    .unwrap();

    assert!(s2 > s1, "la secuencia debe crecer de forma monotónica");

    db.cerrar().await;
}

#[tokio::test]
async fn operaciones_idempotentes_son_unicas_por_dispositivo_y_clave() {
    let db = setup().await;
    let u = usuario(&db.pool, "Fabi").await;
    let d1: Uuid = sqlx::query_scalar(
        "INSERT INTO dispositivos(usuario_id, nombre) VALUES ($1,'Equipo 1') RETURNING id",
    )
    .bind(u)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let d2: Uuid = sqlx::query_scalar(
        "INSERT INTO dispositivos(usuario_id, nombre) VALUES ($1,'Equipo 2') RETURNING id",
    )
    .bind(u)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let clave = Uuid::new_v4();

    for d in [d1, d2] {
        sqlx::query(
            "INSERT INTO operaciones_idempotentes(clave, dispositivo_id, tipo, payload_hash)
             VALUES ($1,$2,'CREAR_PEDIDO','sha256')",
        )
        .bind(clave)
        .bind(d)
        .execute(&db.pool)
        .await
        .expect("la misma clave en otro dispositivo es válida");
    }

    let duplicada = sqlx::query(
        "INSERT INTO operaciones_idempotentes(clave, dispositivo_id, tipo, payload_hash)
         VALUES ($1,$2,'CREAR_PEDIDO','sha256')",
    )
    .bind(clave)
    .bind(d1)
    .execute(&db.pool)
    .await;
    assert!(duplicada.is_err(), "clave y dispositivo deben ser únicos");

    db.cerrar().await;
}

#[tokio::test]
async fn custodia_activa_sigue_siendo_unica() {
    let db = setup().await;
    let u = usuario(&db.pool, "Gina").await;
    let exp = expediente(&db.pool, "EXP-C-1").await;

    sqlx::query("INSERT INTO custodias(expediente_id, usuario_id) VALUES ($1,$2)")
        .bind(exp)
        .bind(u)
        .execute(&db.pool)
        .await
        .expect("primera custodia");
    let segunda = sqlx::query("INSERT INTO custodias(expediente_id, usuario_id) VALUES ($1,$2)")
        .bind(exp)
        .bind(u)
        .execute(&db.pool)
        .await;
    assert!(segunda.is_err(), "solo puede existir una custodia activa");

    db.cerrar().await;
}

#[tokio::test]
async fn numero_de_expediente_es_unico_por_unidad_y_anio() {
    let db = setup().await;

    sqlx::query("INSERT INTO expedientes(numero, unidad, anio) VALUES ('A-1','JUZGADO 1',2026)")
        .execute(&db.pool)
        .await
        .expect("primer expediente");
    sqlx::query("INSERT INTO expedientes(numero, unidad, anio) VALUES ('A-1','JUZGADO 2',2026)")
        .execute(&db.pool)
        .await
        .expect("mismo número en otra unidad");

    let duplicado =
        sqlx::query("INSERT INTO expedientes(numero, unidad, anio) VALUES ('A-1','JUZGADO 1',2026)")
            .execute(&db.pool)
            .await;
    assert!(duplicado.is_err(), "número repetido en la misma unidad y año");

    db.cerrar().await;
}
