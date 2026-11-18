//! El administrador crea pedidos masivos a nombre de otro usuario.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use common::*;

#[tokio::test]
async fn administrador_asigna_un_pedido_a_otro_usuario() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    let destino = crear_usuario(&t.db, "destino@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    let exp1 = crear_expediente(&t.db, "EXP-ASIG-1").await;
    let exp2 = crear_expediente(&t.db, "EXP-ASIG-2").await;
    let (tok_admin, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos/asignar",
        Some(&tok_admin),
        Some(json!({ "destino_id": destino, "expedientes": [exp1, exp2] })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    assert_eq!(cuerpo["solicitante_id"], destino.to_string());
    assert_eq!(cuerpo["total"], 2);
    let pedido_id = cuerpo["pedido_id"].as_str().unwrap();

    let solicitante: Uuid = sqlx::query_scalar("SELECT solicitante_id FROM pedidos WHERE id = $1")
        .bind(Uuid::parse_str(pedido_id).unwrap())
        .fetch_one(&t.db)
        .await
        .unwrap();
    assert_eq!(solicitante, destino);

    let items: i64 = sqlx::query_scalar("SELECT count(*) FROM pedido_items WHERE pedido_id = $1")
        .bind(Uuid::parse_str(pedido_id).unwrap())
        .fetch_one(&t.db)
        .await
        .unwrap();
    assert_eq!(items, 2);

    let auditorias: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auditoria WHERE accion = 'pedidos.asignar' AND entidad_id = $1",
    )
    .bind(Uuid::parse_str(pedido_id).unwrap())
    .fetch_one(&t.db)
    .await
    .unwrap();
    assert_eq!(auditorias, 1);

    let (tok_destino, _) = iniciar_sesion(&t.app, "destino@test.mx").await;
    let (estado, propios) = pedir(&t.app, "GET", "/api/v1/pedidos", Some(&tok_destino), None).await;
    assert_eq!(estado, StatusCode::OK, "{propios}");
    assert!(
        propios["pedidos"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == pedido_id),
        "el destinatario debe ver el pedido asignado: {propios}"
    );

    let custodias: i64 = sqlx::query_scalar("SELECT count(*) FROM custodias WHERE expediente_id = ANY($1)")
        .bind(&vec![exp1, exp2])
        .fetch_one(&t.db)
        .await
        .unwrap();
    assert_eq!(custodias, 0, "la asignación no debe cambiar la custodia");

    t.cerrar().await;
}

#[tokio::test]
async fn solo_quien_puede_asignar_crea_pedidos_ajenos() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    crear_usuario(&t.db, "destino@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "actor@test.mx", "USUARIO").await;
    let destino = crear_usuario(&t.db, "otro@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-ASIG-3").await;
    let (tok_actor, _) = iniciar_sesion(&t.app, "actor@test.mx").await;

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos/asignar",
        Some(&tok_actor),
        Some(json!({ "destino_id": destino, "expedientes": [exp] })),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    t.cerrar().await;
}

#[tokio::test]
async fn rechaza_destino_inactivo_y_expedientes_invalidos() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    let destino = crear_usuario(&t.db, "destino@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-ASIG-4").await;
    let (tok_admin, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    sqlx::query("UPDATE usuarios SET activo = false WHERE id = $1")
        .bind(destino)
        .execute(&t.db)
        .await
        .unwrap();
    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos/asignar",
        Some(&tok_admin),
        Some(json!({ "destino_id": destino, "expedientes": [exp] })),
    )
    .await;
    assert_eq!(estado, StatusCode::UNPROCESSABLE_ENTITY);

    sqlx::query("UPDATE usuarios SET activo = true WHERE id = $1")
        .bind(destino)
        .execute(&t.db)
        .await
        .unwrap();

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos/asignar",
        Some(&tok_admin),
        Some(json!({ "destino_id": destino, "expedientes": [exp, exp] })),
    )
    .await;
    assert_eq!(estado, StatusCode::BAD_REQUEST);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos/asignar",
        Some(&tok_admin),
        Some(json!({ "destino_id": destino, "expedientes": [Uuid::new_v4()] })),
    )
    .await;
    assert_eq!(estado, StatusCode::UNPROCESSABLE_ENTITY);

    t.cerrar().await;
}
