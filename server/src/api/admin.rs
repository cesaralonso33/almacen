use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;
use url::Url;
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditarUsuario {
    pub nombre: Option<String>,
    pub correo: Option<String>,
    pub password: Option<String>,
}

pub async fn editar_usuario(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<EditarUsuario>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("admin:usuarios")?;

    let nombre = match input.nombre.as_deref() {
        Some(valor) => {
            let limpio = valor.trim();
            if limpio.is_empty() {
                return Err(ApiError::inprocesable("El nombre no puede estar vacío"));
            }
            Some(limpio.to_string())
        }
        None => None,
    };

    let correo = match input.correo.as_deref() {
        Some(valor) => {
            let limpio = valor.trim().to_lowercase();
            if !limpio.contains('@') || limpio.len() < 5 {
                return Err(ApiError::inprocesable("El correo no es válido"));
            }
            Some(limpio)
        }
        None => None,
    };

    let password = match input.password.as_deref() {
        Some(valor) => {
            if valor.chars().count() < 12 {
                return Err(ApiError::inprocesable(
                    "La contraseña debe tener al menos 12 caracteres",
                ));
            }
            Some(valor.to_string())
        }
        None => None,
    };

    if nombre.is_none() && correo.is_none() && password.is_none() {
        return Err(ApiError::inprocesable("No hay cambios que aplicar"));
    }

    let hash = match password.as_deref() {
        Some(valor) => Some(crate::auth::hash_password(valor).map_err(|error| {
            tracing::error!(?error, "no fue posible calcular el hash de la contraseña");
            ApiError::interno("No fue posible actualizar la contraseña")
        })?),
        None => None,
    };

    let mut tx = app.db.begin().await?;

    if let Some(nuevo_correo) = &correo {
        let ocupado: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM usuarios WHERE lower(correo) = $1 AND id <> $2)",
        )
        .bind(nuevo_correo)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if ocupado {
            return Err(ApiError::conflicto("Ya existe un usuario con ese correo"));
        }
    }

    let actualizacion: Result<Option<Uuid>, sqlx::Error> = sqlx::query_scalar(
        "UPDATE usuarios
         SET nombre = COALESCE($2, nombre),
             correo = COALESCE($3, correo),
             password_hash = COALESCE($4::text, password_hash),
             debe_cambiar_password = CASE WHEN $4::text IS NULL THEN debe_cambiar_password ELSE true END,
             actualizado_en = now()
         WHERE id = $1
         RETURNING id",
    )
    .bind(id)
    .bind(nombre.as_deref())
    .bind(correo.as_deref())
    .bind(hash.as_deref())
    .fetch_optional(&mut *tx)
    .await;

    let actualizado = match actualizacion {
        Ok(valor) => valor,
        Err(error)
            if error
                .as_database_error()
                .map(|base| base.is_unique_violation())
                .unwrap_or(false) =>
        {
            return Err(ApiError::conflicto("Ya existe un usuario con ese correo"));
        }
        Err(error) => return Err(error.into()),
    };

    if actualizado.is_none() {
        return Err(ApiError::no_encontrado("Usuario no encontrado"));
    }

    if hash.is_some() {
        // Cambiar la contraseña cierra todas las sesiones del usuario.
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
        "admin.usuarios.editar",
        "usuarios",
        id,
        "EXITO",
        json!({
            "nombre": nombre,
            "correo": correo,
            "password_actualizada": hash.is_some(),
        }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({ "usuario_id": id })))
}

pub async fn respaldar_base(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Response, ApiError> {
    user.exigir("admin:backup")?;

    let conexion = Url::parse(&app.config.database_url)
        .map_err(|error| ApiError::interno(format!("DATABASE_URL no es válida: {error}")))?;
    let host = conexion.host_str().unwrap_or("127.0.0.1").to_string();
    let puerto = conexion.port().unwrap_or(5432).to_string();
    let usuario = conexion.username().to_string();
    let clave = conexion.password().unwrap_or_default().to_string();
    let base = conexion.path().trim_start_matches('/').to_string();
    if base.is_empty() {
        return Err(ApiError::interno(
            "La URL de la base de datos no incluye el nombre de la base",
        ));
    }

    let nombre = format!("respaldo-{}.dump", Utc::now().format("%Y%m%d-%H%M%S"));
    let ruta = std::env::temp_dir().join(&nombre);

    let comando = Command::new("pg_dump")
        .arg("--format=custom")
        .arg("--no-owner")
        .arg("--no-privileges")
        .arg("--host")
        .arg(&host)
        .arg("--port")
        .arg(&puerto)
        .arg("--username")
        .arg(&usuario)
        .arg("--dbname")
        .arg(&base)
        .arg("--file")
        .arg(&ruta)
        .env("PGPASSWORD", clave)
        .kill_on_drop(true)
        .output();

    let salida = tokio::time::timeout(Duration::from_secs(300), comando)
        .await
        .map_err(|_| ApiError::interno("El respaldo excedió el tiempo máximo de 5 minutos"))?
        .map_err(|error| {
            ApiError::interno(format!("No se pudo ejecutar pg_dump: {error}"))
        })?;

    if !salida.status.success() {
        let _ = tokio::fs::remove_file(&ruta).await;
        tracing::error!(
            error = %String::from_utf8_lossy(&salida.stderr),
            "falló la generación del respaldo"
        );
        return Err(ApiError::interno(
            "No se pudo generar el respaldo de la base de datos",
        ));
    }

    let metadatos = tokio::fs::metadata(&ruta).await.map_err(|error| {
        ApiError::interno(format!("No se pudo leer el respaldo generado: {error}"))
    })?;
    if metadatos.len() == 0 || metadatos.len() > 512 * 1024 * 1024 {
        let _ = tokio::fs::remove_file(&ruta).await;
        return Err(ApiError::interno(
            "El respaldo generado está vacío o excede el tamaño permitido",
        ));
    }
    let bytes = tokio::fs::read(&ruta).await.map_err(|error| {
        ApiError::interno(format!("No se pudo leer el respaldo generado: {error}"))
    })?;
    let _ = tokio::fs::remove_file(&ruta).await;

    let mut tx = app.db.begin().await?;
    audit::registrar(
        &mut tx,
        Some(user.id),
        "admin.backup",
        "base_datos",
        Uuid::nil(),
        "EXITO",
        json!({ "archivo": nombre, "tamano": bytes.len() }),
    )
    .await?;
    tx.commit().await?;

    let disposicion = format!("attachment; filename=\"{nombre}\"");
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposicion)
                .map_err(|error| ApiError::interno(error.to_string()))?,
        )
        .body(Body::from(bytes))
        .map_err(|error| ApiError::interno(error.to_string()))
}
