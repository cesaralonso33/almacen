use axum::{
    body::Body,
    extract::{FromRequest, FromRequestParts, Query, Request},
    http::request::Parts,
    Json,
};
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// Extractor JSON que convierte cualquier rechazo en el error uniforme de la API.
///
/// Se usa en lugar de `Json<T>` para que cuerpos inválidos, campos desconocidos y
/// tipos incorrectos devuelvan `{"error": {...}}` con 400 en vez del texto plano de axum.
pub struct ApiJson<T>(pub T);

impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request<Body>, estado: &S) -> Result<Self, Self::Rejection> {
        let Json(valor) = Json::<T>::from_request(req, estado)
            .await
            .map_err(ApiError::from)?;
        Ok(ApiJson(valor))
    }
}

/// Extractor de query string que devuelve el error uniforme de la API.
pub struct ApiQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, estado: &S) -> Result<Self, Self::Rejection> {
        let Query(valor) = Query::<T>::from_request_parts(parts, estado)
            .await
            .map_err(ApiError::from)?;
        Ok(ApiQuery(valor))
    }
}