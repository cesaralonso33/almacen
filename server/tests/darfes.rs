//! Módulo DARFES: CRUD, documentos, exportación y reportes.

mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::json;
use std::io::Read;
use tower::ServiceExt;

use common::*;

fn texto_docx(bytes: &[u8]) -> String {
    let mut archivo = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("DOCX válido");
    let mut xml = String::new();
    archivo
        .by_name("word/document.xml")
        .expect("document.xml")
        .read_to_string(&mut xml)
        .expect("leer document.xml");
    xml
}

fn darfe(numero: &str, nombre: &str) -> serde_json::Value {
    json!({
        "numero_darfe": numero,
        "fecha": "2026-11-18",
        "nombre": nombre,
        "expediente": "EXP-DARFE-1",
        "registro": "REG-1",
        "banco": "Banco Institucional",
        "curp": "ABCD800101HDFXYZ01",
        "rfc": "ABCD800101XYZ",
        "telefono": "555-0101",
        "correo": "persona@ejemplo.mx",
        "descripcion": "Pago de prueba",
        "monto": 1500.50,
        "numero_cuenta": "1234567890",
        "tipo": "REEMBOLSO",
        "estatus": "PENDIENTE",
        "campos_extra": [{ "nombre": "Teléfono", "valor": "555-0101" }]
    })
}

async fn descargar(t: &TestApp, token: &str, ruta: &str) -> (StatusCode, Vec<u8>) {
    let respuesta = t
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(ruta)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let estado = respuesta.status();
    let bytes = respuesta
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (estado, bytes)
}

async fn descargar_encabezados(
    t: &TestApp,
    token: &str,
    metodo: &str,
    ruta: &str,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let respuesta = t
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method(metodo)
                .uri(ruta)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let estado = respuesta.status();
    let encabezados = respuesta.headers().clone();
    let bytes = respuesta
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (estado, encabezados, bytes)
}

