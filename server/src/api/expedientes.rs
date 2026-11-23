use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{Datelike, Utc};
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActualizarExpediente {
    pub descripcion: String,
}

/// Normaliza la parte anterior a la primera barra a 8 caracteres con ceros a la izquierda.
///
/// `5259/2026` → `00005259/2026`. Sin barra, o con una parte de 8 o más caracteres, se conserva.
pub fn normalizar_numero(numero: &str) -> String {
    match numero.split_once('/') {
        Some((prefijo, resto)) if !prefijo.is_empty() && prefijo.chars().count() < 8 => {
            let relleno = 8 - prefijo.chars().count();
            format!("{}{}/{}", "0".repeat(relleno), prefijo, resto)
        }
        _ => numero.to_string(),
    }
}

/// Alta de un expediente físico: el creador queda como custodio inicial.
///
/// La custodia se mantiene con esa persona hasta que solicite la devolución y el
/// almacén acuse la recepción.
pub async fn crear(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<NuevoExpediente>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("expedientes:crear")?;

    let numero = normalizar_numero(input.numero.trim());
    if numero.is_empty() {
        return Err(ApiError::inprocesable("El número de expediente es obligatorio"));
    }
    if numero.chars().count() > 100 {
        return Err(ApiError::inprocesable("El número de expediente es demasiado largo"));
    }

    // Sin año explícito se usa el año en curso.
    let anio = input.anio.unwrap_or_else(|| Utc::now().year() as i16);
    if !(1900..=2200).contains(&anio) {
        return Err(ApiError::inprocesable("El año debe estar entre 1900 y 2200"));
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
        "INSERT INTO expedientes(numero, descripcion, unidad, anio, estado, custodio_id, version)
         VALUES ($1, COALESCE($2, ''), $3, $4, 'EN_CUSTODIA', $5, 1)
         RETURNING id",
    )
    .bind(&numero)
    .bind(descripcion)
    .bind(unidad)
    .bind(anio)
    .bind(user.id)
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
                "Ya existe un expediente con ese número",
            ));
        }
        Err(error) => return Err(error.into()),
    };

    sqlx::query("INSERT INTO custodias(expediente_id, usuario_id) VALUES ($1,$2)")
        .bind(expediente_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "INSERT INTO movimientos(expediente_id, tipo, actor_id, detalle)
         VALUES ($1,'ALTA',$2,'Registro del expediente y custodia inicial')",
    )
    .bind(expediente_id)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

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
            "estado": "EN_CUSTODIA",
            "custodio_id": user.id,
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
        json!({ "numero": numero, "unidad": unidad, "anio": anio, "custodio_inicial": user.id }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({
        "expediente_id": expediente_id,
        "numero": numero,
        "custodio_id": user.id,
        "anio": anio,
    })))
}

/// Actualización de la descripción autorizada del expediente.
pub async fn actualizar(
    State(app): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(input): ApiJson<ActualizarExpediente>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("expedientes:administrar")?;

    let descripcion = input.descripcion.trim().to_string();
    if descripcion.chars().count() > 2000 {
        return Err(ApiError::inprocesable(
            "La descripción admite hasta 2000 caracteres",
        ));
    }

    let mut tx = app.db.begin().await?;
    let fila: Option<(String, String, Option<Uuid>, i64)> = sqlx::query_as(
        "SELECT numero, estado, custodio_id, version
         FROM expedientes WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((numero, estado, custodio_id, _)) = fila else {
        return Err(ApiError::no_encontrado("Expediente no encontrado"));
    };

    let version: i64 = sqlx::query_scalar(
        "UPDATE expedientes
         SET descripcion = $2, version = version + 1
         WHERE id = $1
         RETURNING version",
    )
    .bind(id)
    .bind(&descripcion)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO movimientos(expediente_id, tipo, actor_id, detalle)
         VALUES ($1, 'EDICION', $2, 'Actualización de la descripción')",
    )
    .bind(id)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

    sync::global(
        &mut tx,
        "expediente",
        id,
        "ACTUALIZAR",
        version,
        json!({
            "id": id,
            "numero": numero,
            "descripcion": descripcion,
            "estado": estado,
            "custodio_id": custodio_id,
            "version": version,
        }),
    )
    .await?;

    audit::registrar(
        &mut tx,
        Some(user.id),
        "expedientes.editar",
        "expedientes",
        id,
        "EXITO",
        json!({ "numero": numero, "descripcion": descripcion, "version": version }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({
        "expediente_id": id,
        "descripcion": descripcion,
        "version": version,
    })))
}

