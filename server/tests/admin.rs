//! Edición de usuarios desde la administración.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use common::*;

async fn administrador(t: &TestApp, correo: &str) -> String {
    crear_usuario(&t.db, correo, "ADMINISTRADOR").await;
    iniciar_sesion(&t.app, correo).await.0
}

#[tokio::test]
async fn admin_edita_nombre_correo_y_contrasena() {
    let t = setup().await;
    let tok_admin = administrador(&t, "admin@test.mx").await;
    let usuario = crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok_usuario_previo, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/admin/usuarios/{usuario}"),
        Some(&tok_admin),
        Some(json!({ "nombre": "Ana Pérez", "correo": "ana@test.mx" })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "ana@test.mx", "password": PASSWORD })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "el correo cambió y la contraseña sigue igual");

    let nueva = "Nueva#Clave-2026";
    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/admin/usuarios/{usuario}"),
        Some(&tok_admin),
        Some(json!({ "password": nueva })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/auth/me", Some(&tok_usuario_previo), None).await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED, "la sesión previa se revoca");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "ana@test.mx", "password": PASSWORD })),
    )
    .await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED, "la contraseña anterior deja de servir");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "ana@test.mx", "password": nueva })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    t.cerrar().await;
}

#[tokio::test]
async fn edicion_valida_correo_unico_permisos_y_longitud() {
    let t = setup().await;
    let tok_admin = administrador(&t, "admin@test.mx").await;
    let usuario = crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "ocupado@test.mx", "USUARIO").await;

    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/admin/usuarios/{usuario}"),
        Some(&tok_admin),
        Some(json!({ "correo": "ocupado@test.mx" })),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/admin/usuarios/{usuario}"),
        Some(&tok_admin),
        Some(json!({ "password": "corta" })),
    )
    .await;
    assert_eq!(estado, StatusCode::UNPROCESSABLE_ENTITY);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/admin/usuarios/{usuario}"),
        Some(&tok_admin),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::UNPROCESSABLE_ENTITY);

    let (tok_usuario, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;
    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/admin/usuarios/{usuario}"),
        Some(&tok_usuario),
        Some(json!({ "nombre": "Intento" })),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/admin/usuarios/{}", Uuid::new_v4()),
        Some(&tok_admin),
        Some(json!({ "nombre": "Nadie" })),
    )
    .await;
    assert_eq!(estado, StatusCode::NOT_FOUND);

    t.cerrar().await;
}
