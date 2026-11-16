use axum::{
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

/// Error uniforme de la API. Toda respuesta de error tiene la forma
/// `{"error": {"code", "message", "correlacion_id"}}`.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub codigo: &'static str,
    pub mensaje: String,
}

impl ApiError {
    pub fn nuevo(status: StatusCode, codigo: &'static str, mensaje: impl Into<String>) -> Self {
        Self { status, codigo, mensaje: mensaje.into() }
    }

    pub fn solicitud_invalida(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::BAD_REQUEST, "SOLICITUD_INVALIDA", mensaje)
    }

    pub fn no_autenticado(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::UNAUTHORIZED, "NO_AUTENTICADO", mensaje)
    }

    pub fn credenciales(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::UNAUTHORIZED, "CREDENCIALES_INVALIDAS", mensaje)
    }

    pub fn prohibido(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::FORBIDDEN, "SIN_PERMISO", mensaje)
    }

    pub fn no_encontrado(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::NOT_FOUND, "NO_ENCONTRADO", mensaje)
    }

    pub fn conflicto(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::CONFLICT, "CONFLICTO", mensaje)
    }

    pub fn inprocesable(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::UNPROCESSABLE_ENTITY, "DATOS_INVALIDOS", mensaje)
    }

    pub fn demasiadas_solicitudes(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::TOO_MANY_REQUESTS, "DEMASIADAS_SOLICITUDES", mensaje)
    }

    pub fn interno(mensaje: impl Into<String>) -> Self {
        Self::nuevo(StatusCode::INTERNAL_SERVER_ERROR, "ERROR_INTERNO", mensaje)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let correlacion = Uuid::new_v4();
        if self.status.is_server_error() {
            tracing::error!(codigo = %self.codigo, mensaje = %self.mensaje, %correlacion, "error de la API");
        }
        let cuerpo = json!({
            "error": {
                "code": self.codigo,
                "message": self.mensaje,
                "correlacion_id": correlacion,
            }
        });
        (self.status, Json(cuerpo)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(?error, "error de base de datos");
        ApiError::interno("Error de base de datos")
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rechazo: JsonRejection) -> Self {
        ApiError::solicitud_invalida(format!("Cuerpo JSON inválido: {}", rechazo.body_text()))
    }
}
