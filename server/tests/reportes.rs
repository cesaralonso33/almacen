//! Reporte de expedientes nuevos agregados a custodia por usuario.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use common::*;

#[tokio::test]
async fn reporte_de_altas_agrupa_por_dia_mes_y_anio() {
    let t = setup().await;
    crear_usuario(&t.db, "ana@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "beto@test.mx", "USUARIO").await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;

    let (tok_ana, _) = iniciar_sesion(&t.app, "ana@test.mx").await;
    let (tok_beto, _) = iniciar_sesion(&t.app, "beto@test.mx").await;
    let (tok_almacen, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;

    let mut expedientes = Vec::new();
    for (token, numero) in [
        (&tok_ana, "ANA-1"),
        (&tok_ana, "ANA-2"),
        (&tok_beto, "BETO-1"),
    ] {
        let (estado, alta) = pedir(
            &t.app,
            "POST",
            "/api/v1/expedientes",
            Some(token),
            Some(json!({ "numero": numero })),
        )
        .await;
        assert_eq!(estado, StatusCode::OK, "{alta}");
        expedientes.push(alta["expediente_id"].as_str().unwrap().to_string());
    }

    // Fechas controladas para verificar la agrupación.
    let fechas = [
        "2026-11-16T15:00:00Z",
        "2026-10-05T15:00:00Z",
        "2025-12-31T15:00:00Z",
    ];
    for (expediente, fecha) in expedientes.iter().zip(fechas.iter()) {
        sqlx::query("UPDATE movimientos SET creado_en = $2 WHERE expediente_id = $1 AND tipo = 'ALTA'")
            .bind(expediente.parse::<Uuid>().unwrap())
            .bind(fecha.parse::<chrono::DateTime<chrono::Utc>>().unwrap())
            .execute(&t.db)
            .await
            .unwrap();
    }

    let (estado, reporte) = pedir(
        &t.app,
        "GET",
        "/api/v1/reportes/altas",
        Some(&tok_almacen),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{reporte}");
    assert_eq!(reporte["total"], 3, "{reporte}");

    let por_dia = reporte["por_dia"].as_array().unwrap();
    assert!(por_dia
        .iter()
        .any(|fila| fila["periodo"] == "2026-11-16" && fila["usuario"] == "ana@test.mx" && fila["total"] == 1));
    assert!(por_dia
        .iter()
        .any(|fila| fila["periodo"] == "2026-10-05" && fila["usuario"] == "ana@test.mx"));
    assert!(por_dia
        .iter()
        .any(|fila| fila["periodo"] == "2025-12-31" && fila["usuario"] == "beto@test.mx"));

    let por_mes = reporte["por_mes"].as_array().unwrap();
    assert!(por_mes.iter().any(|fila| fila["periodo"] == "2026-11"));
    assert!(por_mes.iter().any(|fila| fila["periodo"] == "2026-10"));
    assert!(por_mes.iter().any(|fila| fila["periodo"] == "2025-12"));

    let por_anio = reporte["por_anio"].as_array().unwrap();
    let ana_2026 = por_anio
        .iter()
        .find(|fila| fila["periodo"] == "2026" && fila["usuario"] == "ana@test.mx")
        .expect("ana 2026");
    assert_eq!(ana_2026["total"], 2);

    t.cerrar().await;
}

#[tokio::test]
async fn reporte_de_altas_requiere_permiso() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/reportes/altas", Some(&tok), None).await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    t.cerrar().await;
}