#[tokio::test]
async fn administrativo_crea_documenta_exporta_y_consulta_reportes() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRATIVO").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    let (estado, creado) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok),
        Some(darfe("DARFE-001", "Persona Uno")),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{creado}");
    let id = creado["id"].as_str().unwrap();
    assert_eq!(creado["estatus"], "PENDIENTE");
    assert_eq!(creado["telefono"], "555-0101");
    assert_eq!(creado["correo"], "persona@ejemplo.mx");
    assert_eq!(creado["campos_extra"][0]["nombre"], "Teléfono");

    let (estado, por_correo) = pedir(
        &t.app,
        "GET",
        "/api/v1/darfes?buscar=persona@ejemplo.mx",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{por_correo}");
    assert_eq!(por_correo["total"], 1);

    let (estado, listado) = pedir(
        &t.app,
        "GET",
        "/api/v1/darfes?buscar=Persona&estatus=PENDIENTE",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{listado}");
    assert_eq!(listado["total"], 1);
    assert_eq!(listado["darfes"][0]["numero_darfe"], "DARFE-001");

    let (estado, documento) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/darfes/{id}/documentos"),
        Some(&tok),
        Some(json!({
            "campo": "CURP",
            "nombre": "curp.pdf",
            "mime": "application/pdf",
            "contenido_base64": "JVBERi0="
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{documento}");

    let (estado, deposito) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/darfes/{id}/documentos"),
        Some(&tok),
        Some(json!({
            "campo": "DEPOSITO",
            "nombre": "deposito.pdf",
            "mime": "application/pdf",
            "contenido_base64": "JVBERi0="
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{deposito}");
    assert_eq!(deposito["coincide"], false);

    let (estado, escrito) = pedir(
        &t.app,
        "POST",
        &format!("/api/v1/darfes/{id}/documentos"),
        Some(&tok),
        Some(json!({
            "campo": "ESCRITO_LIBRE",
            "nombre": "escrito.pdf",
            "mime": "application/pdf",
            "contenido_base64": "JVBERi0="
        })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{escrito}");

    let (estado, pdf) = descargar(&t, &tok, &format!("/api/v1/darfes/{id}/documentos/CURP")).await;
    assert_eq!(estado, StatusCode::OK);
    assert!(pdf.starts_with(b"%PDF"));

    let (estado, xlsx) = descargar(&t, &tok, "/api/v1/darfes/exportar?estatus=PENDIENTE").await;
    assert_eq!(estado, StatusCode::OK);
    assert!(xlsx.starts_with(b"PK"));

    let (estado, reportes) =
        pedir(&t.app, "GET", "/api/v1/darfes/reportes", Some(&tok), None).await;
    assert_eq!(estado, StatusCode::OK, "{reportes}");
    assert_eq!(reportes["total"], 1);
    assert_eq!(reportes["por_estatus"]["PENDIENTE"], 1);
    assert_eq!(reportes["documentos"]["CURP"], 1);
    assert_eq!(reportes["documentos"]["ESCRITO_LIBRE"], 1);

    let (estado, actualizado) = pedir(
        &t.app,
        "PUT",
        &format!("/api/v1/darfes/{id}"),
        Some(&tok),
        Some({
            let mut cuerpo = darfe("DARFE-001", "Persona Actualizada");
            cuerpo["estatus"] = json!("COMPLETO");
            cuerpo
        }),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{actualizado}");
    assert_eq!(actualizado["estatus"], "COMPLETO");

    let (estado, _) = pedir(
        &t.app,
        "DELETE",
        &format!("/api/v1/darfes/{id}"),
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (_, despues) = pedir(&t.app, "GET", "/api/v1/darfes", Some(&tok), None).await;
    assert_eq!(despues["total"], 0);

    t.cerrar().await;
}

#[tokio::test]
async fn rechaza_duplicados_y_usuarios_sin_permiso() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRATIVO").await;
    crear_usuario(&t.db, "usuario@test.mx", "USUARIO").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok),
        Some(darfe("DARFE-002", "Persona Dos")),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);

    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok),
        Some(darfe("DARFE-002", "Duplicada")),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT);

    let (tok_usuario, _) = iniciar_sesion(&t.app, "usuario@test.mx").await;
    let (estado, _) = pedir(&t.app, "GET", "/api/v1/darfes", Some(&tok_usuario), None).await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    t.cerrar().await;
}

#[tokio::test]
async fn canasta_crea_tres_items_mensuales() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRATIVO").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;
    let mut cuerpo = darfe("DARFE-CANASTA-1", "Persona Canasta");
    cuerpo["fecha"] = json!("2026-01-15");
    cuerpo["tipo"] = json!("CANASTA");
    cuerpo["descripcion"] = json!("Se reemplaza por la descripción automática");

    let (estado, respuesta) =
        pedir(&t.app, "POST", "/api/v1/darfes", Some(&tok), Some(cuerpo)).await;
    assert_eq!(estado, StatusCode::OK, "{respuesta}");
    assert_eq!(respuesta["total"], 3);
    let items = respuesta["darfes"].as_array().unwrap();
    assert_eq!(items[0]["fecha"], "2026-01-15");
    assert_eq!(items[1]["fecha"], "2026-02-15");
    assert_eq!(items[2]["fecha"], "2026-03-15");
    assert_eq!(items[0]["descripcion"], "1ra canasta 3");
    assert_eq!(items[1]["descripcion"], "2da canasta de 3");
    assert_eq!(items[2]["descripcion"], "3ra canasta de 3");
    assert_eq!(items[0]["numero_darfe"], "DARFE-CANASTA-1");
    assert_eq!(items[1]["numero_darfe"], "DARFE-CANASTA-1");

    let (_, listado) = pedir(
        &t.app,
        "GET",
        "/api/v1/darfes?buscar=DARFE-CANASTA-1",
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(listado["total"], 3);

    t.cerrar().await;
}

#[tokio::test]
async fn genera_recibo_docx_con_folio_consecutivo() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRATIVO").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    // Un contador del año anterior no afecta el primer folio del año en curso.
    sqlx::query("INSERT INTO darfe_folios (anio, ultimo) VALUES (2025, 600)")
        .execute(&t.db)
        .await
        .unwrap();

    let mut ids = Vec::new();
    for (indice, numero) in ["DARFE-F-1", "DARFE-F-2", "DARFE-F-3"].iter().enumerate() {
        let (estado, creado) = pedir(
            &t.app,
            "POST",
            "/api/v1/darfes",
            Some(&tok),
            Some(darfe(numero, &format!("Persona Recibo {indice}"))),
        )
        .await;
        assert_eq!(estado, StatusCode::OK, "{creado}");
        ids.push(creado["id"].as_str().unwrap().to_string());
    }

    let mut folios = Vec::new();
    for id in &ids {
        let (estado, encabezados, docx) = descargar_encabezados(
            &t,
            &tok,
            "POST",
            &format!("/api/v1/darfes/{id}/formato"),
        )
        .await;
        assert_eq!(estado, StatusCode::OK);
        assert!(docx.starts_with(b"PK"));
        if folios.is_empty() {
            let texto = texto_docx(&docx);
            assert!(!texto.contains("{{"), "{texto}");
            assert!(texto.contains("557"), "{texto}");
            assert!(texto.contains("DARFE-F-1"), "{texto}");
            assert!(texto.contains("1,500.50"), "{texto}");
        }
        let disposicion = encabezados
            .get(header::CONTENT_DISPOSITION)
            .and_then(|valor| valor.to_str().ok())
            .unwrap_or_default();
        let folio: i32 = disposicion
            .split("Recibo-")
            .nth(1)
            .and_then(|resto| resto.split('.').next())
            .and_then(|valor| valor.parse().ok())
            .unwrap_or_default();
        folios.push(folio);
    }
    assert_eq!(folios, vec![557, 558, 559]);

    // Una reimpresión conserva el folio asignado.
    let (estado, encabezados, _) = descargar_encabezados(
        &t,
        &tok,
        "POST",
        &format!("/api/v1/darfes/{}/formato", ids[0]),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);
    let disposicion = encabezados
        .get(header::CONTENT_DISPOSITION)
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or_default();
    assert!(disposicion.contains("Recibo-557.docx"), "{disposicion}");

    t.cerrar().await;
}

#[tokio::test]
async fn genera_recibo_con_tipo_de_apoyo() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRATIVO").await;
    let (tok, _) = iniciar_sesion(&t.app, "admin@test.mx").await;

    let mut cuerpo = darfe("DARFE-MED-1", "Persona Medicamento");
    cuerpo["tipo"] = json!("MEDICAMENTO");
    cuerpo["descripcion"] = json!("Compra de medicamentos");
    let (estado, creado) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok),
        Some(cuerpo.clone()),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{creado}");
    assert_eq!(creado["tipo"], "MEDICAMENTO");
    let id = creado["id"].as_str().unwrap();

    let (estado, _, docx) = descargar_encabezados(
        &t,
        &tok,
        "POST",
        &format!("/api/v1/darfes/{id}/formato"),
    )
    .await;
    assert_eq!(estado, StatusCode::OK);
    assert!(docx.starts_with(b"PK"));
    let texto = texto_docx(&docx);
    assert!(!texto.contains("{{"), "{texto}");
    assert!(texto.contains("MEDICAMENTO"), "{texto}");
    assert!(texto.contains("OTRAS AYUDAS"), "{texto}");
    assert!(texto.contains("DARFE-MED-1"), "{texto}");

    // El número sigue siendo único para los tipos que no son canasta.
    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok),
        Some(cuerpo),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT);

    // Un tipo fuera del catálogo se rechaza.
    let (estado, _) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok),
        Some({
            let mut invalido = darfe("DARFE-MAL-1", "Persona Inválida");
            invalido["tipo"] = json!("NO EXISTE");
            invalido
        }),
    )
    .await;
    assert_eq!(estado, StatusCode::UNPROCESSABLE_ENTITY);

    t.cerrar().await;
}

