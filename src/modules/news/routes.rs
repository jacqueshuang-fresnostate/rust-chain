use super::presentation::{PublicNewsItemResponse, PublicNewsItemsResponse, PublicNewsQuery};
use crate::{error::AppResult, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/news", get(list_public_news_items))
        .route("/news/:id", get(get_public_news_item))
}

async fn list_public_news_items(
    State(state): State<AppState>,
    Query(query): Query<PublicNewsQuery>,
) -> AppResult<Json<PublicNewsItemsResponse>> {
    let pool = super::application::mysql_pool(&state)?;
    Ok(Json(
        super::application::list_public_news_items(&pool, query.into()).await?,
    ))
}

async fn get_public_news_item(
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
) -> AppResult<Json<PublicNewsItemResponse>> {
    let pool = super::application::mysql_pool(&state)?;
    Ok(Json(
        super::application::get_public_news_item(&pool, news_id).await?,
    ))
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_news_routes_tests.rs"]
mod tests;
