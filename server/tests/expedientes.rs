//! Alta de expedientes con custodia inicial del creador.

mod common;

use axum::http::StatusCode;
use chrono::Datelike;
use serde_json::json;

use common::*;

#[tokio::test]
async fn el_creador_queda_como_custodio_y_el_anio_es_el_actual() {
    let t = setup().await;
    let usuario = crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, respuesta) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok),
        Some(json!({
            "numero": "EXP-2026-0200",
            "descripcion": "Contrato de arrendamiento",
            "unidad": "JUZGADO 1"
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{respuesta}");

    let expediente_id = respuesta["expediente_id"].as_str().unwrap();

    let (estado_expediente, custodio, anio): (String, Option<uuid::Uuid>, i16) =
        sqlx::query_as("SELECT estado, custodio_id, anio FROM expedientes WHERE id = $1")
            .bind(expediente_id.parse::<uuid::Uuid>().unwrap())
            .fetch_one(&t.db)
            .await
            .unwrap();

    assert_eq!(estado_expediente, "EN_CUSTODIA");
    assert_eq!(custodio, Some(usuario));
    assert_eq!(anio as i32, chrono::Utc::now().year());

    assert_eq!(custodias_activas(&t.db, expediente_id.parse().unwrap()).await, 1);

    let (_, custodias) = pedir(&t.app, "GET", "/api/v1/custodias", Some(&tok), None).await;
    assert_eq!(custodias["custodias"].as_array().unwrap().len(), 1);

    t.cerrar().await;
}

#[tokio::test]
async fn el_creador_puede_devolver_al_almacen_con_acuse() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    let (tok_usuario, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;
    let (tok_almacen, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;

    let (_, alta) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_usuario),
        Some(json!({ "numero": "EXP-2026-0201" })),
    )
    .await;
    let expediente_id = alta["expediente_id"].as_str().unwrap().to_string();

    let (estado, devolucion) = pedir(
        &t.app,
        "POST",
        "/api/v1/devoluciones",
        Some(&tok_usuario),
        Some(json!({ "expediente_id": expediente_id })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{devolucion}");

    let devolucion_id = devolucion["devolucion_id"].as_str().unwrap().to_string();
    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/devoluciones/{devolucion_id}/confirmar"),
        Some(&tok_almacen),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let estado_expediente: String =
        sqlx::query_scalar("SELECT estado FROM expedientes WHERE id = $1")
            .bind(expediente_id.parse::<uuid::Uuid>().unwrap())
            .fetch_one(&t.db)
            .await
            .unwrap();
    assert_eq!(estado_expediente, "ALMACEN");
    assert_eq!(custodias_activas(&t.db, expediente_id.parse().unwrap()).await, 0);

    t.cerrar().await;
}

#[tokio::test]
async fn alta_respeta_permisos_y_unicidad() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "consulta@test.mx", "CONSULTA").await;
    let (tok_usuario, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;
    let (tok_consulta, _) = iniciar_sesion(&t.app, "consulta@test.mx").await;

    let cuerpo = json!({
        "numero": "EXP-2026-0300",
        "unidad": "JUZGADO 1",
        "anio": 2026
    });

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_usuario),
        Some(cuerpo.clone()),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_usuario),
        Some(cuerpo),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT, "no debe duplicarse");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_usuario),
        Some(json!({ "numero": "EXP-2026-0300", "unidad": "JUZGADO 2", "anio": 2024 })),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT, "el número es único globalmente");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_consulta),
        Some(json!({ "numero": "EXP-2026-0301" })),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    t.cerrar().await;
}

#[tokio::test]
async fn normaliza_el_numero_a_ocho_caracteres_antes_de_la_barra() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, alta) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok),
        Some(json!({ "numero": "5259/2026" })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{alta}");
    assert_eq!(alta["numero"], "00005259/2026");

    let guardado: String =
        sqlx::query_scalar("SELECT numero FROM expedientes WHERE id = $1")
            .bind(alta["expediente_id"].as_str().unwrap().parse::<uuid::Uuid>().unwrap())
            .fetch_one(&t.db)
            .await
            .unwrap();
    assert_eq!(guardado, "00005259/2026");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok),
        Some(json!({ "numero": "00005259/2026" })),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT, "el duplicado se compara ya normalizado");

    let (_, completo) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok),
        Some(json!({ "numero": "12345678/2026" })),
    )
    .await;
    assert_eq!(completo["numero"], "12345678/2026");

    let (_, largo) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok),
        Some(json!({ "numero": "123456789/2026" })),
    )
    .await;
    assert_eq!(largo["numero"], "123456789/2026", "no se recorta");

    let (_, sin_barra) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok),
        Some(json!({ "numero": "EXP-1" })),
    )
    .await;
    assert_eq!(sin_barra["numero"], "EXP-1", "sin barra se conserva");

    t.cerrar().await;
}
