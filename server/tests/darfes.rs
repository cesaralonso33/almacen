//! Módulo DARFES: CRUD, documentos, exportación y reportes.

mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use common::*;

fn darfe(numero: &str, nombre: &str) -> serde_json::Value {
    json!({
        "numero_darfe": numero,
        "fecha": "2026-11-18",
        "nombre": nombre,
        "expediente": "EXP-DARFE-1",
        "registro": "REG-1",
        "estado_cuenta": "CUENTA-1",
        "banco": "Banco Institucional",
        "identificacion": "INE-1",
        "curp": "ABCD800101HDFXYZ01",
        "rfc": "ABCD800101XYZ",
        "estatus": "PENDIENTE",
        "campos_extra": [{ "nombre": "Teléfono", "valor": "555-0101" }]
    })
}

async fn descargar(t: &TestApp, token: &str, ruta: &str) -> (StatusCode, Vec<u8>) {
    let respuesta = t
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(ruta)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let estado = respuesta.status();
    let bytes = respuesta.into_body().collect().await.unwrap().to_bytes().to_vec();
    (estado, bytes)
}

#[tokio::test]
async fn administrativo_crea_documenta_exporta_y_consulta_reportes() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRATIVO").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    let (estado, creado) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok),
        Some(darfe("DARFE-001", "Persona Uno")),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{creado}");
    let id = creado["id"].as_str().unwrap();
    assert_eq!(creado["estatus"], "PENDIENTE");
    assert_eq!(creado["campos_extra"][0]["nombre"], "Teléfono");

    let (estado, listado) = pedir(
        &t.app,
        "GET",
        "/api/v1/darfes?buscar=Persona&estatus=PENDIENTE",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{listado}");
    assert_eq!(listado["total"], 1);
    assert_eq!(listado["darfes"][0]["numero_darfe"], "DARFE-001");

    let (estado, documento) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/darfes/{id}/documentos"),
        Some(&tok),
        Some(json!({
            "campo": "CURP",
            "nombre": "curp.pdf",
            "mime": "application/pdf",
            "contenido_base64": "JVBERi0="
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{documento}");

    let (estado, pdf) = descargar(&t, &tok, &format!("/api/v1/darfes/{id}/documentos/CURP")).await;
    assert_eq!(estado, StatusCode::OK);
    assert!(pdf.starts_with(b"%PDF"));

    let (estado, xlsx) = descargar(&t, &tok, "/api/v1/darfes/exportar?estatus=PENDIENTE").await;
    assert_eq!(estado, StatusCode::OK);
    assert!(xlsx.starts_with(b"PK"));

    let (estado, reportes) = pedir(&t.app, "GET", "/api/v1/darfes/reportes", Some(&tok), None).await;
    assert_eq!(estado, StatusCode::OK, "{reportes}");
    assert_eq!(reportes["total"], 1);
    assert_eq!(reportes["por_estatus"]["PENDIENTE"], 1);
    assert_eq!(reportes["documentos"]["CURP"], 1);

    let (estado, actualizado) = pedir(
        &t.app,
        "PUT",
        &format!("/api/v1/darfes/{id}"),
        Some(&tok),
        Some({
            let mut cuerpo = darfe("DARFE-001", "Persona Actualizada");
            cuerpo["estatus"] = json!("COMPLETO");
            cuerpo
        }),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{actualizado}");
    assert_eq!(actualizado["estatus"], "COMPLETO");

    let (estado, _) = pedir(
        &t.app,
        "DELETE",
        &format!("/api/v1/darfes/{id}"),
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (_, despues) = pedir(&t.app, "GET", "/api/v1/darfes", Some(&tok), None).await;
    assert_eq!(despues["total"], 0);

    t.cerrar().await;
}

#[tokio::test]
async fn rechaza_duplicados_y_usuarios_sin_permiso() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRATIVO").await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok),
        Some(darfe("DARFE-002", "Persona Dos")),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok),
        Some(darfe("DARFE-002", "Duplicada")),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT);

    let (tok_usuario, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;
    let (estado, _) = pedir(&t.app, "GET", "/api/v1/darfes", Some(&tok_usuario), None).await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    t.cerrar().await;
}
