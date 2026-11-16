//! Búsqueda avanzada de expedientes con totales y paginación.

mod common;

use axum::http::StatusCode;

use common::*;

#[tokio::test]
async fn busqueda_avanzada_devuelve_total_filtros_y_paginas() {
    let t = setup().await;
    let ana = crear_usuario(&t.db, "ana@test.mx", "USUARIO").await;
    let (tok, _) = iniciar_sesion(&t.app, "ana@test.mx").await;

    sqlx::query(
        "INSERT INTO expedientes(numero, unidad, anio, estado) VALUES
         ('A-1','JUZGADO 1',2026,'ALMACEN'),
         ('A-2','JUZGADO 1',2026,'ALMACEN'),
         ('B-1','JUZGADO 2',2025,'ALMACEN')",
    )
    .execute(&t.db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO expedientes(numero, unidad, anio, estado, custodio_id)
         VALUES ('C-1','JUZGADO 1',2026,'EN_CUSTODIA',$1)",
    )
    .bind(ana)
    .execute(&t.db)
    .await
    .unwrap();

    let (estado, primera) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?pagina=1&tamano=2",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{primera}");
    assert_eq!(primera["total"], 4);
    assert_eq!(primera["expedientes"].as_array().unwrap().len(), 2);
    assert_eq!(primera["por_estado"]["ALMACEN"], 3);
    assert_eq!(primera["por_estado"]["EN_CUSTODIA"], 1);
    assert!(primera["unidades"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u == "JUZGADO 1"));

    let (_, segunda) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?pagina=2&tamano=2",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(segunda["expedientes"].as_array().unwrap().len(), 2);

    let (_, por_estado) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?estado=EN_CUSTODIA",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(por_estado["total"], 1);
    assert_eq!(por_estado["expedientes"][0]["numero"], "C-1");
    assert_eq!(por_estado["por_estado"]["ALMACEN"], 3, "el resumen conserva el resto");

    let (_, combinado) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?unidad=JUZGADO%201&anio=2026",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(combinado["total"], 3);

    let (_, por_custodio) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?custodio=ana",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(por_custodio["total"], 1);

    let (_, por_numero) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?buscar=B-1",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(por_numero["total"], 1);

    t.cerrar().await;
}

#[tokio::test]
async fn busqueda_avanzada_valida_el_tamano_de_pagina() {
    let t = setup().await;
    crear_usuario(&t.db, "ana@test.mx", "USUARIO").await;
    let (tok, _) = iniciar_sesion(&t.app, "ana@test.mx").await;

    let (estado, cuerpo) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?tamano=5000",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");
    assert_eq!(cuerpo["tamano"], 100, "el tamaño se acota a 100");

    let (estado, _) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes?pagina=abc",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::BAD_REQUEST);

    t.cerrar().await;
}
