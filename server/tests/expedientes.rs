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

#[tokio::test]
async fn solo_administrador_exporta_expedientes() {
    use axum::{
        body::Body,
        http::{header, Request},
    };
    use http_body_util::BodyExt;
    use std::io::Read;
    use tower::ServiceExt;

    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok_admin, _) = iniciar_sesion(&t.app, "admin@test.mx").await;
    let (tok_usuario, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, creado) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_usuario),
        Some(json!({
            "numero": "EXP-EXPORT-1",
            "descripcion": "Expediente para exportar",
            "unidad": "JUZGADO 1"
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{creado}");

    let (estado, _) = pedir(
        &t.app,
        "GET",
        "/api/v1/expedientes/exportar",
        Some(&tok_usuario),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    let respuesta = t
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/expedientes/exportar")
                .header(header::AUTHORIZATION, format!("Bearer {tok_admin}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let estado = respuesta.status();
    let disposicion = respuesta
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = respuesta
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(
        estado,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(disposicion.contains(".xlsx"), "{disposicion}");

    assert!(bytes.starts_with(b"PK"), "no es un archivo xlsx");
    let mut archivo = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).expect("xlsx válido");
    let mut contiene_expediente = false;
    for indice in 0..archivo.len() {
        let mut entrada = archivo.by_index(indice).unwrap();
        let es_xml = entrada
            .name()
            .map(|nombre| nombre.ends_with(".xml"))
            .unwrap_or(false);
        if !es_xml {
            continue;
        }
        let mut contenido = String::new();
        if entrada.read_to_string(&mut contenido).is_ok()
            && contenido.contains("EXP-EXPORT-1")
        {
            contiene_expediente = true;
            break;
        }
    }
    assert!(contiene_expediente, "el expediente no aparece en el Excel");

    t.cerrar().await;
}

#[tokio::test]
async fn almacen_edita_la_descripcion_y_se_sincroniza() {
    let t = setup().await;
    crear_usuario(&t.db, "almacen@test.mx", "ALMACEN").await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok_almacen, _) = iniciar_sesion(&t.app, "almacen@test.mx").await;
    let (tok_usuario, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, creado) = pedir(
        &t.app,
        "POST",
        "/api/v1/expedientes",
        Some(&tok_usuario),
        Some(json!({
            "numero": "EXP-EDIT-1",
            "descripcion": "Descripción original"
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{creado}");
    let id = creado["expediente_id"].as_str().unwrap();

    let (estado, _) = pedir(
        &t.app,
        "PUT",
        &format!("/api/v1/expedientes/{id}"),
        Some(&tok_usuario),
        Some(json!({ "descripcion": "No autorizado" })),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    let (estado, actualizado) = pedir(
        &t.app,
        "PUT",
        &format!("/api/v1/expedientes/{id}"),
        Some(&tok_almacen),
        Some(json!({ "descripcion": "Descripción corregida" })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{actualizado}");
    assert_eq!(actualizado["descripcion"], "Descripción corregida");
    assert_eq!(actualizado["version"], 2);

    let (descripcion, version): (String, i64) =
        sqlx::query_as("SELECT descripcion, version FROM expedientes WHERE id = $1")
            .bind(id.parse::<uuid::Uuid>().unwrap())
            .fetch_one(&t.db)
            .await
            .unwrap();
    assert_eq!(descripcion, "Descripción corregida");
    assert_eq!(version, 2);

    let payload: serde_json::Value = sqlx::query_scalar(
        "SELECT payload FROM sync_events
         WHERE entidad = 'expediente' AND entidad_id = $1
         ORDER BY secuencia DESC LIMIT 1",
    )
    .bind(id.parse::<uuid::Uuid>().unwrap())
    .fetch_one(&t.db)
    .await
    .unwrap();
    assert_eq!(payload["descripcion"], "Descripción corregida");
    assert_eq!(payload["version"], 2);

    t.cerrar().await;
}
