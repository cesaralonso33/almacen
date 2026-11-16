//! Protocolo de sincronización offline: snapshot, push idempotente y pull incremental.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use common::*;

struct Sincronizacion {
    token: String,
    dispositivo: String,
}

async fn preparar(t: &TestApp, correo: &str) -> Sincronizacion {
    let (token, _) = iniciar_sesion(&t.app, correo).await;
    let dispositivo = registrar_dispositivo(&t.app, &token, "Equipo de pruebas").await;
    Sincronizacion { token, dispositivo }
}

async fn pull(t: &TestApp, s: &Sincronizacion, cursor: i64, limite: i64) -> serde_json::Value {
    let (estado, cuerpo) = pedir(
        &t.app,
        "GET",
        &format!(
            "/api/v1/sync/pull?dispositivo_id={}&cursor={cursor}&limite={limite}",
            s.dispositivo
        ),
        Some(&s.token),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    cuerpo
}

#[tokio::test]
async fn snapshot_inicial_incluye_expedientes_y_cursor() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-SYNC-1").await;
    let s = preparar(&t, "usuario@test.mx").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "GET",
        &format!("/api/v1/sync/snapshot?dispositivo_id={}", s.dispositivo),
        Some(&s.token),
        None,
    )
    .await;

    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    let expedientes = cuerpo["expedientes"].as_array().expect("expedientes");
    assert!(expedientes.iter().any(|e| e["id"] == exp.to_string().as_str()));
    assert!(cuerpo["cursor"].as_i64().is_some());

    t.cerrar().await;
}

#[tokio::test]
async fn push_crea_el_pedido_y_emite_evento_para_el_solicitante() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-SYNC-2").await;
    let s = preparar(&t, "usuario@test.mx").await;

    let (estado, resultado) = push_operacion(
        &t.app,
        &s.token,
        &s.dispositivo,
        Uuid::new_v4(),
        "CREAR_PEDIDO",
        json!({ "expedientes": [exp] }),
    )
    .await;

    assert_eq!(estado, StatusCode::OK);
    assert_eq!(resultado["estado"], "ACEPTADA", "{resultado}");
    let pedido_id = resultado["resultado"]["pedido_id"].as_str().unwrap().to_string();

    let cuerpo = pull(&t, &s, 0, 100).await;
    let eventos = cuerpo["eventos"].as_array().unwrap();
    assert!(
        eventos
            .iter()
            .any(|e| e["entidad"] == "pedido" && e["entidad_id"] == pedido_id),
        "debe existir un evento del pedido: {cuerpo}"
    );

    t.cerrar().await;
}

#[tokio::test]
async fn push_es_idempotente_con_la_misma_clave() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-SYNC-3").await;
    let s = preparar(&t, "usuario@test.mx").await;
    let clave = Uuid::new_v4();

    let (_, primero) = push_operacion(
        &t.app,
        &s.token,
        &s.dispositivo,
        clave,
        "CREAR_PEDIDO",
        json!({ "expedientes": [exp] }),
    )
    .await;
    let (_, segundo) = push_operacion(
        &t.app,
        &s.token,
        &s.dispositivo,
        clave,
        "CREAR_PEDIDO",
        json!({ "expedientes": [exp] }),
    )
    .await;

    assert_eq!(primero["estado"], "ACEPTADA");
    assert_eq!(segundo["estado"], "ACEPTADA");
    assert_eq!(primero["resultado"]["pedido_id"], segundo["resultado"]["pedido_id"]);

    let pedidos: i64 = sqlx::query_scalar("SELECT count(*) FROM pedidos")
        .fetch_one(&t.db)
        .await
        .unwrap();
    assert_eq!(pedidos, 1, "el reintento no debe duplicar el pedido");

    t.cerrar().await;
}

#[tokio::test]
async fn push_duplicado_concurrente_solo_crea_un_pedido() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-SYNC-4").await;
    let s = preparar(&t, "usuario@test.mx").await;
    let clave = Uuid::new_v4();
    let cuerpo = json!({
        "dispositivo_id": s.dispositivo,
        "operaciones": [{ "clave": clave, "tipo": "CREAR_PEDIDO", "payload": { "expedientes": [exp] } }]
    });

    let (a, b) = tokio::join!(
        pedir(&t.app, "POST", "/api/v1/sync/push", Some(&s.token), Some(cuerpo.clone())),
        pedir(&t.app, "POST", "/api/v1/sync/push", Some(&s.token), Some(cuerpo)),
    );
    assert_eq!(a.0, StatusCode::OK);
    assert_eq!(b.0, StatusCode::OK);

    let pedidos: i64 = sqlx::query_scalar("SELECT count(*) FROM pedidos")
        .fetch_one(&t.db)
        .await
        .unwrap();
    assert_eq!(pedidos, 1, "la clave de idempotencia evita duplicados");

    t.cerrar().await;
}

