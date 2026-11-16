#![allow(dead_code)]

//! Utilidades compartidas por las pruebas de integración de la API.

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

pub const PASSWORD: &str = "Demo#2026-segura";

pub struct TestApp {
    pub admin: PgPool,
    pub db: PgPool,
    pub app: Router,
    pub nombre: String,
}

pub async fn setup() -> TestApp {
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
        .max_connections(10)
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
    pub async fn cerrar(self) {
        self.db.close().await;
        sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)", self.nombre))
            .execute(&self.admin)
            .await
            .expect("eliminar base de prueba");
        self.admin.close().await;
    }
}

pub async fn crear_usuario(db: &PgPool, correo: &str, rol: &str) -> Uuid {
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

pub async fn crear_expediente(db: &PgPool, numero: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO expedientes(numero) VALUES ($1) RETURNING id")
        .bind(numero)
        .fetch_one(db)
        .await
        .expect("crear expediente")
}

pub async fn pedir(
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

pub async fn iniciar_sesion(app: &Router, correo: &str) -> (String, String) {
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

/// Deja un expediente en custodia del solicitante usando el flujo real de la API.
pub async fn poner_en_custodia(t: &TestApp, solicitante: &str, expediente: Uuid) -> (String, String) {
    let (tok_sol, _) = iniciar_sesion(&t.app, solicitante).await;
    let (tok_alm, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;

    let (estado, pedido) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&tok_sol),
        Some(json!({ "expedientes": [expediente] })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "crear pedido: {pedido}");
    let pid = pedido["pedido_id"].as_str().unwrap().to_string();

    let (_, detalle) = pedir(&t.app, "GET", &format!("/api/v1/pedidos/{pid}"), Some(&tok_alm), None).await;
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
    assert_eq!(estado, StatusCode::OK, "despachar: {despacho}");
    let entrega = despacho["entregas"][0].as_str().unwrap().to_string();

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/entregas/{entrega}/confirmar"),
        Some(&tok_sol),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "confirmar: {cuerpo}");

    (tok_sol, pid)
}

pub async fn custodias_activas(db: &PgPool, expediente: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM custodias WHERE expediente_id = $1 AND fin IS NULL")
        .bind(expediente)
        .fetch_one(db)
        .await
        .expect("contar custodias")
}
