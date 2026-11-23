pub mod admin;
pub mod auth;
pub mod consultas;
pub mod darfes;
pub mod devoluciones;
pub mod directorio;
pub mod entrada;
pub mod entregas;
pub mod expedientes;
pub mod pa;
pub mod pedidos;
pub mod reportes;
pub mod sync;
pub mod transferencias;

use std::time::Duration;

use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderName, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post, put},
    Json, Router,
};
use serde_json::{json, Value};
use tower_http::{
    catch_panic::CatchPanicLayer, limit::RequestBodyLimitLayer, services::ServeDir,
    timeout::TimeoutLayer,
};

use crate::{error::ApiError, AppState};

const LIMITE_CUERPO: usize = 12 * 1024 * 1024;

async fn cabeceras_seguridad(solicitud: Request, siguiente: Next) -> Response {
    let mut respuesta = siguiente.run(solicitud).await;
    let cabeceras = respuesta.headers_mut();
    cabeceras.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    cabeceras.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    cabeceras.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    cabeceras.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    cabeceras.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    cabeceras.insert(
        header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000"),
    );
    cabeceras.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );
    cabeceras.insert(
        HeaderName::from_static("x-permitted-cross-domain-policies"),
        HeaderValue::from_static("none"),
    );
    respuesta
}

/// Construye el enrutador completo de la API.
pub fn router(estado: AppState) -> Router {
    let directorio_actualizaciones =
        std::env::var("UPDATES_DIR").unwrap_or_else(|_| "/updates".to_string());

    Router::new()
        .nest_service("/updates", ServeDir::new(directorio_actualizaciones))
        .route("/health", get(health))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/password", post(auth::cambiar_password))
        .route("/api/v1/auth/me", get(auth::me))
        .route(
            "/api/v1/admin/usuarios",
            get(admin::listar_usuarios).post(admin::crear_usuario),
        )
        .route("/api/v1/admin/usuarios/{id}", post(admin::editar_usuario))
        .route("/api/v1/admin/usuarios/{id}/activar", post(admin::activar))
        .route(
            "/api/v1/admin/usuarios/{id}/desactivar",
            post(admin::desactivar),
        )
        .route("/api/v1/admin/roles", get(admin::listar_roles))
        .route("/api/v1/admin/backup", post(admin::respaldar_base))
        .route("/api/v1/darfes", get(darfes::listar).post(darfes::crear))
        .route("/api/v1/darfes/exportar", get(darfes::exportar))
        .route("/api/v1/darfes/reportes", get(darfes::reportes))
        .route(
            "/api/v1/darfes/{id}",
            get(darfes::detalle)
                .put(darfes::actualizar)
                .delete(darfes::eliminar),
        )
        .route(
            "/api/v1/darfes/{id}/documentos",
            post(darfes::subir_documento),
        )
        .route("/api/v1/darfes/{id}/formato", post(darfes::generar_formato))
        .route("/api/v1/darfes/{id}/folio", put(darfes::actualizar_folio))
        .route(
            "/api/v1/darfes/{id}/documentos/{campo}",
            get(darfes::documento).delete(darfes::eliminar_documento),
        )
        .route("/api/v1/pa", get(pa::listar).post(pa::crear))
        .route(
            "/api/v1/pa/{id}",
            get(pa::detalle).put(pa::actualizar).delete(pa::eliminar),
        )
        .route("/api/v1/pa/{id}/documento", get(pa::documento))
        .route(
            "/api/v1/pedidos",
            get(consultas::listar_pedidos).post(pedidos::crear),
        )
        .route("/api/v1/pedidos/asignar", post(pedidos::asignar))
        .route("/api/v1/dashboard", get(consultas::dashboard))
        .route("/api/v1/reportes/altas", get(reportes::altas))
        .route("/api/v1/usuarios/directorio", get(directorio::directorio))
        .route("/api/v1/custodias", get(consultas::custodias))
        .route(
            "/api/v1/expedientes",
            get(consultas::buscar_expedientes).post(expedientes::crear),
        )
        .route(
            "/api/v1/expedientes/importar",
            post(expedientes::importar),
        )
        .route(
            "/api/v1/expedientes/exportar",
            get(consultas::exportar_expedientes),
        )
        .route(
            "/api/v1/expedientes/{id}",
            put(expedientes::actualizar),
        )
        .route(
            "/api/v1/expedientes/{id}/historial",
            get(consultas::historial),
        )
        .route(
            "/api/v1/entregas/pendientes",
            get(consultas::entregas_pendientes),
        )
        .route("/api/v1/pedidos/{id}", get(pedidos::detalle))
        .route("/api/v1/pedidos/{id}/cancelar", post(pedidos::cancelar))
        .route("/api/v1/items/{id}/preparar", post(pedidos::preparar))
        .route("/api/v1/items/{id}/descartar", post(pedidos::descartar))
        .route("/api/v1/entregas", post(entregas::despachar))
        .route("/api/v1/entregas/{id}/confirmar", post(entregas::confirmar))
        .route(
            "/api/v1/devoluciones",
            get(devoluciones::listar).post(devoluciones::solicitar),
        )
        .route(
            "/api/v1/devoluciones/{id}/confirmar",
            post(devoluciones::confirmar),
        )
        .route("/api/v1/transferencias", post(transferencias::solicitar))
        .route(
            "/api/v1/transferencias/pendientes",
            get(transferencias::pendientes),
        )
        .route(
            "/api/v1/transferencias/{id}/aceptar",
            post(transferencias::aceptar),
        )
        .route(
            "/api/v1/transferencias/{id}/rechazar",
            post(transferencias::rechazar),
        )
        .route(
            "/api/v1/sync/dispositivos",
            post(sync::registrar_dispositivo),
        )
        .route("/api/v1/sync/push", post(sync::push))
        .route("/api/v1/sync/pull", get(sync::pull))
        .route("/api/v1/sync/snapshot", get(sync::snapshot))
        .fallback(ruta_no_encontrada)
        .method_not_allowed_fallback(metodo_no_permitido)
        .layer(DefaultBodyLimit::max(LIMITE_CUERPO))
        .layer(RequestBodyLimitLayer::new(LIMITE_CUERPO))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(60),
        ))
        .layer(CatchPanicLayer::new())
        .layer(middleware::from_fn(cabeceras_seguridad))
        .with_state(estado)
}

async fn health(State(estado): State<AppState>) -> Result<Json<Value>, ApiError> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&estado.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn ruta_no_encontrada() -> ApiError {
    ApiError::no_encontrado("Ruta no encontrada")
}

async fn metodo_no_permitido() -> ApiError {
    ApiError::nuevo(
        StatusCode::METHOD_NOT_ALLOWED,
        "METODO_NO_PERMITIDO",
        "Método no permitido para esta ruta",
    )
}
