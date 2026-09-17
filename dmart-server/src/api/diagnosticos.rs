use crate::db::Database;
use axum::{extract::Query, extract::State, http::StatusCode, response::Json};
use dmart_shared::models::*;

type ApiResult<T> = Result<Json<ApiResponse<T>>, (StatusCode, String)>;

#[derive(serde::Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

pub async fn list_diagnosticos(State(db): State<Database>) -> ApiResult<Vec<Diagnostico>> {
    let diagnosticos = crate::db::list_diagnosticos(&db).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            crate::security::sanitize_internal_error(&e),
        )
    })?;
    Ok(Json(ApiResponse::ok(diagnosticos)))
}

pub async fn search_diagnosticos(
    State(db): State<Database>,
    Query(params): Query<SearchQuery>,
) -> ApiResult<Vec<Diagnostico>> {
    let query = params.q.unwrap_or_default();
    let filtered = crate::db::search_diagnosticos(&db, &query)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                crate::security::sanitize_internal_error(&e),
            )
        })?;
    Ok(Json(ApiResponse::ok(filtered)))
}
