//! Consultas de lectura para el tablero y las bandejas de la interfaz.

mod common;

use axum::http::StatusCode;
use serde_json::json;

use common::*;

#[tokio::test]
async fn dashboard_refleja_los_conteos_operativos() {
    let t = setup().await;
    crear_usuario(&t.db, "solicitante@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    let exp = crear_expediente(&t.db, "EXP-CON-1").await;
    let tok_sol = poner_en_custodia(&t, "solicitante@test.mx", exp).await.0;
    let (tok_alm, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;

    let otro = crear_expediente(&t.db, "EXP-CON-2").await;
    pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&tok_sol),
        Some(json!({ "expedientes": [otro] })),
    )
    .await;

    let (estado, cuerpo) = pedir(&t.app, "GET", "/api/v1/dashboard", Some(&tok_alm), None).await;

    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    assert!(cuerpo["custodias_activas"].as_i64().unwrap() >= 1);
    assert!(cuerpo["pedidos_pendientes"].as_i64().unwrap() >= 1);
    assert!(cuerpo["pedidos_por_preparar"].as_i64().unwrap() >= 1);
    assert!(cuerpo["en_entrega"].as_i64().unwrap() >= 0);
    assert!(cuerpo["incidencias"].as_i64().is_some());

    t.cerrar().await;
}

#[tokio::test]
async fn un_usuario_sin_rol_de_almacen_solo_ve_sus_pedidos() {
    let t = setup().await;
    crear_usuario(&t.db, "ana@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "beto@test.mx", "USUARIO").await;
    let e1 = crear_expediente(&t.db, "EXP-CON-A").await;
    let e2 = crear_expediente(&t.db, "EXP-CON-B").await;

    let (tok_ana, _) = iniciar_sesion(&t.app, "ana@test.mx").await;
    let (tok_beto, _) = iniciar_sesion(&t.app, "beto@test.mx").await;

    pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&tok_ana),
        Some(json!({ "expedientes": [e1] })),
    )
    .await;
    pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&tok_beto),
        Some(json!({ "expedientes": [e2] })),
    )
    .await;

    let (estado, cuerpo) = pedir(&t.app, "GET", "/api/v1/pedidos", Some(&tok_ana), None).await;

    assert_eq!(estado, StatusCode::OK);
    let pedidos = cuerpo["pedidos"].as_array().unwrap();
    assert_eq!(pedidos.len(), 1, "solo debe ver su pedido: {cuerpo}");
    assert_eq!(pedidos[0]["solicitante_nombre"], "ana@test.mx");
    assert_eq!(pedidos[0]["total"], 1);

    t.cerrar().await;
}

