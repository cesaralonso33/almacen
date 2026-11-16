use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{api::entrada::ApiJson, audit, auth::AuthUser, error::ApiError, AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NuevoUsuario {
    pub nombre: String,
    pub correo: String,
    pub password: String,
    pub roles: Vec<String>,
}

pub async fn listar_usuarios(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("admin:usuarios")?;

    let filas = sqlx::query_as::<_, (Uuid, String, Option<String>, bool, bool, Vec<String>)>(
        "SELECT u.id, u.nombre, u.correo, u.activo, u.debe_cambiar_password,
                COALESCE(array_agg(r.clave ORDER BY r.clave)
                         FILTER (WHERE r.clave IS NOT NULL), ARRAY[]::text[]) AS roles
         FROM usuarios u
         LEFT JOIN usuario_roles ur ON ur.usuario_id = u.id AND ur.revocado_en IS NULL
         LEFT JOIN roles r ON r.id = ur.rol_id
         GROUP BY u.id
         ORDER BY u.nombre",
    )
    .fetch_all(&app.db)
    .await?;

    let usuarios: Vec<Value> = filas
        .into_iter()
        .map(|(id, nombre, correo, activo, debe_cambiar, roles)| {
            json!({
                "id": id,
                "nombre": nombre,
                "correo": correo,
                "activo": activo,
                "debe_cambiar_password": debe_cambiar,
                "roles": roles,
            })
        })
        .collect();

    Ok(Json(json!({ "usuarios": usuarios })))
}

pub async fn crear_usuario(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<NuevoUsuario>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("admin:usuarios")?;

    let nombre = input.nombre.trim();
    let correo = input.correo.trim().to_lowercase();

    if nombre.is_empty() {
        return Err(ApiError::inprocesable("El nombre es obligatorio"));
    }
    if !correo.contains('@') || correo.len() < 5 {
        return Err(ApiError::inprocesable("El correo no es válido"));
    }
    if input.password.chars().count() < 12 {
        return Err(ApiError::inprocesable(
            "La contraseña debe tener al menos 12 caracteres",
        ));
    }
    if input.roles.is_empty() {
        return Err(ApiError::inprocesable("Debe asignar al menos un rol"));
    }

    let existe: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usuarios WHERE lower(correo) = $1)")
            .bind(&correo)
            .fetch_one(&app.db)
            .await?;
    if existe {
        return Err(ApiError::conflicto("Ya existe un usuario con ese correo"));
    }

    let hash = crate::auth::hash_password(&input.password).map_err(|error| {
        tracing::error!(?error, "no fue posible calcular el hash de la contraseña");
        ApiError::interno("No fue posible crear la contraseña")
    })?;

    let mut tx = app.db.begin().await?;

    let usuario_id: Uuid = sqlx::query_scalar(
        "INSERT INTO usuarios(nombre, correo, password_hash, debe_cambiar_password)
         VALUES ($1,$2,$3,true) RETURNING id",
    )
    .bind(nombre)
    .bind(&correo)
    .bind(hash)
    .fetch_one(&mut *tx)
    .await?;

    for clave in &input.roles {
        let asignados = sqlx::query(
            "INSERT INTO usuario_roles(usuario_id, rol_id)
             SELECT $1, id FROM roles WHERE clave = $2",
        )
        .bind(usuario_id)
        .bind(clave)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if asignados == 0 {
            tx.rollback().await?;
            return Err(ApiError::inprocesable(format!("El rol {clave} no existe")));
        }
    }

    audit::registrar(
        &mut tx,
        Some(user.id),
        "admin.usuarios.crear",
        "usuarios",
        usuario_id,
        "EXITO",
        json!({ "correo": correo, "roles": input.roles }),
    )
    .await?;

    tx.commit().await?;
    Ok(Json(json!({ "usuario_id": usuario_id, "debe_cambiar_password": true })))
}

pub async fn activar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("admin:usuarios")?;
    cambiar_estado(&app, &user, id, true).await
}

pub async fn desactivar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("admin:usuarios")?;
    if id == user.id {
        return Err(ApiError::conflicto("No puede desactivar su propia cuenta"));
    }
    cambiar_estado(&app, &user, id, false).await
}

async fn cambiar_estado(
    app: &AppState,
    user: &AuthUser,
    id: Uuid,
    activo: bool,
) -> Result<Json<Value>, ApiError> {
    let mut tx = app.db.begin().await?;

    let actualizado: Option<Uuid> = sqlx::query_scalar(
        "UPDATE usuarios SET activo = $2, actualizado_en = now() WHERE id = $1 RETURNING id",
    )
    .bind(id)
    .bind(activo)
    .fetch_optional(&mut *tx)
    .await?;

    if actualizado.is_none() {
        return Err(ApiError::no_encontrado("Usuario no encontrado"));
    }

    if !activo {
        // Un usuario desactivado no puede conservar sesiones abiertas.
        sqlx::query(
            "UPDATE sesiones SET revocada_en = now() WHERE usuario_id = $1 AND revocada_en IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }

    audit::registrar(
        &mut tx,
        Some(user.id),
        if activo { "admin.usuarios.activar" } else { "admin.usuarios.desactivar" },
        "usuarios",
        id,
        "EXITO",
        json!({ "activo": activo }),
    )
    .await?;

    tx.commit().await?;
    Ok(Json(json!({ "usuario_id": id, "activo": activo })))
}

pub async fn listar_roles(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("admin:roles")?;

    let filas = sqlx::query_as::<_, (String, String, Vec<String>)>(
        "SELECT r.clave, r.nombre,
                COALESCE(array_agg(p.clave ORDER BY p.clave)
                         FILTER (WHERE p.clave IS NOT NULL), ARRAY[]::text[]) AS permisos
         FROM roles r
         LEFT JOIN rol_permisos rp ON rp.rol_id = r.id
         LEFT JOIN permisos p ON p.id = rp.permiso_id
         GROUP BY r.id
         ORDER BY r.clave",
    )
    .fetch_all(&app.db)
    .await?;

    let roles: Vec<Value> = filas
        .into_iter()
        .map(|(clave, nombre, permisos)| json!({ "clave": clave, "nombre": nombre, "permisos": permisos }))
        .collect();

    Ok(Json(json!({ "roles": roles })))
}
