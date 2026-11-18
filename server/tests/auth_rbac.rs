//! Pruebas de autenticación, sesiones y autorización por permisos.
//!
//! Cada prueba usa una base PostgreSQL efímera y ejerce la API por HTTP en memoria
//! (`tower::ServiceExt`) para verificar identidad de sesión, no parámetros del cliente.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tower::ServiceExt;
use uuid::Uuid;

use expedientes_api::{
    api, auth::hash_password, config::Config, limiter::LoginLimiter, AppState,
};

const PASSWORD: &str = "Demo#2026-segura";

struct TestApp {
    admin: PgPool,
    db: PgPool,
    app: Router,
    nombre: String,
}

async fn setup() -> TestApp {
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
    let raiz = admin_url.rsplit_once('/').expect("URL con base").0;
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&format!("{raiz}/{nombre}"))
        .await
        .expect("conectar a la base de prueba");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("aplicar migraciones");

    let config = Config::para_pruebas(format!("{raiz}/{nombre}"));
    let estado = AppState::nuevo(db.clone(), config, LoginLimiter::nuevo(5, 900));
    TestApp { admin, db, app: api::router(estado), nombre }
}

impl TestApp {
    async fn cerrar(self) {
        self.db.close().await;
        sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)", self.nombre))
            .execute(&self.admin)
            .await
            .expect("eliminar base de prueba");
        self.admin.close().await;
    }
}

async fn crear_usuario(db: &PgPool, correo: &str, rol: &str) -> Uuid {
    let hash = hash_password(PASSWORD).expect("hash argon2");
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO usuarios(nombre, correo, password_hash, debe_cambiar_password)
         VALUES ($1,$2,$3,false) RETURNING id",
    )
    .bind(correo)
    .bind(correo)
    .bind(hash)
    .fetch_one(db)
    .await
    .expect("crear usuario");
    sqlx::query("INSERT INTO usuario_roles(usuario_id, rol_id) SELECT $1, id FROM roles WHERE clave = $2")
        .bind(id)
        .bind(rol)
        .execute(db)
        .await
        .expect("asignar rol");
    id
}

async fn crear_expediente(db: &PgPool, numero: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO expedientes(numero) VALUES ($1) RETURNING id")
        .bind(numero)
        .fetch_one(db)
        .await
        .expect("crear expediente")
}

async fn pedir(
    app: &Router,
    metodo: &str,
    ruta: &str,
    token: Option<&str>,
    cuerpo: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(metodo).uri(ruta);
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let request = match cuerpo {
        Some(c) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(c.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let respuesta = app.clone().oneshot(request).await.expect("respuesta");
    let estado = respuesta.status();
    let bytes = respuesta.into_body().collect().await.unwrap().to_bytes();
    let valor = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("respuesta JSON")
    };
    (estado, valor)
}

async fn iniciar_sesion(app: &Router, correo: &str) -> (String, String) {
    let (estado, cuerpo) = pedir(
        app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": correo, "password": PASSWORD })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "login de {correo}: {cuerpo}");
    (
        cuerpo["access_token"].as_str().expect("access").to_string(),
        cuerpo["refresh_token"].as_str().expect("refresh").to_string(),
    )
}

#[tokio::test]
async fn login_valido_devuelve_tokens_y_permisos() {
    let t = setup().await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "almacen@test.mx", "password": PASSWORD })),
    )
    .await;

    assert_eq!(estado, StatusCode::OK);
    assert!(!cuerpo["access_token"].as_str().unwrap().is_empty());
    assert!(!cuerpo["refresh_token"].as_str().unwrap().is_empty());
    let permisos = cuerpo["permisos"].as_array().expect("permisos");
    assert!(permisos.iter().any(|p| p == "pedidos:preparar"));

    t.cerrar().await;
}

#[tokio::test]
async fn credenciales_incorrectas_devuelven_401_sin_filtrar_detalle() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "usuario@test.mx", "password": "incorrecta" })),
    )
    .await;

    assert_eq!(estado, StatusCode::UNAUTHORIZED);
    assert_eq!(cuerpo["error"]["code"], "CREDENCIALES_INVALIDAS");
    assert!(cuerpo["error"]["message"].as_str().unwrap().contains("inválidas"));

    t.cerrar().await;
}

#[tokio::test]
async fn ruta_protegida_sin_token_devuelve_401() {
    let t = setup().await;
    let (estado, cuerpo) = pedir(&t.app, "GET", "/api/v1/auth/me", None, None).await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED);
    assert_eq!(cuerpo["error"]["code"], "NO_AUTENTICADO");
    t.cerrar().await;
}

