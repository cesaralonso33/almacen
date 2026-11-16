//! Devoluciones, transferencias de custodia y cancelación de pedidos.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use common::*;

async fn preparar_usuarios(t: &TestApp) -> (Uuid, Uuid) {
    let solicitante = crear_usuario(&t.db, "solicitante@test.mx", "USUARIO").await;
    let otro = crear_usuario(&t.db, "otro@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    (solicitante, otro)
}

#[tokio::test]
async fn devolucion_no_cierra_custodia_hasta_el_acuse_del_almacen() {
    let t = setup().await;
    preparar_usuarios(&t).await;
    let exp = crear_expediente(&t.db, "EXP-DEV-1").await;
    let (tok_sol, _) = poner_en_custodia(&t, "solicitante@test.mx", exp).await;
    let (tok_alm, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;

    let (estado, dev) = pedir(
        &t.app,
        "POST",
        "/api/v1/devoluciones",
        Some(&tok_sol),
        Some(json!({ "expediente_id": exp, "observaciones": "Se entrega en recepción" })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{dev}");
    let dev_id = dev["devolucion_id"].as_str().unwrap().to_string();

    assert_eq!(custodias_activas(&t.db, exp).await, 1, "la custodia sigue con el custodio");

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/devoluciones/{dev_id}/confirmar"),
        Some(&tok_sol),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN, "el custodio no puede acusar por el almacén: {cuerpo}");

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/devoluciones/{dev_id}/confirmar"),
        Some(&tok_alm),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    assert_eq!(custodias_activas(&t.db, exp).await, 0, "la custodia se cierra con el acuse");

    let estado_expediente: String =
        sqlx::query_scalar("SELECT estado FROM expedientes WHERE id = $1")
            .bind(exp)
            .fetch_one(&t.db)
            .await
            .unwrap();
    assert_eq!(estado_expediente, "ALMACEN");

    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/devoluciones/{dev_id}/confirmar"),
        Some(&tok_alm),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT, "no se puede acusar dos veces");

    t.cerrar().await;
}

#[tokio::test]
async fn solo_el_custodio_puede_solicitar_la_devolucion() {
    let t = setup().await;
    preparar_usuarios(&t).await;
    let exp = crear_expediente(&t.db, "EXP-DEV-2").await;
    poner_en_custodia(&t, "solicitante@test.mx", exp).await;
    let (tok_otro, _) = iniciar_sesion(&t.app, "otro@test.mx").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/devoluciones",
        Some(&tok_otro),
        Some(json!({ "expediente_id": exp })),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT, "{cuerpo}");

    t.cerrar().await;
}

#[tokio::test]
async fn transferencia_no_cambia_custodia_hasta_que_el_destino_acepta() {
    let t = setup().await;
    let (_, otro) = preparar_usuarios(&t).await;
    let exp = crear_expediente(&t.db, "EXP-TRA-1").await;
    let (tok_sol, _) = poner_en_custodia(&t, "solicitante@test.mx", exp).await;
    let (tok_otro, _) = iniciar_sesion(&t.app, "otro@test.mx").await;

    let (estado, tr) = pedir(
        &t.app,
        "POST",
        "/api/v1/transferencias",
        Some(&tok_sol),
        Some(json!({ "expediente_id": exp, "destino_id": otro })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{tr}");
    let tr_id = tr["transferencia_id"].as_str().unwrap().to_string();

    let custodio: Uuid = sqlx::query_scalar(
        "SELECT usuario_id FROM custodias WHERE expediente_id = $1 AND fin IS NULL",
    )
    .bind(exp)
    .fetch_one(&t.db)
    .await
    .unwrap();
    assert_eq!(custodio, crear_id(&t, "solicitante@test.mx").await);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/transferencias/{tr_id}/aceptar"),
        Some(&tok_sol),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN, "solo el destino acepta");

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/transferencias/{tr_id}/aceptar"),
        Some(&tok_otro),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    assert_eq!(custodias_activas(&t.db, exp).await, 1, "una sola custodia activa");

    let nuevo: Uuid = sqlx::query_scalar(
        "SELECT usuario_id FROM custodias WHERE expediente_id = $1 AND fin IS NULL",
    )
    .bind(exp)
    .fetch_one(&t.db)
    .await
    .unwrap();
    assert_eq!(nuevo, otro);

    t.cerrar().await;
}

#[tokio::test]
async fn transferencia_rechazada_no_cambia_la_custodia() {
    let t = setup().await;
    let (_, otro) = preparar_usuarios(&t).await;
    let exp = crear_expediente(&t.db, "EXP-TRA-2").await;
    let (tok_sol, _) = poner_en_custodia(&t, "solicitante@test.mx", exp).await;
    let (tok_otro, _) = iniciar_sesion(&t.app, "otro@test.mx").await;

    let (_, tr) = pedir(
        &t.app,
        "POST",
        "/api/v1/transferencias",
        Some(&tok_sol),
        Some(json!({ "expediente_id": exp, "destino_id": otro })),
    )
    .await;
    let tr_id = tr["transferencia_id"].as_str().unwrap().to_string();

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/transferencias/{tr_id}/rechazar"),
        Some(&tok_otro),
        Some(json!({})),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    assert_eq!(custodias_activas(&t.db, exp).await, 1);

    let custodio: Uuid = sqlx::query_scalar(
        "SELECT usuario_id FROM custodias WHERE expediente_id = $1 AND fin IS NULL",
    )
    .bind(exp)
    .fetch_one(&t.db)
    .await
    .unwrap();
    assert_eq!(custodio, crear_id(&t, "solicitante@test.mx").await);

    t.cerrar().await;
}

#[tokio::test]
async fn dos_aceptaciones_simultaneas_solo_aplican_una() {
    let t = setup().await;
    let (_, otro) = preparar_usuarios(&t).await;
    let exp = crear_expediente(&t.db, "EXP-TRA-3").await;
    let (tok_sol, _) = poner_en_custodia(&t, "solicitante@test.mx", exp).await;
    let (tok_otro, _) = iniciar_sesion(&t.app, "otro@test.mx").await;

    let (_, tr) = pedir(
        &t.app,
        "POST",
        "/api/v1/transferencias",
        Some(&tok_sol),
        Some(json!({ "expediente_id": exp, "destino_id": otro })),
    )
    .await;
    let tr_id = tr["transferencia_id"].as_str().unwrap().to_string();
    let ruta = format!("/api/v1/transferencias/{tr_id}/aceptar");

    let (a, b) = tokio::join!(
        pedir(&t.app, "POST", &ruta, Some(&tok_otro), Some(json!({}))),
        pedir(&t.app, "POST", &ruta, Some(&tok_otro), Some(json!({}))),
    );

    let codigos = [a.0, b.0];
    assert!(codigos.contains(&StatusCode::OK), "una debe aceptarse: {codigos:?}");
    assert!(
        codigos.contains(&StatusCode::CONFLICT),
        "la otra debe rechazarse con conflicto: {codigos:?}"
    );
    assert_eq!(custodias_activas(&t.db, exp).await, 1);
    let aceptadas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM transferencias WHERE expediente_id = $1 AND estado = 'ACEPTADA'")
            .bind(exp)
            .fetch_one(&t.db)
            .await
            .unwrap();
    assert_eq!(aceptadas, 1);

    t.cerrar().await;
}

#[tokio::test]
async fn devolucion_y_transferencia_son_mutuamente_excluyentes() {
    let t = setup().await;
    let (_, otro) = preparar_usuarios(&t).await;
    let exp = crear_expediente(&t.db, "EXP-DEV-3").await;
    let (tok_sol, _) = poner_en_custodia(&t, "solicitante@test.mx", exp).await;

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/devoluciones",
        Some(&tok_sol),
        Some(json!({ "expediente_id": exp })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        "/api/v1/transferencias",
        Some(&tok_sol),
        Some(json!({ "expediente_id": exp, "destino_id": otro })),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT, "hay una devolución en curso: {cuerpo}");

    t.cerrar().await;
}

#[tokio::test]
async fn cancelar_pedido_libera_renglones_sin_borrar_historial() {
    let t = setup().await;
    preparar_usuarios(&t).await;
    let e1 = crear_expediente(&t.db, "EXP-CAN-1").await;
    let e2 = crear_expediente(&t.db, "EXP-CAN-2").await;
    let (tok_sol, _) = iniciar_sesion(&t.app, "solicitante@test.mx").await;
    let (tok_alm, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;

    let (_, pedido) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&tok_sol),
        Some(json!({ "expedientes": [e1, e2] })),
    )
    .await;
    let pid = pedido["pedido_id"].as_str().unwrap().to_string();

    let (_, detalle) = pedir(&t.app, "GET", &format!("/api/v1/pedidos/{pid}"), Some(&tok_sol), None).await;
    let item = detalle["items"][0]["id"].as_str().unwrap().to_string();
    pedir(&t.app, "POST", &format!("/api/v1/items/{item}/preparar"), Some(&tok_alm), Some(json!({}))).await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/pedidos/{pid}/cancelar"),
        Some(&tok_sol),
        Some(json!({ "motivo": "El área ya no requiere los expedientes" })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");

    let (_, final_) = pedir(&t.app, "GET", &format!("/api/v1/pedidos/{pid}"), Some(&tok_sol), None).await;
    assert_eq!(final_["estado"], "CANCELADO", "{final_}");
    for item_json in final_["items"].as_array().unwrap() {
        assert_eq!(item_json["estado"], "CANCELADO");
    }

    let reservas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM reservas WHERE liberada_en IS NULL AND pedido_item_id = $1")
            .bind(item.parse::<Uuid>().unwrap())
            .fetch_one(&t.db)
            .await
            .unwrap();
    assert_eq!(reservas, 0, "la reserva debe liberarse");

    let pedidos: i64 = sqlx::query_scalar("SELECT count(*) FROM pedidos WHERE id = $1")
        .bind(pid.parse::<Uuid>().unwrap())
        .fetch_one(&t.db)
        .await
        .unwrap();
    assert_eq!(pedidos, 1, "el pedido no se borra");

    t.cerrar().await;
}

#[tokio::test]
async fn no_se_cancela_un_pedido_con_entregas_confirmadas() {
    let t = setup().await;
    preparar_usuarios(&t).await;
    let exp = crear_expediente(&t.db, "EXP-CAN-3").await;
    let (tok_sol, pid) = poner_en_custodia(&t, "solicitante@test.mx", exp).await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/pedidos/{pid}/cancelar"),
        Some(&tok_sol),
        Some(json!({ "motivo": "Intento inválido" })),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT, "{cuerpo}");

    t.cerrar().await;
}

async fn crear_id(t: &TestApp, correo: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM usuarios WHERE correo = $1")
        .bind(correo)
        .fetch_one(&t.db)
        .await
        .unwrap()
}

