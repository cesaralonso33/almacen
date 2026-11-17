//! Bandeja de transferencias pendientes de aceptación.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use common::*;

#[tokio::test]
async fn el_destino_ve_y_acepta_la_transferencia() {
    let t = setup().await;
    let (_, beto) = preparar(&t).await;
    let exp = crear_expediente(&t.db, "EXP-TRA-UI-1").await;
    let (tok_ana, _) = poner_en_custodia(&t, "ana@test.mx", exp).await;
    let (tok_beto, _) = iniciar_sesion(&t.app, "beto@test.mx").await;

    let (estado, solicitud) = pedir(
        &t.app,
        "POST",
        "/api/v1/transferencias",
        Some(&tok_ana),
        Some(json!({ "expediente_id": exp, "destino_id": beto })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{solicitud}");

    let (estado, bandeja) = pedir(
        &t.app,
        "GET",
        "/api/v1/transferencias/pendientes",
        Some(&tok_beto),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{bandeja}");
    assert_eq!(bandeja["transferencias"].as_array().unwrap().len(), 1);
    assert_eq!(bandeja["transferencias"][0]["numero"], "EXP-TRA-UI-1");

    let (_, ajena) = pedir(
        &t.app,
        "GET",
        "/api/v1/transferencias/pendientes",
        Some(&tok_ana),
        None,
    )
    .await;
    assert_eq!(ajena["transferencias"].as_array().unwrap().len(), 0);

    let transferencia = bandeja["transferencias"][0]["id"].as_str().unwrap();
    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/transferencias/{transferencia}/aceptar"),
        Some(&tok_beto),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (_, despues) = pedir(
        &t.app,
        "GET",
        "/api/v1/transferencias/pendientes",
        Some(&tok_beto),
        None,
    )
    .await;
    assert_eq!(despues["transferencias"].as_array().unwrap().len(), 0);

    let custodio: Uuid = sqlx::query_scalar(
        "SELECT usuario_id FROM custodias WHERE expediente_id = $1 AND fin IS NULL",
    )
    .bind(exp)
    .fetch_one(&t.db)
    .await
    .unwrap();
    assert_eq!(custodio, beto);

    t.cerrar().await;
}

async fn preparar(t: &TestApp) -> (Uuid, Uuid) {
    let ana = crear_usuario(&t.db, "ana@test.mx", "USUARIO").await;
    let beto = crear_usuario(&t.db, "beto@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    (ana, beto)
}