#[tokio::test]
async fn crear_pedido_usa_la_identidad_de_la_sesion() {
    let t = setup().await;
    let usuario = crear_usuario(&t.db, "solicitante@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-AUTH-1").await;
    let (token, _) = iniciar_sesion(&t.app, "solicitante@test.mx").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&token),
        Some(json!({ "expedientes": [exp] })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");

    let dueno: Uuid = sqlx::query_scalar("SELECT solicitante_id FROM pedidos WHERE id = $1")
        .bind(cuerpo["pedido_id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(&t.db)
        .await
        .unwrap();
    assert_eq!(dueno, usuario, "el solicitante debe salir del token, no del cuerpo");

    t.cerrar().await;
}

#[tokio::test]
async fn usuario_sin_permiso_no_puede_preparar() {
    let t = setup().await;
    crear_usuario(&t.db, "solicitante@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "consulta@test.mx", "CONSULTA").await;
    let exp = crear_expediente(&t.db, "EXP-AUTH-2").await;

    let (token_sol, _) = iniciar_sesion(&t.app, "solicitante@test.mx").await;
    let (_, pedido) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&token_sol),
        Some(json!({ "expedientes": [exp] })),
    )
    .await;
    let (_, detalle) = pedir(
        &t.app,
        "GET",
        &format!("/api/v1/pedidos/{}", pedido["pedido_id"].as_str().unwrap()),
        Some(&token_sol),
        None,
    )
    .await;
    let item = detalle["items"][0]["id"].as_str().unwrap().to_string();

    let (token_consulta, _) = iniciar_sesion(&t.app, "consulta@test.mx").await;
    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/items/{item}/preparar"),
        Some(&token_consulta),
        Some(json!({})),
    )
    .await;

    assert_eq!(estado, StatusCode::FORBIDDEN, "{cuerpo}");
    assert_eq!(cuerpo["error"]["code"], "SIN_PERMISO");

    t.cerrar().await;
}

#[tokio::test]
async fn flujo_completo_con_identidades_autenticadas() {
    let t = setup().await;
    crear_usuario(&t.db, "solicitante@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    let exp = crear_expediente(&t.db, "EXP-AUTH-3").await;

    let (tok_sol, _) = iniciar_sesion(&t.app, "solicitante@test.mx").await;
    let (tok_alm, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;

    let (_, pedido) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&tok_sol),
        Some(json!({ "expedientes": [exp] })),
    )
    .await;
    let pedido_id = pedido["pedido_id"].as_str().unwrap().to_string();

    let (_, detalle) = pedir(&t.app, "GET", &format!("/api/v1/pedidos/{pedido_id}"), Some(&tok_alm), None).await;
    let item = detalle["items"][0]["id"].as_str().unwrap().to_string();

    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/items/{item}/preparar"),
        Some(&tok_alm),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (estado, despacho) = pedir(
        &t.app,
        "POST",
        "/api/v1/entregas",
        Some(&tok_alm),
        Some(json!({ "item_ids": [item] })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{despacho}");
    let entrega = despacho["entregas"][0].as_str().unwrap().to_string();

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/entregas/{entrega}/confirmar"),
        Some(&tok_sol),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");

    let (_, final_) = pedir(&t.app, "GET", &format!("/api/v1/pedidos/{pedido_id}"), Some(&tok_sol), None).await;
    assert_eq!(final_["estado"], "COMPLETADO");
    assert_eq!(final_["entregados"], 1);

    t.cerrar().await;
}

#[tokio::test]
async fn un_tercero_no_puede_confirmar_la_entrega() {
    let t = setup().await;
    crear_usuario(&t.db, "solicitante@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    crear_usuario(&t.db, "intruso@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-AUTH-4").await;

    let (tok_sol, _) = iniciar_sesion(&t.app, "solicitante@test.mx").await;
    let (tok_alm, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;
    let (tok_intruso, _) = iniciar_sesion(&t.app, "intruso@test.mx").await;

    let (_, pedido) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&tok_sol),
        Some(json!({ "expedientes": [exp] })),
    )
    .await;
    let pid = pedido["pedido_id"].as_str().unwrap().to_string();
    let (_, detalle) = pedir(&t.app, "GET", &format!("/api/v1/pedidos/{pid}"), Some(&tok_alm), None).await;
    let item = detalle["items"][0]["id"].as_str().unwrap().to_string();

    pedir(&t.app, "POST", &format!("/api/v1/items/{item}/preparar"), Some(&tok_alm), Some(json!({}))).await;
    let (_, despacho) = pedir(
        &t.app,
        "POST",
        "/api/v1/entregas",
        Some(&tok_alm),
        Some(json!({ "item_ids": [item] })),
    )
    .await;
    let entrega = despacho["entregas"][0].as_str().unwrap().to_string();

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/entregas/{entrega}/confirmar"),
        Some(&tok_intruso),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN, "{cuerpo}");

    t.cerrar().await;
}

#[tokio::test]
async fn refresh_rota_el_token_y_el_anterior_deja_de_servir() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (_, refresh) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, nuevo) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{nuevo}");
    assert_ne!(nuevo["refresh_token"].as_str().unwrap(), refresh);
    assert!(
        nuevo["access_expira_en"].as_str().is_some(),
        "la renovación debe informar la expiración del access token: {nuevo}"
    );

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED, "el refresh usado debe quedar revocado");

    t.cerrar().await;
}

