use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::entrada::ApiJson;
use crate::{
    audit,
    auth::{password::verificar_password, tokens, AuthUser},
    error::ApiError,
    AppState,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginInput {
    pub correo: String,
    pub password: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshInput {
    pub refresh_token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CambioPasswordInput {
    pub actual: String,
    pub nueva: String,
}

pub async fn login(
    State(app): State<AppState>,
    ApiJson(input): ApiJson<LoginInput>,
) -> Result<Json<Value>, ApiError> {
    let correo = input.correo.trim().to_lowercase();
    if correo.is_empty() || input.password.is_empty() {
        return Err(ApiError::solicitud_invalida("Correo y contraseña son obligatorios"));
    }
    if correo.len() > 254 || input.password.len() > 512 {
        return Err(ApiError::solicitud_invalida(
            "Correo o contraseña exceden la longitud permitida",
        ));
    }

    if !app.limitador.permitir(&correo) {
        return Err(ApiError::demasiadas_solicitudes(
            "Demasiados intentos fallidos; espere unos minutos antes de reintentar",
        ));
    }

    let fila = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>, bool, bool)>(
        "SELECT id, nombre, correo, password_hash, activo, debe_cambiar_password
         FROM usuarios WHERE lower(correo) = $1",
    )
    .bind(&correo)
    .fetch_optional(&app.db)
    .await?;

    let Some((usuario_id, nombre, correo_bd, hash, activo, debe_cambiar)) = fila else {
        app.limitador.registrar_fallo(&correo);
        return Err(ApiError::credenciales("Credenciales inválidas"));
    };

    let valido = hash
        .as_deref()
        .map(|almacenado| verificar_password(almacenado, &input.password))
        .unwrap_or(false);

    if !valido || !activo {
        app.limitador.registrar_fallo(&correo);
        return Err(ApiError::credenciales("Credenciales inválidas"));
    }

    app.limitador.limpiar(&correo);

    let (access, refresh, expira_access, expira_refresh, sesion_id) =
        abrir_sesion(&app.db, &app.config, usuario_id).await?;

    let mut tx = app.db.begin().await?;
    audit::registrar(
        &mut tx,
        Some(usuario_id),
        "auth.login",
        "sesiones",
        sesion_id,
        "EXITO",
        json!({ "correo": &correo_bd }),
    )
    .await?;
    tx.commit().await?;

    let (roles, permisos) = roles_y_permisos(&app.db, usuario_id).await?;

    Ok(Json(json!({
        "access_token": access,
        "access_expira_en": expira_access,
        "refresh_token": refresh,
        "refresh_expira_en": expira_refresh,
        "usuario": { "id": usuario_id, "nombre": nombre, "correo": correo_bd },
        "debe_cambiar_password": debe_cambiar,
        "roles": roles,
        "permisos": permisos,
    })))
}

pub async fn cambiar_password(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<CambioPasswordInput>,
) -> Result<Json<Value>, ApiError> {
    if input.nueva.chars().count() < 12 {
        return Err(ApiError::inprocesable(
            "La nueva contraseña debe tener al menos 12 caracteres",
        ));
    }
    if input.actual.len() > 512 || input.nueva.len() > 512 {
        return Err(ApiError::inprocesable(
            "La contraseña no puede exceder 512 caracteres",
        ));
    }
    if input.nueva == input.actual {
        return Err(ApiError::inprocesable(
            "La nueva contraseña debe ser distinta de la actual",
        ));
    }

    let hash_actual: Option<String> =
        sqlx::query_scalar("SELECT password_hash FROM usuarios WHERE id = $1")
            .bind(user.id)
            .fetch_optional(&app.db)
            .await?;

    let hash = hash_actual.ok_or_else(|| ApiError::no_encontrado("Usuario no encontrado"))?;

    if !verificar_password(&hash, &input.actual) {
        return Err(ApiError::credenciales("La contraseña actual no es correcta"));
    }

    let nuevo_hash = crate::auth::hash_password(&input.nueva).map_err(|error| {
        tracing::error!(?error, "no fue posible calcular el hash de la contraseña");
        ApiError::interno("No fue posible actualizar la contraseña")
    })?;

    let mut tx = app.db.begin().await?;
    sqlx::query(
        "UPDATE usuarios
         SET password_hash = $2, debe_cambiar_password = false, actualizado_en = now()
         WHERE id = $1",
    )
    .bind(user.id)
    .bind(nuevo_hash)
    .execute(&mut *tx)
    .await?;

    // Un cambio de contraseña invalida todas las sesiones, incluida la actual.
    sqlx::query("UPDATE sesiones SET revocada_en = now() WHERE usuario_id = $1 AND revocada_en IS NULL")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "auth.cambiar_password",
        "usuarios",
        user.id,
        "EXITO",
        json!({}),
    )
    .await?;

    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn refresh(
    State(app): State<AppState>,
    ApiJson(input): ApiJson<RefreshInput>,
) -> Result<Json<Value>, ApiError> {
    if input.refresh_token.trim().is_empty() {
        return Err(ApiError::solicitud_invalida("refresh_token es obligatorio"));
    }
    let hash = tokens::hash_token(input.refresh_token.trim());

    let fila = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>)>(
        "SELECT s.id, s.usuario_id, u.nombre, u.correo
         FROM sesiones s
         JOIN usuarios u ON u.id = s.usuario_id
         WHERE s.refresh_token_hash = $1
           AND s.revocada_en IS NULL
           AND s.expira_en > now()
           AND u.activo",
    )
    .bind(&hash)
    .fetch_optional(&app.db)
    .await?;

    let (sesion_anterior, usuario_id, nombre, correo) =
        fila.ok_or_else(|| ApiError::no_autenticado("Sesión no válida o expirada"))?;

    let nuevo_refresh = tokens::nuevo_refresh();
    let nuevo_hash = tokens::hash_token(&nuevo_refresh);
    let expira = expira_refresh(&app.config);

    let mut tx = app.db.begin().await?;
    let sesion_nueva: Uuid = sqlx::query_scalar(
        "INSERT INTO sesiones(usuario_id, refresh_token_hash, expira_en)
         VALUES ($1,$2,$3) RETURNING id",
    )
    .bind(usuario_id)
    .bind(&nuevo_hash)
    .bind(expira)
    .fetch_one(&mut *tx)
    .await?;

    let afectadas = sqlx::query(
        "UPDATE sesiones SET revocada_en = now(), reemplazada_por = $2
         WHERE id = $1 AND revocada_en IS NULL",
    )
    .bind(sesion_anterior)
    .bind(sesion_nueva)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if afectadas == 0 {
        tx.rollback().await?;
        return Err(ApiError::no_autenticado("Sesión no válida o expirada"));
    }

    audit::registrar(
        &mut tx,
        Some(usuario_id),
        "auth.refresh",
        "sesiones",
        sesion_nueva,
        "EXITO",
        json!({ "reemplaza": sesion_anterior }),
    )
    .await?;
    tx.commit().await?;

    let (access, expira_access) = tokens::emitir_access(
        &app.config.jwt_secret,
        usuario_id,
        sesion_nueva,
        app.config.access_ttl,
    )?;

    Ok(Json(json!({
        "access_token": access,
        "access_expira_en": expira_access,
        "refresh_token": nuevo_refresh,
        "refresh_expira_en": expira,
        "usuario": { "id": usuario_id, "nombre": nombre, "correo": correo },
    })))
}

pub async fn logout(State(app): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    let mut tx = app.db.begin().await?;
    sqlx::query("UPDATE sesiones SET revocada_en = now() WHERE id = $1 AND revocada_en IS NULL")
        .bind(user.sesion_id)
        .execute(&mut *tx)
        .await?;
    audit::registrar(
        &mut tx,
        Some(user.id),
        "auth.logout",
        "sesiones",
        user.sesion_id,
        "EXITO",
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn me(user: AuthUser) -> Result<Json<Value>, ApiError> {
    let roles = user.roles.clone();
    let permisos: Vec<String> = {
        let mut lista: Vec<String> = user.permisos.iter().cloned().collect();
        lista.sort();
        lista
    };
    Ok(Json(json!({
        "usuario": { "id": user.id, "nombre": user.nombre, "correo": user.correo },
        "roles": roles,
        "permisos": permisos,
        "sesion_id": user.sesion_id,
    })))
}

async fn abrir_sesion(
    db: &PgPool,
    config: &crate::config::Config,
    usuario_id: Uuid,
) -> Result<(String, String, DateTime<Utc>, DateTime<Utc>, Uuid), ApiError> {
    let refresh = tokens::nuevo_refresh();
    let hash = tokens::hash_token(&refresh);
    let expira = expira_refresh(config);

    let sesion_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sesiones(usuario_id, refresh_token_hash, expira_en)
         VALUES ($1,$2,$3) RETURNING id",
    )
    .bind(usuario_id)
    .bind(&hash)
    .bind(expira)
    .fetch_one(db)
    .await?;

    let (access, expira_access) = tokens::emitir_access(
        &config.jwt_secret,
        usuario_id,
        sesion_id,
        config.access_ttl,
    )?;

    Ok((access, refresh, expira_access, expira, sesion_id))
}

fn expira_refresh(config: &crate::config::Config) -> DateTime<Utc> {
    Utc::now()
        + chrono::Duration::from_std(config.refresh_ttl)
            .unwrap_or_else(|_| chrono::Duration::hours(1))
}

async fn roles_y_permisos(
    db: &PgPool,
    usuario_id: Uuid,
) -> Result<(Vec<String>, Vec<String>), ApiError> {
    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT r.clave FROM usuario_roles ur JOIN roles r ON r.id = ur.rol_id
         WHERE ur.usuario_id = $1 AND ur.revocado_en IS NULL ORDER BY r.clave",
    )
    .bind(usuario_id)
    .fetch_all(db)
    .await?;

    let permisos: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.clave
         FROM usuario_roles ur
         JOIN rol_permisos rp ON rp.rol_id = ur.rol_id
         JOIN permisos p ON p.id = rp.permiso_id
         WHERE ur.usuario_id = $1 AND ur.revocado_en IS NULL
         ORDER BY p.clave",
    )
    .bind(usuario_id)
    .fetch_all(db)
    .await?;

    Ok((roles, permisos))
}




