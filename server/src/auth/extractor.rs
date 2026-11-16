use std::collections::HashSet;

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{auth::tokens, error::ApiError, AppState};

/// Identidad autenticada derivada exclusivamente del token de acceso y de la base.
///
/// Ningún dato de identidad se acepta desde el cuerpo o la ruta de la petición.
pub struct AuthUser {
    pub id: Uuid,
    pub sesion_id: Uuid,
    pub nombre: String,
    pub correo: Option<String>,
    pub roles: Vec<String>,
    pub permisos: HashSet<String>,
}

impl AuthUser {
    pub fn puede(&self, permiso: &str) -> bool {
        self.permisos.contains(permiso)
    }

    pub fn exigir(&self, permiso: &str) -> Result<(), ApiError> {
        if self.puede(permiso) {
            Ok(())
        } else {
            Err(ApiError::prohibido(format!(
                "La operación requiere el permiso {permiso}"
            )))
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        estado: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|valor| valor.to_str().ok())
            .and_then(|valor| valor.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| ApiError::no_autenticado("Se requiere una sesión autenticada"))?;

        let claims = tokens::validar_access(&estado.config.jwt_secret, token)?;

        let fila = sqlx::query_as::<_, (Uuid, String, Option<String>, bool, bool, DateTime<Utc>, Option<Uuid>)>(
            "SELECT u.id, u.nombre, u.correo, u.activo,
                    (s.revocada_en IS NOT NULL), s.expira_en, s.dispositivo_id
             FROM sesiones s
             JOIN usuarios u ON u.id = s.usuario_id
             WHERE s.id = $1 AND u.id = $2",
        )
        .bind(claims.sid)
        .bind(claims.sub)
        .fetch_optional(&estado.db)
        .await?;

        let (_id, nombre, correo, activo, revocada, expira_en, _dispositivo) = fila
            .ok_or_else(|| ApiError::no_autenticado("Sesión no válida o expirada"))?;

        if revocada || !activo || expira_en <= Utc::now() {
            return Err(ApiError::no_autenticado("Sesión no válida o expirada"));
        }

        let permisos: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT p.clave
             FROM usuario_roles ur
             JOIN rol_permisos rp ON rp.rol_id = ur.rol_id
             JOIN permisos p ON p.id = rp.permiso_id
             WHERE ur.usuario_id = $1 AND ur.revocado_en IS NULL",
        )
        .bind(claims.sub)
        .fetch_all(&estado.db)
        .await?;

        let roles: Vec<String> = sqlx::query_scalar(
            "SELECT r.clave
             FROM usuario_roles ur
             JOIN roles r ON r.id = ur.rol_id
             WHERE ur.usuario_id = $1 AND ur.revocado_en IS NULL
             ORDER BY r.clave",
        )
        .bind(claims.sub)
        .fetch_all(&estado.db)
        .await?;

        Ok(AuthUser {
            id: claims.sub,
            sesion_id: claims.sid,
            nombre,
            correo,
            roles,
            permisos: permisos.into_iter().collect(),
        })
    }
}
