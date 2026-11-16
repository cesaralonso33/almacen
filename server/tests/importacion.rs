//! Importación masiva de expedientes.

mod common;

use axum::http::StatusCode;
use chrono::Datelike;
use serde_json::json;
use uuid::Uuid;

use common::*;

#[tokio::test]
async fn importa_lista_normaliza_y_omite_duplicados() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    // Existente: se debe omitir en la importación.
    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok),
        Some(json!({ "numero": "100/2026" })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (estado, resultado) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes/importar",
        Some(&tok),
        Some(json!({
            "expedientes": [
                { "numero": "5259/2026", "unidad": "JUZGADO 1", "descripcion": "Demanda" },
                { "numero": "100/2026" },
                { "numero": "00000101/2026", "anio": 2020 },
                { "numero": "   " }
            ]
        })),
    )
    .await;

    assert_eq!(estado, StatusCode::OK, "{resultado}");
    assert_eq!(resultado["importados"], 2, "{resultado}");
    assert_eq!(resultado["omitidos"], 1, "{resultado}");
    assert_eq!(resultado["errores"].as_array().unwrap().len(), 1);

    let (estado_expediente, custodio, anio): (String, Option<Uuid>, i16) = sqlx::query_as(
        "SELECT estado, custodio_id, anio FROM expedientes WHERE numero = '00005259/2026'",
    )
    .fetch_one(&t.db)
    .await
    .expect("expediente importado y normalizado");

    assert_eq!(estado_expediente, "ALMACEN", "la importación no asigna custodia");
    assert!(custodio.is_none());
    assert_eq!(anio as i32, chrono::Utc::now().year());

    t.cerrar().await;
}

#[tokio::test]
async fn importacion_requiere_permiso_y_respeta_el_limite() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "consulta@test.mx", "CONSULTA").await;
    let (tok_usuario, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;
    let (tok_consulta, _) = iniciar_sesion(&t.app, "consulta@test.mx").await;

    for token in [&tok_usuario, &tok_consulta] {
        let (estado, _) = pedir(
            &t.app,
            "POST",
            "/api/v1/expedientes/importar",
            Some(token),
            Some(json!({ "expedientes": [{ "numero": "A/2026" }] })),
        )
        .await;
        assert_eq!(estado, StatusCode::FORBIDDEN);
    }

    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    let (tok_admin, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    let filas: Vec<serde_json::Value> = (0..501)
        .map(|i| json!({ "numero": format!("MASIVO-{i}") }))
        .collect();
    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes/importar",
        Some(&tok_admin),
        Some(json!({ "expedientes": filas })),
    )
    .await;
    assert_eq!(estado, StatusCode::UNPROCESSABLE_ENTITY);

    t.cerrar().await;
}
