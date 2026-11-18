//! Módulo P.A.: CRUD, folio anual, campos libres, filtros y constancia PDF.

mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use common::*;

fn registro(fecha: &str, nombre: &str) -> serde_json::Value {
    json!({
        "fecha": fecha,
        "curp": "ABCD800101HDFXYZ01",
        "nombre": nombre,
        "genero": "MASCULINO",
        "municipio": "Toluca",
        "edad": 46,
        "observaciones": "Registro de prueba",
        "campos_extra": [
            { "nombre": "Teléfono", "valor": "555-0101" },
            { "nombre": "Expediente relacionado", "valor": "PA-2026-0001" }
        ]
    })
}

#[tokio::test]
async fn crea_consulta_actualiza_elimina_y_genera_pdf() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    let (estado, creado) = pedir(
        &t.app,
        "POST",
        "/api/v1/pa",
        Some(&tok),
        Some(registro("2026-11-18", "Ana Pérez")),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{creado}");
    assert_eq!(creado["folio"], "PA-2026-0001");
    assert_eq!(creado["nombre"], "Ana Pérez");
    assert_eq!(creado["campos_extra"][0]["nombre"], "Teléfono");
    let id = creado["id"].as_str().unwrap();

    let (estado, listado) = pedir(
        &t.app,
        "GET",
        "/api/v1/pa?nombre=Ana&campo=Tel%C3%A9fono&valor=555",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{listado}");
    assert_eq!(listado["total"], 1);
    assert_eq!(listado["registros"][0]["folio"], "PA-2026-0001");

    let (estado, actualizado) = pedir(
        &t.app,
        "PUT",
        &format!("/api/v1/pa/{id}"),
        Some(&tok),
        Some(json!({
            "fecha": "2026-11-18",
            "curp": "ABCD800101HDFXYZ01",
            "nombre": "Ana Pérez Actualizada",
            "genero": "FEMENINO",
            "municipio": "Toluca",
            "edad": 47,
            "observaciones": "Actualizado",
            "campos_extra": []
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{actualizado}");
    assert_eq!(actualizado["folio"], "PA-2026-0001");
    assert_eq!(actualizado["nombre"], "Ana Pérez Actualizada");

    let respuesta = t
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/pa/{id}/documento"))
                .header(header::AUTHORIZATION, format!("Bearer {tok}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(respuesta.status(), StatusCode::OK);
    assert_eq!(
        respuesta.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/pdf"
    );
    let pdf = respuesta.into_body().collect().await.unwrap().to_bytes();
    assert!(pdf.starts_with(b"%PDF"));

    let (estado, _) = pedir(
        &t.app,
        "DELETE",
        &format!("/api/v1/pa/{id}"),
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (_, despues) = pedir(&t.app, "GET", "/api/v1/pa", Some(&tok), None).await;
    assert_eq!(despues["total"], 0);

    t.cerrar().await;
}

#[tokio::test]
async fn folio_es_consecutivo_por_anio() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    for (fecha, esperado) in [
        ("2026-01-10", "PA-2026-0001"),
        ("2026-06-20", "PA-2026-0002"),
        ("2027-02-01", "PA-2027-0001"),
    ] {
        let (estado, cuerpo) = pedir(
            &t.app,
            "POST",
            "/api/v1/pa",
            Some(&tok),
            Some(registro(fecha, "Persona")),
        )
        .await;
        assert_eq!(estado, StatusCode::OK, "{cuerpo}");
        assert_eq!(cuerpo["folio"], esperado);
    }

    t.cerrar().await;
}

#[tokio::test]
async fn consulta_lee_pero_no_crea() {
    let t = setup().await;
    crear_usuario(&t.db, "consulta@test.mx", "CONSULTA").await;
    let (tok, _) = iniciar_sesion(&t.app, "consulta@test.mx").await;

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/pa", Some(&tok), None).await;
    assert_eq!(estado, StatusCode::OK);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/pa",
        Some(&tok),
        Some(registro("2026-11-18", "No permitido")),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    t.cerrar().await;
}
