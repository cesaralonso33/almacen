//! Integraciones externas: consulta de expedientes y Paperless-ngx.

mod common;

use axum::{
    extract::Path,
    http::{header, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

use common::*;

async fn administrador(t: &TestApp) -> String {
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    iniciar_sesion(&t.app, "admin@test.mx").await.0
}

async fn iniciar_mock_expediente() -> String {
    let app = Router::new().route(
        "/BuscaExpediente",
        post(|Json(cuerpo): Json<Value>| async move {
            let numero = cuerpo
                .get("expediente")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            Json(json!({
                "expediente": numero,
                "nombre": "Persona de Prueba",
                "correo": "",
                "correo2": "persona@ejemplo.mx",
                "idfud": 123,
                "tel1": "",
                "tel2": "5555555555"
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let direccion = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{direccion}/BuscaExpediente")
}

async fn iniciar_mock_paperless() -> String {
    let app = Router::new()
        .route(
            "/api/documents/{id}/download/",
            get(|Path(id): Path<String>| async move {
                (
                    [
                        (header::CONTENT_TYPE, "application/pdf"),
                        (
                            header::CONTENT_DISPOSITION,
                            "inline; filename=\"documento-paperless.pdf\"",
                        ),
                    ],
                    format!("%PDF-1.4\n% documento {id}\n"),
                )
            }),
        )
        .route(
            "/api/documents/",
            get(|| async { Json(json!({ "count": 0, "results": [] })) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let direccion = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{direccion}")
}

#[tokio::test]
async fn administrador_configura_y_consulta_expediente() {
    let t = setup().await;
    let tok_admin = administrador(&t).await;
    crear_usuario(&t.db, "aux@test.mx", "ADMINISTRATIVO").await;
    let (tok_aux, _) = iniciar_sesion(&t.app, "aux@test.mx").await;
    let url = iniciar_mock_expediente().await;

    let (estado, _) = pedir(
        &t.app,
        "PUT",
        "/api/v1/integraciones/EXPEDIENTE",
        Some(&tok_aux),
        Some(json!({ "url": url, "token": "token", "activo": true })),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    let (estado, configurada) = pedir(
        &t.app,
        "PUT",
        "/api/v1/integraciones/EXPEDIENTE",
        Some(&tok_admin),
        Some(json!({ "url": url, "token": "token-prueba", "activo": true })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{configurada}");
    assert_eq!(configurada["token_configurado"], true);
    assert!(configurada.get("token").is_none(), "el token no se devuelve");

    let (estado, listado) = pedir(
        &t.app,
        "GET",
        "/api/v1/integraciones",
        Some(&tok_admin),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{listado}");
    assert!(listado["integraciones"][0].get("token").is_none());

    let (estado, datos) = pedir(
        &t.app,
        "POST",
        "/api/v1/integraciones/expediente/consultar",
        Some(&tok_aux),
        Some(json!({ "expediente": "2898/2026" })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{datos}");
    assert_eq!(datos["nombre"], "Persona de Prueba");
    assert_eq!(datos["correo"], "persona@ejemplo.mx");
    assert_eq!(datos["telefono"], "5555555555");
    assert!(datos["curp"].is_null());
    assert_eq!(datos["curp_disponible"], false);

    t.cerrar().await;
}

#[tokio::test]
async fn importa_un_documento_desde_paperless() {
    let t = setup().await;
    let tok_admin = administrador(&t).await;
    crear_usuario(&t.db, "aux@test.mx", "ADMINISTRATIVO").await;
    let (tok_aux, _) = iniciar_sesion(&t.app, "aux@test.mx").await;
    let url = iniciar_mock_paperless().await;

    let (estado, configurada) = pedir(
        &t.app,
        "PUT",
        "/api/v1/integraciones/PAPERLESS",
        Some(&tok_admin),
        Some(json!({ "url": url, "token": "token-paperless", "activo": true })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{configurada}");

    let (estado, creado) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok_aux),
        Some(json!({
            "numero_darfe": "DARFE-PAPERLESS-1",
            "fecha": "2026-11-23",
            "nombre": "Persona Paperless",
            "monto": 100.0,
            "tipo": "OTRAS AYUDAS",
            "estatus": "PENDIENTE"
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{creado}");
    let id = creado["id"].as_str().unwrap();

    let (estado, importado) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/darfes/{id}/paperless"),
        Some(&tok_aux),
        Some(json!({ "documento_id": "42", "campo": "CURP" })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{importado}");
    assert_eq!(importado["campo"], "CURP");
    assert_eq!(importado["nombre"], "documento-paperless.pdf");

    let (estado, detalle) = pedir(
        &t.app,
        "GET",
        &format!("/api/v1/darfes/{id}"),
        Some(&tok_aux),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{detalle}");
    let documentos = detalle["documentos"].as_array().unwrap();
    assert!(
        documentos.iter().any(|documento| documento == "CURP"),
        "el documento importado no aparece: {detalle}"
    );

    t.cerrar().await;
}
