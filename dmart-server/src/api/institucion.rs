use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use dmart_shared::models::*;
use crate::db::Database;
use anyhow::Error;

type ApiResult<T> = Result<Json<ApiResponse<T>>, (StatusCode, String)>;

fn err_to_str(e: Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

pub async fn get_institucion(
    State(db): State<Database>,
) -> ApiResult<InstitucionConfig> {
    let config = crate::db::get_institucion_config(&db)
        .await
        .map_err(err_to_str)?
        .unwrap_or_else(InstitucionConfig::default_config);
    Ok(Json(ApiResponse::ok(config)))
}

pub async fn upsert_institucion(
    State(db): State<Database>,
    Json(config): Json<InstitucionConfig>,
) -> ApiResult<InstitucionConfig> {
    let mut c = config;
    c.updated_at = chrono::Utc::now().to_rfc3339();
    let saved = crate::db::upsert_institucion_config(&db, c)
        .await
        .map_err(err_to_str)?;
    Ok(Json(ApiResponse::ok(saved)))
}
