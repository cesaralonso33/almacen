//! Duración de la sesión y cierre por inactividad.

mod common;

use axum::http::StatusCode;

use common::*;

#[tokio::test]
async fn la_sesion_dura_veinticuatro_horas() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (_access, _refresh) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let horas: f64 = sqlx::query_scalar(
        "SELECT (extract(epoch FROM (expira_en - now())) / 3600.0)::float8
         FROM sesiones ORDER BY creada_en DESC LIMIT 1",
    )
    .fetch_one(&t.db)
    .await
    .unwrap();

    assert!(
        (horas - 24.0).abs() < 0.5,
        "la sesión debe durar 24 horas, dura {horas}"
    );

    t.cerrar().await;
}

#[tokio::test]
async fn la_inactividad_cierra_y_revoca_la_sesion() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (access, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/auth/me", Some(&access), None).await;
    assert_eq!(estado, StatusCode::OK);

    sqlx::query("UPDATE sesiones SET ultima_actividad_en = now() - interval '19 hours'")
        .execute(&t.db)
        .await
        .unwrap();

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/auth/me", Some(&access), None).await;
    assert_eq!(estado, StatusCode::UNAUTHORIZED, "debe cerrarse por inactividad");

    let revocadas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sesiones WHERE revocada_en IS NOT NULL")
            .fetch_one(&t.db)
            .await
            .unwrap();
    assert_eq!(revocadas, 1);

    t.cerrar().await;
}

#[tokio::test]
async fn la_actividad_reciente_mantiene_la_sesion() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (access, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;

    sqlx::query("UPDATE sesiones SET ultima_actividad_en = now() - interval '17 hours'")
        .execute(&t.db)
        .await
        .unwrap();

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/auth/me", Some(&access), None).await;
    assert_eq!(estado, StatusCode::OK);

    let reciente: bool = sqlx::query_scalar(
        "SELECT ultima_actividad_en > now() - interval '1 minute' FROM sesiones LIMIT 1",
    )
    .fetch_one(&t.db)
    .await
    .unwrap();
    assert!(reciente, "la petición debe actualizar la última actividad");

    t.cerrar().await;
}

#[tokio::test]
async fn la_sincronizacion_no_cuenta_como_actividad() {
    let t = setup().await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (access, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;
    let dispositivo = registrar_dispositivo(&t.app, &access, "Equipo de pruebas").await;

    sqlx::query("UPDATE sesiones SET ultima_actividad_en = now() - interval '19 hours'")
        .execute(&t.db)
        .await
        .unwrap();

    let (estado, cuerpo) = pedir(
        &t.app,
        "GET",
        &format!("/api/v1/sync/pull?dispositivo_id={dispositivo}&cursor=0"),
        Some(&access),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{cuerpo}");

    let (estado, _) = pedir(&t.app, "GET", "/api/v1/auth/me", Some(&access), None).await;
    assert_eq!(
        estado,
        StatusCode::UNAUTHORIZED,
        "el pull no debe renovar la actividad del usuario"
    );

    t.cerrar().await;
}