#[tokio::test]
async fn pull_no_expone_pedidos_de_otros_usuarios() {
    let t = setup().await;
    crear_usuario(&t.db, "ana@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "beto@test.mx", "USUARIO").await;
    let exp_a = crear_expediente(&t.db, "EXP-SYNC-A").await;
    let exp_b = crear_expediente(&t.db, "EXP-SYNC-B").await;

    let ana = preparar(&t, "ana@test.mx").await;
    let beto = preparar(&t, "beto@test.mx").await;

    let (_, pa) = push_operacion(
        &t.app,
        &ana.token,
        &ana.dispositivo,
        Uuid::new_v4(),
        "CREAR_PEDIDO",
        json!({ "expedientes": [exp_a] }),
    )
    .await;
    let (_, pb) = push_operacion(
        &t.app,
        &beto.token,
        &beto.dispositivo,
        Uuid::new_v4(),
        "CREAR_PEDIDO",
        json!({ "expedientes": [exp_b] }),
    )
    .await;

    let pedido_ana = pa["resultado"]["pedido_id"].as_str().unwrap().to_string();
    let pedido_beto = pb["resultado"]["pedido_id"].as_str().unwrap().to_string();

    let cuerpo = pull(&t, &ana, 0, 200).await;
    let ids: Vec<String> = cuerpo["eventos"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["entidad"] == "pedido")
        .map(|e| e["entidad_id"].as_str().unwrap().to_string())
        .collect();

    assert!(ids.contains(&pedido_ana), "debe ver su propio pedido");
    assert!(!ids.contains(&pedido_beto), "no debe ver pedidos ajenos: {cuerpo}");

    t.cerrar().await;
}

#[tokio::test]
async fn pull_pagina_en_orden_y_continua_con_el_cursor() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let s = preparar(&t, "usuario@test.mx").await;

    for i in 1..=3 {
        let exp = crear_expediente(&t.db, &format!("EXP-SYNC-P{i}")).await;
        push_operacion(
            &t.app,
            &s.token,
            &s.dispositivo,
            Uuid::new_v4(),
            "CREAR_PEDIDO",
            json!({ "expedientes": [exp] }),
        )
        .await;
    }

    let primera = pull(&t, &s, 0, 2).await;
    let eventos = primera["eventos"].as_array().unwrap();
    assert_eq!(eventos.len(), 2);
    assert_eq!(primera["hay_mas"], true);
    let secuencias: Vec<i64> = eventos.iter().map(|e| e["secuencia"].as_i64().unwrap()).collect();
    assert!(secuencias[0] < secuencias[1]);

    let cursor = primera["siguiente_cursor"].as_i64().unwrap();
    let segunda = pull(&t, &s, cursor, 100).await;
    let secuencias_2: Vec<i64> = segunda["eventos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["secuencia"].as_i64().unwrap())
        .collect();
    assert!(secuencias_2.iter().all(|s| *s > cursor));

    t.cerrar().await;
}

#[tokio::test]
async fn cursor_fuera_de_rango_pide_reiniciar_con_snapshot() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let s = preparar(&t, "usuario@test.mx").await;

    let cuerpo = pull(&t, &s, 9_999_999, 100).await;
    assert_eq!(cuerpo["reiniciar"], true);
    assert_eq!(cuerpo["eventos"].as_array().unwrap().len(), 0);

    t.cerrar().await;
}

#[tokio::test]
async fn operaciones_no_permitidas_offline_se_rechazan() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let s = preparar(&t, "usuario@test.mx").await;

    let (estado, resultado) = push_operacion(
        &t.app,
        &s.token,
        &s.dispositivo,
        Uuid::new_v4(),
        "CONFIRMAR_ENTREGA",
        json!({ "entrega_id": Uuid::new_v4() }),
    )
    .await;

    assert_eq!(estado, StatusCode::OK);
    assert_eq!(resultado["estado"], "RECHAZADA", "{resultado}");

    t.cerrar().await;
}

#[tokio::test]
async fn no_se_puede_usar_un_dispositivo_ajeno() {
    let t = setup().await;
    crear_usuario(&t.db, "ana@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "beto@test.mx", "USUARIO").await;
    let exp = crear_expediente(&t.db, "EXP-SYNC-5").await;

    let ana = preparar(&t, "ana@test.mx").await;
    let (token_beto, _) = iniciar_sesion(&t.app, "beto@test.mx").await;

    let (estado, cuerpo) = push_operacion(
        &t.app,
        &token_beto,
        &ana.dispositivo,
        Uuid::new_v4(),
        "CREAR_PEDIDO",
        json!({ "expedientes": [exp] }),
    )
    .await;

    assert_eq!(estado, StatusCode::FORBIDDEN, "{cuerpo}");

    t.cerrar().await;
}
