use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{api::entrada::ApiJson, audit, auth::AuthUser, error::ApiError, sync, AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NuevoExpediente {
    pub numero: String,
    pub descripcion: Option<String>,
    pub unidad: Option<String>,
    pub anio: Option<i16>,
}

/// Alta de un expediente físico en el inventario.
pub async fn crear(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<NuevoExpediente>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("expedientes:administrar")?;

    let numero = input.numero.trim();
    if numero.is_empty() {
        return Err(ApiError::inprocesable("El número de expediente es obligatorio"));
    }
    if numero.chars().count() > 100 {
        return Err(ApiError::inprocesable("El número de expediente es demasiado largo"));
    }

    let anio = input.anio;
    if let Some(valor) = anio {
        if !(1900..=2200).contains(&valor) {
            return Err(ApiError::inprocesable("El año debe estar entre 1900 y 2200"));
        }
    }

    let descripcion = input
        .descripcion
        .as_deref()
        .map(str::trim)
        .filter(|texto| !texto.is_empty());
    let unidad = input
        .unidad
        .as_deref()
        .map(str::trim)
        .filter(|texto| !texto.is_empty());

    let mut tx = app.db.begin().await?;

    let insercion: Result<Uuid, sqlx::Error> = sqlx::query_scalar(
        "INSERT INTO expedientes(numero, descripcion, unidad, anio, version)
         VALUES ($1, COALESCE($2, ''), $3, $4, 1)
         RETURNING id",
    )
    .bind(numero)
    .bind(descripcion)
    .bind(unidad)
    .bind(anio)
    .fetch_one(&mut *tx)
    .await;

    let expediente_id = match insercion {
        Ok(id) => id,
        Err(error)
            if error
                .as_database_error()
                .map(|base| base.is_unique_violation())
                .unwrap_or(false) =>
        {
            return Err(ApiError::conflicto(
                "Ya existe un expediente con ese número en la misma unidad y año",
            ));
        }
        Err(error) => return Err(error.into()),
    };

    sync::global(
        &mut tx,
        "expediente",
        expediente_id,
        "CREAR",
        1,
        json!({
            "id": expediente_id,
            "numero": numero,
            "descripcion": descripcion,
            "estado": "ALMACEN",
            "version": 1,
        }),
    )
    .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "expedientes.crear",
        "expedientes",
        expediente_id,
        "EXITO",
        json!({ "numero": numero, "unidad": unidad, "anio": anio }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({ "expediente_id": expediente_id })))
}
