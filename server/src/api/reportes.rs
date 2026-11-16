use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{auth::AuthUser, error::ApiError, AppState};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReporteQuery {
    pub desde: Option<NaiveDate>,
    pub hasta: Option<NaiveDate>,
}

/// Altas de expedientes por usuario, agrupadas por día, mes y año.
///
/// Se basa en los movimientos `ALTA` (registro individual con custodia inicial); las
/// importaciones masivas usan `IMPORTACION` y no cuentan como alta en custodia.
pub async fn altas(
    State(app): State<AppState>,
    user: AuthUser,
    Query(consulta): Query<ReporteQuery>,
) -> Result<Json<Value>, ApiError> {
    user.exigir("reportes:leer")?;

    let desde = consulta
        .desde
        .map(|fecha| fecha.and_hms_opt(0, 0, 0).expect("hora válida").and_utc());
    let hasta = consulta
        .hasta
        .and_then(|fecha| fecha.succ_opt())
        .map(|fecha| fecha.and_hms_opt(0, 0, 0).expect("hora válida").and_utc());

    let por_dia = agrupar(&app, "YYYY-MM-DD", "day", desde, hasta).await?;
    let por_mes = agrupar(&app, "YYYY-MM", "month", desde, hasta).await?;
    let por_anio = agrupar(&app, "YYYY", "year", desde, hasta).await?;

    let total: i64 = por_anio
        .iter()
        .filter_map(|fila| fila.get("total").and_then(Value::as_i64))
        .sum();

    Ok(Json(json!({
        "total": total,
        "por_dia": por_dia,
        "por_mes": por_mes,
        "por_anio": por_anio,
    })))
}

async fn agrupar(
    app: &AppState,
    formato: &str,
    truncado: &str,
    desde: Option<DateTime<Utc>>,
    hasta: Option<DateTime<Utc>>,
) -> Result<Vec<Value>, ApiError> {
    let sql = format!(
        "SELECT to_char(date_trunc('{truncado}', m.creado_en AT TIME ZONE 'America/Mexico_City'), '{formato}'),
                u.id, u.nombre, count(*)
         FROM movimientos m
         JOIN usuarios u ON u.id = m.actor_id
         WHERE m.tipo = 'ALTA'
           AND ($1::timestamptz IS NULL OR m.creado_en >= $1)
           AND ($2::timestamptz IS NULL OR m.creado_en < $2)
         GROUP BY 1, 2, 3
         ORDER BY 1 DESC, 4 DESC"
    );

    let filas = sqlx::query_as::<_, (String, Uuid, String, i64)>(&sql)
        .bind(desde)
        .bind(hasta)
        .fetch_all(&app.db)
        .await?;

    Ok(filas
        .into_iter()
        .map(|(periodo, usuario_id, usuario, total)| {
            json!({
                "periodo": periodo,
                "usuario_id": usuario_id,
                "usuario": usuario,
                "total": total,
            })
        })
        .collect())
}
