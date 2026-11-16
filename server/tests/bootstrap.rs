//! Aprovisionamiento del administrador inicial sin datos de demostración.

mod common;

use std::process::Command;

use common::*;

#[tokio::test]
async fn crear_admin_funciona_y_rechaza_correos_duplicados() {
    let t = setup().await;
    let ejecutable = env!("CARGO_BIN_EXE_crear_admin");

    let primera = Command::new(ejecutable)
        .env("DATABASE_URL", &t.url)
        .env("ADMIN_CORREO", "jefe@institucion.mx")
        .env("ADMIN_PASSWORD", "Clave#Segura-2026")
        .env("ADMIN_NOMBRE", "Dirección jurídica")
        .output()
        .expect("ejecutar crear_admin");

    assert!(
        primera.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&primera.stderr)
    );

    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT r.clave
         FROM usuario_roles ur
         JOIN usuarios u ON u.id = ur.usuario_id
         JOIN roles r ON r.id = ur.rol_id
         WHERE lower(u.correo) = 'jefe@institucion.mx'",
    )
    .fetch_all(&t.db)
    .await
    .unwrap();

    assert_eq!(roles, vec!["ADMINISTRADOR".to_string()]);

    let segunda = Command::new(ejecutable)
        .env("DATABASE_URL", &t.url)
        .env("ADMIN_CORREO", "jefe@institucion.mx")
        .env("ADMIN_PASSWORD", "Otra#Clave-2026")
        .output()
        .expect("segunda ejecución");

    assert!(!segunda.status.success(), "no debe duplicar el administrador");

    t.cerrar().await;
}

#[tokio::test]
async fn crear_admin_rechaza_contrasenas_cortas() {
    let t = setup().await;
    let ejecutable = env!("CARGO_BIN_EXE_crear_admin");

    let salida = Command::new(ejecutable)
        .env("DATABASE_URL", &t.url)
        .env("ADMIN_CORREO", "jefe@institucion.mx")
        .env("ADMIN_PASSWORD", "corta")
        .output()
        .expect("ejecutar crear_admin");

    assert!(!salida.status.success());

    let usuarios: i64 = sqlx::query_scalar("SELECT count(*) FROM usuarios")
        .fetch_one(&t.db)
        .await
        .unwrap();
    assert_eq!(usuarios, 0, "no debe crear el usuario");

    t.cerrar().await;
}
