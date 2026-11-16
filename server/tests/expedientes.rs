//! Alta de expedientes en el inventario.

mod common;

use axum::http::StatusCode;
use serde_json::json;

use common::*;

#[tokio::test]
async fn alta_de_expediente_requiere_permiso_y_respeta_unicidad() {
    let t = setup().await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;

    let (tok_alm, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;
    let (tok_usr, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let cuerpo = json!({
        "numero": "EXP-2026-0100",
        "descripcion": "Demanda laboral",
        "unidad": "JUZGADO 1",
        "anio": 2026
    });

    let (estado, respuesta) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_alm),
        Some(cuerpo.clone()),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{respuesta}");
    assert!(respuesta["expediente_id"].as_str().is_some());

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_alm),
        Some(cuerpo),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT, "no debe duplicarse");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_usr),
        Some(json!({ "numero": "EXP-2026-0101" })),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    let (estado, busqueda) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?buscar=EXP-2026-0100",
        Some(&tok_usr),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK);
    assert_eq!(busqueda["expedientes"].as_array().unwrap().len(), 1);

    t.cerrar().await;
}