#[cfg(test)]
mod pruebas {
    use super::normalizar_numero;

    #[test]
    fn rellena_con_ceros_hasta_ocho_caracteres() {
        assert_eq!(normalizar_numero("5259/2026"), "00005259/2026");
        assert_eq!(normalizar_numero("1/2026"), "00000001/2026");
        assert_eq!(normalizar_numero("12345678/2026"), "12345678/2026");
        assert_eq!(normalizar_numero("123456789/2026"), "123456789/2026");
        assert_eq!(normalizar_numero("12/34/56"), "00000012/34/56");
        assert_eq!(normalizar_numero("EXP-1"), "EXP-1");
        assert_eq!(normalizar_numero("/2026"), "/2026");
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpedienteImportado {
    pub numero: String,
    pub descripcion: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportarLista {
    pub expedientes: Vec<ExpedienteImportado>,
}

/// Importación masiva de expedientes al inventario del almacén.
///
/// No asigna custodia: los expedientes quedan en `ALMACEN` hasta que se soliciten.
pub async fn importar(
    State(app): State<AppState>,
    user: AuthUser,
    ApiJson(input): ApiJson<ImportarLista>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("expedientes:administrar")?;

    if input.expedientes.is_empty() || input.expedientes.len() > 500 {
        return Err(ApiError::inprocesable(
            "La importación admite entre 1 y 500 expedientes por lote",
        ));
    }

    let mut tx = app.db.begin().await?;
    let mut importados = 0_i64;
    let mut omitidos = 0_i64;
    let mut errores: Vec<Value> = Vec::new();
    let mut creados: Vec<Value> = Vec::new();

    for (indice, fila) in input.expedientes.iter().enumerate() {
        let numero = normalizar_numero(fila.numero.trim());

        if numero.is_empty() {
            errores.push(json!({ "fila": indice + 1, "motivo": "Número vacío" }));
            continue;
        }
        if numero.chars().count() > 100 {
            errores.push(json!({ "fila": indice + 1, "motivo": "Número demasiado largo" }));
            continue;
        }

        let anio = Utc::now().year() as i16;

        let descripcion = fila
            .descripcion
            .as_deref()
            .map(str::trim)
            .filter(|texto| !texto.is_empty());

        let insertado: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO expedientes(numero, descripcion, unidad, anio, estado, version)
             VALUES ($1, COALESCE($2, ''), NULL, $3, 'ALMACEN', 1)
             ON CONFLICT (numero) DO NOTHING
             RETURNING id",
        )
        .bind(&numero)
        .bind(descripcion)
        .bind(anio)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(expediente_id) = insertado else {
            omitidos += 1;
            continue;
        };

        importados += 1;
        creados.push(json!({ "id": expediente_id, "numero": numero }));

        sqlx::query(
            "INSERT INTO movimientos(expediente_id, tipo, actor_id, detalle)
             VALUES ($1,'IMPORTACION',$2,'Importación masiva')",
        )
        .bind(expediente_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

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
    }

    audit::registrar(
        &mut tx,
        Some(user.id),
        "expedientes.importar",
        "lote_expedientes",
        Uuid::new_v4(),
        "EXITO",
        json!({ "importados": importados, "omitidos": omitidos, "errores": errores.len() }),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(json!({
        "importados": importados,
        "omitidos": omitidos,
        "errores": errores,
        "creados": creados,
    })))
}
