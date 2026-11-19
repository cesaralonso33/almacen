//! Endurecimiento transversal de la API.

mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

use common::*;

#[tokio::test]
async fn respuestas_incluyen_cabeceras_de_seguridad() {
    let t = setup().await;
    let respuesta = t
        .app
        .clone()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(respuesta.status(), StatusCode::OK);
    let cabeceras = respuesta.headers();
    assert_eq!(cabeceras.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
    assert_eq!(cabeceras.get(header::X_FRAME_OPTIONS).unwrap(), "DENY");
    assert_eq!(cabeceras.get(header::REFERRER_POLICY).unwrap(), "no-referrer");
    assert_eq!(cabeceras.get(header::CACHE_CONTROL).unwrap(), "no-store");
    assert!(cabeceras.get(header::CONTENT_SECURITY_POLICY).is_some());
    assert!(cabeceras.get(header::STRICT_TRANSPORT_SECURITY).is_some());
    assert!(cabeceras.get("x-permitted-cross-domain-policies").is_some());

    t.cerrar().await;
}

#[tokio::test]
async fn rechaza_cuerpos_demasiado_grandes() {
    let t = setup().await;
    let cuerpo = format!(
        "{{\"correo\":\"a@test.mx\",\"password\":\"{}\"}}",
        "x".repeat(12 * 1024 * 1024 + 100)
    );
    let respuesta = t
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(cuerpo))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(respuesta.status(), StatusCode::PAYLOAD_TOO_LARGE);

    t.cerrar().await;
}

#[tokio::test]
async fn rechaza_credenciales_de_longitud_abusiva() {
    let t = setup().await;
    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "correo": "a@test.mx", "password": "x".repeat(600) })),
    )
    .await;
    assert_eq!(estado, StatusCode::BAD_REQUEST, "{cuerpo}");

    t.cerrar().await;
}
