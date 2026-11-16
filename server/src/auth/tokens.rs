use chrono::{DateTime, Duration as ChronoDuration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub sid: Uuid,
    pub iat: i64,
    pub exp: i64,
}

/// Token de refresco opaco: 32 bytes aleatorios en hexadecimal.
pub fn nuevo_refresh() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Solo se almacena el hash SHA-256 del token de refresco.
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn emitir_access(
    secreto: &str,
    usuario: Uuid,
    sesion: Uuid,
    ttl: std::time::Duration,
) -> Result<(String, DateTime<Utc>), ApiError> {
    let ahora = Utc::now();
    let ttl = ChronoDuration::from_std(ttl).unwrap_or_else(|_| ChronoDuration::minutes(15));
    let expira = ahora + ttl;
    let claims = Claims {
        sub: usuario,
        sid: sesion,
        iat: ahora.timestamp(),
        exp: expira.timestamp(),
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secreto.as_bytes()),
    )
    .map_err(|error| {
        tracing::error!(?error, "no fue posible firmar el token de acceso");
        ApiError::interno("No fue posible emitir el token de acceso")
    })?;
    Ok((token, expira))
}

pub fn validar_access(secreto: &str, token: &str) -> Result<Claims, ApiError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secreto.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|datos| datos.claims)
    .map_err(|_| ApiError::no_autenticado("Token inválido o expirado"))
}