#[tokio::test]
async fn logout_revoca_acceso_y_refresh() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (access, refresh) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, _) = pedir(&t.app, "POST", "/api/v1/auth/logout", Some(&access), Some(json!({}))).await;
    assert_eq!(estado, StatusCode::OK);

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/auth/me", Some(&access), None).await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/refresh",
        None,
        Some(json!({ "refresh_token": refresh })),
    )
    .await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED);

    t.cerrar().await;
}

#[tokio::test]
async fn el_login_se_bloquea_tras_cinco_fallos() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;

    for _ in 0..5 {
        let (estado, _) = pedir(
            &t.app,
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({ "correo": "usuario@test.mx", "password": "mala" })),
        )
        .await;
        assert_eq!(estado, StatusCode::UNAUTHORIZED);
    }

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "usuario@test.mx", "password": PASSWORD })),
    )
    .await;
    assert_eq!(estado, StatusCode::TOO_MANY_REQUESTS, "{cuerpo}");

    t.cerrar().await;
}

#[tokio::test]
async fn un_usuario_desactivado_pierde_el_acceso() {
    let t = setup().await;
    let id = crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (access, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    sqlx::query("UPDATE usuarios SET activo = false WHERE id = $1")
        .bind(id)
        .execute(&t.db)
        .await
        .unwrap();

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/auth/me", Some(&access), None).await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED);

    t.cerrar().await;
}

#[tokio::test]
async fn rechaza_campos_de_identidad_en_el_cuerpo() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-AUTH-5").await;
    let (token, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&token),
        Some(json!({
            "solicitante_id": Uuid::new_v4(),
            "expedientes": [exp]
        })),
    )
    .await;

    assert_eq!(estado, StatusCode::BAD_REQUEST, "la identidad no se acepta en el cuerpo");

    t.cerrar().await;
}

#[tokio::test]
async fn cambiar_password_revoca_sesiones_y_actualiza_credenciales() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (access, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;
    let nueva = "Nueva#Clave-2026";

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/password",
        Some(&access),
        Some(json!({ "actual": PASSWORD, "nueva": nueva })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/auth/me", Some(&access), None).await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED, "las sesiones previas deben quedar revocadas");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "usuario@test.mx", "password": PASSWORD })),
    )
    .await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED, "la contraseña anterior deja de servir");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "usuario@test.mx", "password": nueva })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    t.cerrar().await;
}

#[tokio::test]
async fn admin_crea_usuario_y_el_nuevo_puede_entrar() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    let (tok_admin, _) = iniciar_sesion(&t.app, "admin@test.mx").await;
    let nueva = "Otra#Clave-2026";

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/admin/usuarios",
        Some(&tok_admin),
        Some(json!({
            "nombre": "Nuevo almacenista",
            "correo": "nuevo@test.mx",
            "password": nueva,
            "roles": ["ALMACEN"]
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    assert!(cuerpo["usuario_id"].as_str().is_some());

    let (estado, sesion) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "nuevo@test.mx", "password": nueva })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{sesion}");
    assert_eq!(sesion["roles"][0], "ALMACEN");

    t.cerrar().await;
}

#[tokio::test]
async fn sin_permiso_de_administracion_no_se_crean_usuarios() {
    let t = setup().await;
    crear_usuario(&t.db, "consulta@test.mx", "CONSULTA").await;
    let (token, _) = iniciar_sesion(&t.app, "consulta@test.mx").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/admin/usuarios",
        Some(&token),
        Some(json!({
            "nombre": "Intruso",
            "correo": "intruso@test.mx",
            "password": "Otra#Clave-2026",
            "roles": ["ADMINISTRADOR"]
        })),
    )
    .await;

    assert_eq!(estado, StatusCode::FORBIDDEN, "{cuerpo}");
    t.cerrar().await;
}

#[tokio::test]
async fn desactivar_usuario_revoca_sus_sesiones() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    let objetivo = crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok_admin, _) = iniciar_sesion(&t.app, "admin@test.mx").await;
    let (tok_usuario, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/admin/usuarios/{objetivo}/desactivar"),
        Some(&tok_admin),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/auth/me", Some(&tok_usuario), None).await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "usuario@test.mx", "password": PASSWORD })),
    )
    .await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED);

    t.cerrar().await;
}
