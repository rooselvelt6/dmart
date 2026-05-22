use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
};
use dmart_shared::models::*;

type ApiResult<T> = Result<Json<ApiResponse<T>>, (StatusCode, String)>;

#[derive(serde::Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

pub async fn list_diagnosticos() -> ApiResult<Vec<Diagnostico>> {
    let diagnosticos = diagnosticos_uci();
    Ok(Json(ApiResponse::ok(diagnosticos)))
}

pub async fn search_diagnosticos(
    Query(params): Query<SearchQuery>,
) -> ApiResult<Vec<Diagnostico>> {
    let all = diagnosticos_uci();
    let query = params.q.unwrap_or_default().to_lowercase();

    if query.is_empty() {
        return Ok(Json(ApiResponse::ok(all)));
    }

    let filtered: Vec<Diagnostico> = all
        .into_iter()
        .filter(|d| {
            d.codigo.to_lowercase().contains(&query)
                || d.descripcion.to_lowercase().contains(&query)
                || d.categoria.to_lowercase().contains(&query)
        })
        .collect();

    Ok(Json(ApiResponse::ok(filtered)))
}
