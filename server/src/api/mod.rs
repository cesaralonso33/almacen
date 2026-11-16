pub mod admin;
pub mod auth;
pub mod devoluciones;
pub mod entrada;
pub mod entregas;
pub mod pedidos;
pub mod transferencias;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

use crate::{error::ApiError, AppState};

/// Construye el enrutador completo de la API.
pub fn router(estado: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/password", post(auth::cambiar_password))
        .route("/api/v1/auth/me", get(auth::me))
        .route("/api/v1/admin/usuarios", get(admin::listar_usuarios).post(admin::crear_usuario))
        .route("/api/v1/admin/usuarios/{id}/activar", post(admin::activar))
        .route("/api/v1/admin/usuarios/{id}/desactivar", post(admin::desactivar))
        .route("/api/v1/admin/roles", get(admin::listar_roles))
        .route("/api/v1/pedidos", post(pedidos::crear))
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
        .route("/api/v1/devoluciones/{id}/confirmar", post(devoluciones::confirmar))
        .route("/api/v1/transferencias", post(transferencias::solicitar))
        .route("/api/v1/transferencias/{id}/aceptar", post(transferencias::aceptar))
        .route("/api/v1/transferencias/{id}/rechazar", post(transferencias::rechazar))
        .fallback(ruta_no_encontrada)
        .method_not_allowed_fallback(metodo_no_permitido)
        .with_state(estado)
}

async fn health(State(estado): State<AppState>) -> Result<Json<Value>, ApiError> {
    sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&estado.db).await?;
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
