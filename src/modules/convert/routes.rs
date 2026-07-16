use super::{
    application::{
        confirm_convert_quote_with_events as confirm_convert_quote_with_events_use_case,
        create_convert_quote, list_convert_orders, list_convert_pairs,
    },
    presentation::{
        ConfirmConvertQuoteRequest, ConfirmConvertQuoteResponse, ConvertOrdersQuery,
        ConvertOrdersResponse, ConvertPairsResponse, ConvertQuoteResponse,
        CreateConvertQuoteRequest, ListQuery,
    },
};
use crate::{error::AppResult, modules::auth::UserAuth, state::AppState};
use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{get, post},
};

pub fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/convert/pairs", get(list_pairs))
        .route("/convert/quote", post(create_quote))
        .route("/convert/confirm", post(confirm_quote))
        .route("/convert/orders", get(list_orders))
}

async fn list_pairs(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<ConvertPairsResponse>> {
    Ok(Json(list_convert_pairs(state.mysql.clone(), query).await?))
}

async fn create_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateConvertQuoteRequest>,
) -> AppResult<Json<ConvertQuoteResponse>> {
    Ok(Json(
        create_convert_quote(
            state.mysql.clone(),
            state.redis.clone(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

async fn confirm_quote(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Json(request): Json<ConfirmConvertQuoteRequest>,
) -> AppResult<Json<ConfirmConvertQuoteResponse>> {
    Ok(Json(
        confirm_convert_quote_with_events_use_case(
            state.mysql.clone(),
            state.redis.clone(),
            state.event_broadcast_hub.as_ref(),
            &claims.sub,
            request,
        )
        .await?,
    ))
}

async fn list_orders(
    UserAuth(claims): UserAuth,
    State(state): State<AppState>,
    Query(query): Query<ConvertOrdersQuery>,
) -> AppResult<Json<ConvertOrdersResponse>> {
    Ok(Json(
        list_convert_orders(state.mysql.clone(), &claims.sub, query).await?,
    ))
}