#[tokio::test]
async fn almacen_ve_todos_los_pedidos() {
    let t = setup().await;
    crear_usuario(&t.db, "ana@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "beto@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    let e1 = crear_expediente(&t.db, "EXP-CON-C").await;
    let e2 = crear_expediente(&t.db, "EXP-CON-D").await;

    let (tok_ana, _) = iniciar_sesion(&t.app, "ana@test.mx").await;
    let (tok_beto, _) = iniciar_sesion(&t.app, "beto@test.mx").await;
    let (tok_alm, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;

    for (token, exp) in [(&tok_ana, e1), (&tok_beto, e2)] {
        pedir(
            &t.app,
            "POST",
            "/api/v1/pedidos",
            Some(token),
            Some(json!({ "expedientes": [exp] })),
        )
        .await;
    }

    let (_, cuerpo) = pedir(&t.app, "GET", "/api/v1/pedidos", Some(&tok_alm), None).await;
    assert_eq!(cuerpo["pedidos"].as_array().unwrap().len(), 2);

    t.cerrar().await;
}

#[tokio::test]
async fn custodias_lista_solo_las_del_usuario_y_su_historial() {
    let t = setup().await;
    crear_usuario(&t.db, "solicitante@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "otro@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    let exp = crear_expediente(&t.db, "EXP-CON-3").await;
    let (tok_sol, _) = poner_en_custodia(&t, "solicitante@test.mx", exp).await;
    let (tok_otro, _) = iniciar_sesion(&t.app, "otro@test.mx").await;

    let (estado, cuerpo) = pedir(&t.app, "GET", "/api/v1/custodias", Some(&tok_sol), None).await;
    assert_eq!(estado, StatusCode::OK);
    let custodias = cuerpo["custodias"].as_array().unwrap();
    assert_eq!(custodias.len(), 1);
    assert_eq!(custodias[0]["numero"], "EXP-CON-3");

    let (_, ajeno) = pedir(&t.app, "GET", "/api/v1/custodias", Some(&tok_otro), None).await;
    assert_eq!(ajeno["custodias"].as_array().unwrap().len(), 0);

    let (estado, historial) = pedir(
        &t.app,
        "GET",
        &format!("/api/v1/expedientes/{exp}/historial"),
        Some(&tok_sol),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{historial}");
    let movimientos = historial["movimientos"].as_array().unwrap();
    assert!(movimientos.iter().any(|m| m["tipo"] == "PREPARACION"));
    assert!(movimientos.iter().any(|m| m["tipo"] == "RECEPCION"));

    t.cerrar().await;
}

#[tokio::test]
async fn busqueda_de_expedientes_por_numero_y_descripcion() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;
    sqlx::query("INSERT INTO expedientes(numero, descripcion) VALUES ('AMP-2026-9','Juicio de amparo')")
        .execute(&t.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO expedientes(numero, descripcion) VALUES ('LAB-2026-1','Demanda laboral')")
        .execute(&t.db)
        .await
        .unwrap();

    let (estado, cuerpo) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?buscar=amparo",
        Some(&tok),
        None,
    )
    .await;

    assert_eq!(estado, StatusCode::OK);
    let encontrados = cuerpo["expedientes"].as_array().unwrap();
    assert_eq!(encontrados.len(), 1);
    assert_eq!(encontrados[0]["numero"], "AMP-2026-9");

    t.cerrar().await;
}

#[tokio::test]
async fn entregas_pendientes_de_acuse_solo_para_el_receptor() {
    let t = setup().await;
    crear_usuario(&t.db, "solicitante@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "otro@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    let exp = crear_expediente(&t.db, "EXP-CON-4").await;

    let (tok_sol, _) = iniciar_sesion(&t.app, "solicitante@test.mx").await;
    let (tok_alm, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;

    let (_, pedido) = pedir(
        &t.app,
        "POST",
        "/api/v1/pedidos",
        Some(&tok_sol),
        Some(json!({ "expedientes": [exp] })),
    )
    .await;
    let pid = pedido["pedido_id"].as_str().unwrap().to_string();
    let (_, detalle) = pedir(&t.app, "GET", &format!("/api/v1/pedidos/{pid}"), Some(&tok_alm), None).await;
    let item = detalle["items"][0]["id"].as_str().unwrap().to_string();
    pedir(&t.app, "POST", &format!("/api/v1/items/{item}/preparar"), Some(&tok_alm), Some(json!({}))).await;
    pedir(
        &t.app,
        "POST",
        "/api/v1/entregas",
        Some(&tok_alm),
        Some(json!({ "item_ids": [item] })),
    )
    .await;

    let (estado, cuerpo) = pedir(&t.app, "GET", "/api/v1/entregas/pendientes", Some(&tok_sol), None).await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    assert_eq!(cuerpo["entregas"].as_array().unwrap().len(), 1);

    let (_, otro) = pedir(
        &t.app,
        "GET",
        "/api/v1/entregas/pendientes",
        Some(&iniciar_sesion(&t.app, "otro@test.mx").await.0),
        None,
    )
    .await;
    assert_eq!(otro["entregas"].as_array().unwrap().len(), 0);

    t.cerrar().await;
}


#[tokio::test]
async fn directorio_excluye_al_propio_usuario_y_a_los_inactivos() {
    let t = setup().await;
    crear_usuario(&t.db, "ana@test.mx", "USUARIO").await;
    let beto = crear_usuario(&t.db, "beto@test.mx", "USUARIO").await;
    let inactivo = crear_usuario(&t.db, "inactivo@test.mx", "USUARIO").await;
    sqlx::query("UPDATE usuarios SET activo = false WHERE id = $1")
        .bind(inactivo)
        .execute(&t.db)
        .await
        .unwrap();

    let (tok, _) = iniciar_sesion(&t.app, "ana@test.mx").await;
    let (estado, cuerpo) = pedir(&t.app, "GET", "/api/v1/usuarios/directorio", Some(&tok), None).await;

    assert_eq!(estado, StatusCode::OK);
    let usuarios = cuerpo["usuarios"].as_array().unwrap();
    assert_eq!(usuarios.len(), 1, "{cuerpo}");
    assert_eq!(usuarios[0]["id"], beto.to_string());

    t.cerrar().await;
}