#[tokio::test]
async fn solo_administrador_cambia_el_folio() {
    let t = setup().await;
    crear_usuario(&t.db, "admin@test.mx", "ADMINISTRADOR").await;
    crear_usuario(&t.db, "aux@test.mx", "ADMINISTRATIVO").await;
    let (tok_admin, _) = iniciar_sesion(&t.app, "admin@test.mx").await;
    let (tok_aux, _) = iniciar_sesion(&t.app, "aux@test.mx").await;

    let (estado, creado) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok_admin),
        Some(darfe("DARFE-FOLIO-1", "Persona Folio")),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{creado}");
    let id = creado["id"].as_str().unwrap();

    // El perfil administrativo no puede cambiar el folio.
    let (estado, _) = pedir(
        &t.app,
        "PUT",
        &format!("/api/v1/darfes/{id}/folio"),
        Some(&tok_aux),
        Some(json!({ "folio": 700 })),
    )
    .await;
    assert_eq!(estado, StatusCode::FORBIDDEN);

    // El administrador sí puede y el cambio queda reflejado.
    let (estado, actualizado) = pedir(
        &t.app,
        "PUT",
        &format!("/api/v1/darfes/{id}/folio"),
        Some(&tok_admin),
        Some(json!({ "folio": 700 })),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{actualizado}");
    assert_eq!(actualizado["folio_formato"], 700);

    // El folio no puede repetirse dentro del mismo año.
    let (estado, otro) = pedir(
        &t.app,
        "POST",
        "/api/v1/darfes",
        Some(&tok_admin),
        Some(darfe("DARFE-FOLIO-2", "Otra Persona")),
    )
    .await;
    assert_eq!(estado, StatusCode::OK, "{otro}");
    let otro_id = otro["id"].as_str().unwrap();
    let (estado, _) = pedir(
        &t.app,
        "PUT",
        &format!("/api/v1/darfes/{otro_id}/folio"),
        Some(&tok_admin),
        Some(json!({ "folio": 700 })),
    )
    .await;
    assert_eq!(estado, StatusCode::CONFLICT);

    t.cerrar().await;
}
