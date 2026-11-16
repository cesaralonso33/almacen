use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::{auth::AuthUser, error::ApiError, AppState};

/// Directorio mínimo de usuarios activos para elegir el destino de una transferencia.
pub async fn directorio(
    State(app): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, ApiError> {
    user.exigir("expedientes:leer")?;

    let filas = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "SELECT u.id, u.nombre
         FROM usuarios u
         WHERE u.activo AND u.id <> $1
         ORDER BY u.nombre
         LIMIT 200",
    )
    .bind(user.id)
    .fetch_all(&app.db)
    .await?;

    let usuarios: Vec<Value> = filas
        .into_iter()
        .map(|(id, nombre)| json!({ "id": id, "nombre": nombre }))
        .collect();

    Ok(Json(json!({ "usuarios": usuarios })))
}
