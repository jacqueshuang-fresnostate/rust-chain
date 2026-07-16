use crate::modules::admin::service::{admin_id_from_subject, load_market_feed_runtime};
use crate::{
    error::AppResult,
    modules::{
        admin::{
            application::{
                assign_admin_user_agent as assign_user_agent_use_case,
                create_admin_agent as create_agent_use_case,
                create_admin_agent_commission_rule as create_agent_commission_rule_use_case,
                create_admin_asset as create_asset_use_case,
                create_admin_convert_pair as create_convert_pair_use_case,
                create_admin_country as create_admin_country_use_case,
                create_admin_deposit_address_pool as create_deposit_address_pool_use_case,
                create_admin_deposit_address_pool_batch as create_deposit_address_pool_batch_use_case,
                create_admin_deposit_network_config as create_deposit_network_config_use_case,
                create_admin_market_strategy as create_market_strategy_use_case,
                create_admin_new_coin_project as create_new_coin_project_use_case,
                create_admin_news_item as create_admin_news_item_use_case,
                create_admin_risk_rule as create_risk_rule_use_case,
                create_admin_smtp_config as create_smtp_config_use_case,
                create_admin_trading_pair as create_trading_pair_use_case,
                create_admin_user as create_admin_user_use_case,
                delete_admin_asset as delete_asset_use_case,
                delete_admin_convert_pair as delete_convert_pair_use_case,
                distribute_admin_new_coin as distribute_new_coin_use_case,
                get_admin_agent as get_agent_use_case, get_admin_asset as get_asset_use_case,
                get_admin_convert_order as get_convert_order_use_case,
                get_admin_convert_pair as get_convert_pair_use_case,
                get_admin_dashboard as get_admin_dashboard_use_case,
                get_admin_deposit_address_pool as get_deposit_address_pool_use_case,
                get_admin_kyc_config as get_kyc_config_use_case,
                get_admin_kyc_submission as get_kyc_submission_use_case,
                get_admin_margin_liquidation as get_margin_liquidation_use_case,
                get_admin_market_feed_config as get_market_feed_config_use_case,
                get_admin_market_feed_status as get_market_feed_status_use_case,
                get_admin_news_item as get_admin_news_item_use_case,
                get_admin_platform_brand as get_platform_brand_use_case,
                get_admin_security_policy as get_security_policy_use_case,
                get_admin_smtp_config as get_smtp_config_use_case,
                get_admin_trading_pair as get_trading_pair_use_case,
                get_admin_upload_config as get_upload_config_use_case,
                get_admin_user as get_admin_user_use_case,
                list_admin_agent_commission_rules as list_agent_commission_rules_use_case,
                list_admin_agent_commissions as list_agent_commissions_use_case,
                list_admin_agent_users as list_agent_users_use_case,
                list_admin_agents as list_agents_use_case,
                list_admin_assets as list_assets_use_case,
                list_admin_audit_logs as list_admin_audit_logs_use_case,
                list_admin_convert_orders as list_convert_orders_use_case,
                list_admin_convert_pairs as list_convert_pairs_use_case,
                list_admin_countries as list_admin_countries_use_case,
                list_admin_deposit_address_pool as list_deposit_address_pool_use_case,
                list_admin_deposit_network_configs as list_deposit_network_configs_use_case,
                list_admin_kyc_submissions as list_kyc_submissions_use_case,
                list_admin_margin_liquidations as list_margin_liquidations_use_case,
                list_admin_market_feed_credentials as list_market_feed_credentials_use_case,
                list_admin_market_strategies as list_market_strategies_use_case,
                list_admin_new_coin_distributions as list_new_coin_distributions_use_case,
                list_admin_new_coin_distributions_for_project as list_new_coin_distributions_for_project_use_case,
                list_admin_new_coin_lock_positions as list_new_coin_lock_positions_use_case,
                list_admin_new_coin_projects as list_new_coin_projects_use_case,
                list_admin_new_coin_purchases as list_new_coin_purchases_use_case,
                list_admin_new_coin_subscriptions as list_new_coin_subscriptions_use_case,
                list_admin_new_coin_subscriptions_for_project as list_new_coin_subscriptions_for_project_use_case,
                list_admin_new_coin_unlocks as list_new_coin_unlocks_use_case,
                list_admin_news_items as list_admin_news_items_use_case,
                list_admin_risk_events as list_risk_events_use_case,
                list_admin_risk_rules as list_risk_rules_use_case,
                list_admin_smtp_configs as list_smtp_configs_use_case,
                list_admin_trading_pairs as list_trading_pairs_use_case,
                list_admin_users as list_admin_users_use_case,
                list_admin_wallet_accounts as list_wallet_accounts_use_case,
                list_admin_wallet_ledger as list_wallet_ledger_use_case, mysql_pool,
                recharge_admin_user_wallet as recharge_admin_user_wallet_use_case,
                reclaim_admin_deposit_address_pool as reclaim_deposit_address_pool_use_case,
                reload_admin_market_feed_config as reload_market_feed_config_use_case,
                reset_admin_user_two_factor as reset_admin_user_two_factor_use_case,
                review_admin_kyc_submission as review_kyc_submission_use_case,
                save_admin_kyc_config as save_kyc_config_use_case,
                save_admin_market_feed_config as save_market_feed_config_use_case,
                save_admin_platform_brand as save_platform_brand_use_case,
                save_admin_smtp_config as save_smtp_config_use_case,
                save_admin_smtp_delivery_settings as save_smtp_delivery_settings_use_case,
                save_admin_upload_config as save_upload_config_use_case,
                send_admin_smtp_test as send_smtp_test_use_case,
                update_admin_agent_commission_rule as update_agent_commission_rule_use_case,
                update_admin_agent_commission_status as update_agent_commission_status_use_case,
                update_admin_agent_status as update_agent_status_use_case,
                update_admin_asset as update_asset_use_case,
                update_admin_convert_pair as update_convert_pair_use_case,
                update_admin_country as update_admin_country_use_case,
                update_admin_country_status as update_admin_country_status_use_case,
                update_admin_deposit_address_pool as update_deposit_address_pool_use_case,
                update_admin_deposit_network_config as update_deposit_network_config_use_case,
                update_admin_market_strategy as update_market_strategy_use_case,
                update_admin_market_strategy_status as update_market_strategy_status_use_case,
                update_admin_new_coin_lifecycle as update_new_coin_lifecycle_use_case,
                update_admin_new_coin_post_listing_purchase as update_new_coin_post_listing_purchase_use_case,
                update_admin_new_coin_unlock_fee_rule as update_new_coin_unlock_fee_rule_use_case,
                update_admin_new_coin_unlock_rule as update_new_coin_unlock_rule_use_case,
                update_admin_news_item as update_admin_news_item_use_case,
                update_admin_news_status as update_admin_news_status_use_case,
                update_admin_risk_rule_status as update_risk_rule_status_use_case,
                update_admin_security_policy as update_security_policy_use_case,
                update_admin_smtp_config as update_smtp_config_use_case,
                update_admin_trading_pair as update_trading_pair_use_case,
                update_admin_trading_pair_status as update_trading_pair_status_use_case,
                upload_admin_image as upload_image_use_case,
                upsert_admin_market_feed_credential as upsert_market_feed_credential_use_case,
                upsert_admin_new_coin_convert_rule as upsert_new_coin_convert_rule_use_case,
            },
            infrastructure::multipart_file_input,
            presentation::{
                AdminAgentCommissionQuery, AdminAgentCommissionResponse,
                AdminAgentCommissionRuleQuery, AdminAgentCommissionRuleResponse,
                AdminAgentCommissionRulesResponse, AdminAgentCommissionsResponse, AdminAgentQuery,
                AdminAgentResponse, AdminAgentUsersQuery, AdminAgentUsersResponse,
                AdminAgentsResponse, AdminAssetQuery, AdminAssetResponse, AdminAssetsResponse,
                AdminAuditLogsQuery, AdminAuditLogsResponse, AdminConvertOrdersQuery,
                AdminConvertPairQuery, AdminCountriesQuery, AdminCountriesResponse,
                AdminCountryResponse, AdminDashboardResponse, AdminDepositAddressPoolBatchResponse,
                AdminDepositAddressPoolQuery, AdminDepositAddressPoolResponse,
                AdminDepositAddressPoolResponseList, AdminDepositNetworkConfigQuery,
                AdminDepositNetworkConfigResponse, AdminDepositNetworkConfigResponseList,
                AdminKycSubmissionQuery, AdminMarginLiquidationQuery,
                AdminMarginLiquidationResponse, AdminMarginLiquidationsResponse,
                AdminMarketStrategiesResponse, AdminMarketStrategyQuery,
                AdminMarketStrategyResponse, AdminNewCoinFlatListQuery,
                AdminNewCoinLockPositionQuery, AdminNewCoinProjectQuery, AdminNewCoinPurchaseQuery,
                AdminNewCoinScopedListQuery, AdminNewCoinUnlockQuery, AdminNewsItemResponse,
                AdminNewsItemsResponse, AdminNewsQuery, AdminRiskEventQuery, AdminRiskRuleQuery,
                AdminTradingPairQuery, AdminTradingPairResponse, AdminTradingPairsResponse,
                AdminUserQuery, AdminUserRechargeRequest, AdminUserRechargeResponse,
                AdminUserReferralResponse, AdminUserResponse, AdminUserTwoFactorResetResponse,
                AdminUsersResponse, AdminWalletAccountQuery, AdminWalletAccountsResponse,
                AdminWalletLedgerQuery, AdminWalletLedgerResponseList, AssignUserAgentRequest,
                ConvertOrderResponse, ConvertOrdersResponse, ConvertPairResponse,
                ConvertPairsResponse, CreateAdminCountryRequest, CreateAdminNewsItemRequest,
                CreateAdminUserRequest, CreateAgentCommissionRuleRequest, CreateAgentRequest,
                CreateAssetRequest, CreateConvertPairRequest, CreateDepositAddressPoolBatchRequest,
                CreateDepositAddressPoolRequest, CreateDepositNetworkConfigRequest,
                CreateMarketStrategyRequest, CreateNewCoinProjectRequest, CreateRiskRuleRequest,
                CreateTradingPairRequest, DeleteAssetRequest, DeleteConvertPairRequest,
                DistributeNewCoinRequest, MarketFeedConfigResponse, MarketFeedStatusResponse,
                MarketSourceCredentialResponse, MarketSourceCredentialsResponse,
                NewCoinConvertRuleResponse, NewCoinDistributionResponse,
                NewCoinDistributionsResponse, NewCoinLockPositionsResponse, NewCoinProjectResponse,
                NewCoinProjectsResponse, NewCoinPurchasesResponse, NewCoinSubscriptionsResponse,
                NewCoinUnlocksResponse, ReclaimDepositAddressPoolRequest, ReloadMarketFeedRequest,
                ReloadMarketFeedResponse, ResetUserTwoFactorRequest, RiskEventsResponse,
                RiskRuleResponse, RiskRulesResponse, SaveMarketFeedConfigRequest,
                SaveSmtpConfigRequest, SaveSmtpDeliverySettingsRequest, SaveUploadConfigRequest,
                SendSmtpTestRequest, SendSmtpTestResponse, SmtpConfigListResponse,
                SmtpConfigResponse, SmtpDeliverySettingsResponse, UpdateAdminCountryRequest,
                UpdateAdminCountryStatusRequest, UpdateAdminNewsItemRequest,
                UpdateAdminNewsStatusRequest, UpdateAgentCommissionRuleRequest,
                UpdateAgentCommissionStatusRequest, UpdateAgentStatusRequest, UpdateAssetRequest,
                UpdateConvertPairRequest, UpdateDepositAddressPoolRequest,
                UpdateDepositNetworkConfigRequest, UpdateMarketStrategyRequest,
                UpdateMarketStrategyStatusRequest, UpdateNewCoinLifecycleRequest,
                UpdateNewCoinPostListingPurchaseRequest, UpdateNewCoinUnlockFeeRuleRequest,
                UpdateNewCoinUnlockRuleRequest, UpdateRiskRuleStatusRequest,
                UpdateSecurityPolicyRequest, UpdateTradingPairRequest,
                UpdateTradingPairStatusRequest, UploadConfigResponse, UploadImageResponse,
                UpsertMarketSourceCredentialRequest, UpsertNewCoinConvertRuleRequest,
            },
            service::MAX_UPLOAD_BODY_SIZE_BYTES,
        },
        auth::AdminAuth,
        kyc::{
            KycConfigResponse, KycSubmissionResponse, KycSubmissionsResponse,
            ReviewKycSubmissionRequest, SaveKycConfigRequest,
        },
        platform::{PlatformBrandResponse, SavePlatformBrandRequest},
        security::UserSecurityPolicy,
    },
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard", get(get_admin_dashboard))
        .route(
            "/countries",
            get(list_admin_countries).post(create_admin_country),
        )
        .route("/countries/:id", patch(update_admin_country))
        .route("/countries/:id/status", patch(update_admin_country_status))
        .route(
            "/news",
            get(list_admin_news_items).post(create_admin_news_item),
        )
        .route(
            "/news/:id",
            get(get_admin_news_item).patch(update_admin_news_item),
        )
        .route("/news/:id/status", patch(update_admin_news_status))
        .route("/users", get(list_admin_users).post(create_admin_user))
        .route("/users/:id", get(get_admin_user))
        .route("/users/:id/recharge", post(recharge_admin_user_wallet))
        .route("/kyc/config", get(get_kyc_config).patch(save_kyc_config))
        .route("/kyc/submissions", get(list_kyc_submission_routes))
        .route("/kyc/submissions/:id", get(get_kyc_submission))
        .route("/kyc/submissions/:id/review", patch(review_kyc_submission))
        .route("/assets", get(list_assets).post(create_asset))
        .route(
            "/assets/:id",
            get(get_asset).patch(update_asset).delete(delete_asset),
        )
        .route("/wallet/accounts", get(list_wallet_accounts))
        .route("/wallet/ledger", get(list_wallet_ledger))
        .route(
            "/deposit-network-configs",
            get(list_deposit_network_configs).post(create_deposit_network_config),
        )
        .route(
            "/deposit-network-configs/:id",
            patch(update_deposit_network_config),
        )
        .route(
            "/deposit-address-pool",
            get(list_deposit_address_pool).post(create_deposit_address_pool),
        )
        .route(
            "/deposit-address-pool/batch",
            post(create_deposit_address_pool_batch),
        )
        .route(
            "/deposit-address-pool/:id",
            get(get_deposit_address_pool).patch(update_deposit_address_pool),
        )
        .route(
            "/deposit-address-pool/:id/reclaim",
            post(reclaim_deposit_address_pool),
        )
        .route("/risk/rules", get(list_risk_rules).post(create_risk_rule))
        .route("/risk/rules/:id/status", patch(update_risk_rule_status))
        .route("/risk/events", get(list_risk_events))
        .route(
            "/market-pairs",
            get(list_trading_pairs).post(create_trading_pair),
        )
        .route(
            "/market-pairs/:id",
            get(get_trading_pair).patch(update_trading_pair),
        )
        .route(
            "/market-pairs/:id/status",
            patch(update_trading_pair_status),
        )
        .route(
            "/market-feed/config",
            get(get_market_feed_config).patch(save_market_feed_config),
        )
        .route("/market-feed/reload", post(reload_market_feed_config))
        .route("/market-feed/status", get(get_market_feed_status))
        .route(
            "/market-feed/credentials",
            get(list_market_feed_credentials),
        )
        .route(
            "/market-feed/credentials/:provider",
            patch(upsert_market_feed_credential),
        )
        .route(
            "/smtp/config",
            get(get_smtp_config).patch(save_smtp_config_route),
        )
        .route(
            "/smtp/configs",
            get(list_smtp_config_route).post(create_smtp_config_route),
        )
        .route("/smtp/configs/:id", patch(update_smtp_config_route))
        .route(
            "/smtp/delivery-settings",
            patch(save_smtp_delivery_settings_route),
        )
        .route("/smtp/test", post(send_smtp_test))
        .route(
            "/upload/config",
            get(get_upload_config).patch(save_upload_config_route),
        )
        .route(
            "/platform/brand",
            get(get_platform_brand).patch(save_platform_brand),
        )
        .route(
            "/uploads/images",
            post(upload_image_route).layer(DefaultBodyLimit::max(MAX_UPLOAD_BODY_SIZE_BYTES)),
        )
        .route(
            "/new-coins",
            get(list_new_coin_projects).post(create_new_coin_project),
        )
        .route("/new-coins/:id/lifecycle", patch(update_new_coin_lifecycle))
        .route("/new-coins/:id/distribute", post(distribute_new_coin))
        .route(
            "/new-coins/:id/unlock-rule",
            patch(update_new_coin_unlock_rule),
        )
        .route(
            "/new-coins/:id/post-listing-purchase",
            patch(update_new_coin_post_listing_purchase),
        )
        .route(
            "/new-coins/:id/unlock-fee-rule",
            patch(update_new_coin_unlock_fee_rule),
        )
        .route(
            "/new-coins/:id/subscriptions",
            get(list_new_coin_subscriptions),
        )
        .route(
            "/new-coins/:id/distributions",
            get(list_new_coin_distributions),
        )
        .route(
            "/new-coins/subscriptions",
            get(list_all_new_coin_subscriptions),
        )
        .route(
            "/new-coins/distributions",
            get(list_all_new_coin_distributions),
        )
        .route("/new-coins/purchases", get(list_new_coin_purchases))
        .route(
            "/new-coins/lock-positions",
            get(list_new_coin_lock_positions),
        )
        .route("/new-coins/unlocks", get(list_new_coin_unlocks))
        .route(
            "/convert/pairs",
            get(list_convert_pairs).post(create_convert_pair),
        )
        .route(
            "/convert/pairs/:id",
            get(get_convert_pair)
                .patch(update_convert_pair)
                .delete(delete_convert_pair),
        )
        .route(
            "/convert/new-coin-rules",
            post(upsert_new_coin_convert_rule),
        )
        .route("/convert/orders", get(list_convert_orders))
        .route("/convert/orders/:id", get(get_convert_order))
        .route(
            "/market-strategies",
            get(list_market_strategies).post(create_market_strategy),
        )
        .route("/market-strategies/:id", patch(update_market_strategy))
        .route(
            "/market-strategies/:id/status",
            patch(update_market_strategy_status),
        )
        .route(
            "/security-policy",
            get(get_security_policy).patch(update_security_policy),
        )
        .route("/users/:id/2fa/reset", post(reset_admin_user_two_factor))
        .route("/audit-logs", get(list_admin_audit_logs))
        .route("/margin/liquidations", get(list_margin_liquidations))
        .route("/margin/liquidations/:id", get(get_margin_liquidation))
        .route("/agents", get(list_agents).post(create_agent))
        .route("/agents/:id", get(get_agent))
        .route("/agents/:id/status", patch(update_agent_status))
        .route("/agents/:id/users", get(list_agent_users))
        .route("/users/:id/agent", patch(assign_user_agent))
        .route(
            "/agent-commission-rules",
            get(list_agent_commission_rules).post(create_agent_commission_rule),
        )
        .route(
            "/agent-commission-rules/:id",
            patch(update_agent_commission_rule),
        )
        .route("/agent-commissions", get(list_agent_commissions))
        .route(
            "/agent-commissions/:id/status",
            patch(update_agent_commission_status),
        )
}

async fn list_agents(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentQuery>,
) -> AppResult<Json<AdminAgentsResponse>> {
    Ok(Json(
        list_agents_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_agent(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
) -> AppResult<Json<AdminAgentResponse>> {
    Ok(Json(
        get_agent_use_case(state.mysql.clone(), agent_id).await?,
    ))
}

async fn create_agent(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAgentRequest>,
) -> AppResult<Json<AdminAgentResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_agent_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_agent_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
    Json(request): Json<UpdateAgentStatusRequest>,
) -> AppResult<Json<AdminAgentResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_agent_status_use_case(state.mysql.clone(), admin_id, agent_id, request).await?,
    ))
}

async fn list_agent_users(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(agent_id): Path<u64>,
    Query(query): Query<AdminAgentUsersQuery>,
) -> AppResult<Json<AdminAgentUsersResponse>> {
    Ok(Json(
        list_agent_users_use_case(state.mysql.clone(), agent_id, query.limit).await?,
    ))
}

async fn assign_user_agent(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
    Json(request): Json<AssignUserAgentRequest>,
) -> AppResult<Json<AdminUserReferralResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        assign_user_agent_use_case(state.mysql.clone(), admin_id, user_id, request).await?,
    ))
}

async fn list_agent_commission_rules(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentCommissionRuleQuery>,
) -> AppResult<Json<AdminAgentCommissionRulesResponse>> {
    Ok(Json(
        list_agent_commission_rules_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn create_agent_commission_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAgentCommissionRuleRequest>,
) -> AppResult<Json<AdminAgentCommissionRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_agent_commission_rule_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_agent_commission_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(rule_id): Path<u64>,
    Json(request): Json<UpdateAgentCommissionRuleRequest>,
) -> AppResult<Json<AdminAgentCommissionRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_agent_commission_rule_use_case(state.mysql.clone(), admin_id, rule_id, request)
            .await?,
    ))
}

async fn list_agent_commissions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAgentCommissionQuery>,
) -> AppResult<Json<AdminAgentCommissionsResponse>> {
    Ok(Json(
        list_agent_commissions_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn update_agent_commission_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(commission_id): Path<u64>,
    Json(request): Json<UpdateAgentCommissionStatusRequest>,
) -> AppResult<Json<AdminAgentCommissionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_agent_commission_status_use_case(
            state.mysql.clone(),
            admin_id,
            commission_id,
            request,
        )
        .await?,
    ))
}

async fn get_market_feed_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<MarketFeedConfigResponse>>> {
    Ok(Json(
        get_market_feed_config_use_case(state.mysql.clone()).await?,
    ))
}

async fn save_market_feed_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveMarketFeedConfigRequest>,
) -> AppResult<Json<MarketFeedConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = save_market_feed_config_use_case(state.mysql.clone(), admin_id, request).await?;
    Ok(Json(config))
}

async fn get_market_feed_status(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketFeedStatusResponse>> {
    let runtime = load_market_feed_runtime(&state).await;
    Ok(Json(
        get_market_feed_status_use_case(state.mysql.clone(), runtime).await?,
    ))
}

async fn list_market_feed_credentials(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<MarketSourceCredentialsResponse>> {
    Ok(Json(
        list_market_feed_credentials_use_case(state.mysql.clone()).await?,
    ))
}

async fn upsert_market_feed_credential(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Json(request): Json<UpsertMarketSourceCredentialRequest>,
) -> AppResult<Json<MarketSourceCredentialResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let key = state.settings.exposed_credential_encryption_key();
    let credential = upsert_market_feed_credential_use_case(
        state.mysql.clone(),
        admin_id,
        provider,
        key,
        request,
    )
    .await?;
    Ok(Json(credential))
}

async fn get_smtp_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<SmtpConfigResponse>>> {
    Ok(Json(get_smtp_config_use_case(state.mysql.clone()).await?))
}

async fn list_smtp_config_route(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<SmtpConfigListResponse>> {
    Ok(Json(list_smtp_configs_use_case(state.mysql.clone()).await?))
}

async fn create_smtp_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveSmtpConfigRequest>,
) -> AppResult<Json<SmtpConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = create_smtp_config_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

async fn update_smtp_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(config_id): Path<u64>,
    Json(request): Json<SaveSmtpConfigRequest>,
) -> AppResult<Json<SmtpConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = update_smtp_config_use_case(
        state.mysql.clone(),
        admin_id,
        config_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

async fn save_smtp_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveSmtpConfigRequest>,
) -> AppResult<Json<SmtpConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = save_smtp_config_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

async fn save_smtp_delivery_settings_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveSmtpDeliverySettingsRequest>,
) -> AppResult<Json<SmtpDeliverySettingsResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let settings =
        save_smtp_delivery_settings_use_case(state.mysql.clone(), admin_id, request).await?;
    Ok(Json(settings))
}

async fn send_smtp_test(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SendSmtpTestRequest>,
) -> AppResult<Json<SendSmtpTestResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let response = send_smtp_test_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        state.email_sender.clone(),
        request,
    )
    .await?;
    Ok(Json(response))
}

async fn get_upload_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<Option<UploadConfigResponse>>> {
    Ok(Json(get_upload_config_use_case(state.mysql.clone()).await?))
}

async fn save_upload_config_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveUploadConfigRequest>,
) -> AppResult<Json<UploadConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let config = save_upload_config_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        request,
    )
    .await?;
    Ok(Json(config))
}

async fn get_platform_brand(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<PlatformBrandResponse>> {
    Ok(Json(
        get_platform_brand_use_case(state.mysql.clone()).await?,
    ))
}

async fn save_platform_brand(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SavePlatformBrandRequest>,
) -> AppResult<Json<PlatformBrandResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        save_platform_brand_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn upload_image_route(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    multipart: Multipart,
) -> AppResult<Json<UploadImageResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    let input = multipart_file_input(multipart).await?;
    let response = upload_image_use_case(
        state.mysql.clone(),
        admin_id,
        state.settings.exposed_credential_encryption_key(),
        input,
    )
    .await?;
    Ok(Json(response))
}

async fn reload_market_feed_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<ReloadMarketFeedRequest>,
) -> AppResult<Json<ReloadMarketFeedResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        reload_market_feed_config_use_case(state, admin_id, request).await?,
    ))
}

async fn get_admin_dashboard(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<AdminDashboardResponse>> {
    let runtime = load_market_feed_runtime(&state).await;
    Ok(Json(
        get_admin_dashboard_use_case(state.mysql.clone(), runtime).await?,
    ))
}

async fn create_admin_user(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminUserRequest>,
) -> AppResult<Json<AdminUserResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_admin_user_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn get_security_policy(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<UserSecurityPolicy>> {
    Ok(Json(
        get_security_policy_use_case(state.mysql.clone()).await?,
    ))
}

async fn update_security_policy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<UpdateSecurityPolicyRequest>,
) -> AppResult<Json<UserSecurityPolicy>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_security_policy_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn reset_admin_user_two_factor(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
    Json(request): Json<ResetUserTwoFactorRequest>,
) -> AppResult<Json<AdminUserTwoFactorResetResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        reset_admin_user_two_factor_use_case(state.mysql.clone(), admin_id, user_id, request)
            .await?,
    ))
}

async fn list_admin_users(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminUserQuery>,
) -> AppResult<Json<AdminUsersResponse>> {
    Ok(Json(
        list_admin_users_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_admin_user(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
) -> AppResult<Json<AdminUserResponse>> {
    Ok(Json(
        get_admin_user_use_case(state.mysql.clone(), user_id).await?,
    ))
}

async fn recharge_admin_user_wallet(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(user_id): Path<u64>,
    Json(request): Json<AdminUserRechargeRequest>,
) -> AppResult<Json<AdminUserRechargeResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        recharge_admin_user_wallet_use_case(state.mysql.clone(), admin_id, user_id, request)
            .await?,
    ))
}

async fn get_kyc_config(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> AppResult<Json<KycConfigResponse>> {
    Ok(Json(get_kyc_config_use_case(state.mysql.clone()).await?))
}

async fn save_kyc_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<SaveKycConfigRequest>,
) -> AppResult<Json<KycConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        save_kyc_config_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn list_kyc_submission_routes(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminKycSubmissionQuery>,
) -> AppResult<Json<KycSubmissionsResponse>> {
    Ok(Json(
        list_kyc_submissions_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_kyc_submission(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(submission_id): Path<u64>,
) -> AppResult<Json<KycSubmissionResponse>> {
    Ok(Json(
        get_kyc_submission_use_case(state.mysql.clone(), submission_id).await?,
    ))
}

async fn review_kyc_submission(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(submission_id): Path<u64>,
    Json(request): Json<ReviewKycSubmissionRequest>,
) -> AppResult<Json<KycSubmissionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        review_kyc_submission_use_case(state.mysql.clone(), admin_id, submission_id, request)
            .await?,
    ))
}

async fn list_admin_countries(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminCountriesQuery>,
) -> AppResult<Json<AdminCountriesResponse>> {
    Ok(Json(
        list_admin_countries_use_case(&mysql_pool(&state)?, query).await?,
    ))
}

async fn create_admin_country(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminCountryRequest>,
) -> AppResult<Json<AdminCountryResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_admin_country_use_case(&mysql_pool(&state)?, admin_id, request).await?,
    ))
}

async fn update_admin_country(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(country_id): Path<u64>,
    Json(request): Json<UpdateAdminCountryRequest>,
) -> AppResult<Json<AdminCountryResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_admin_country_use_case(&mysql_pool(&state)?, admin_id, country_id, request).await?,
    ))
}

async fn update_admin_country_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(country_id): Path<u64>,
    Json(request): Json<UpdateAdminCountryStatusRequest>,
) -> AppResult<Json<AdminCountryResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_admin_country_status_use_case(&mysql_pool(&state)?, admin_id, country_id, request)
            .await?,
    ))
}

async fn list_admin_news_items(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewsQuery>,
) -> AppResult<Json<AdminNewsItemsResponse>> {
    Ok(Json(
        list_admin_news_items_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_admin_news_item(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    Ok(Json(
        get_admin_news_item_use_case(state.mysql.clone(), news_id).await?,
    ))
}

async fn create_admin_news_item(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAdminNewsItemRequest>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_admin_news_item_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_admin_news_item(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
    Json(request): Json<UpdateAdminNewsItemRequest>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_admin_news_item_use_case(state.mysql.clone(), admin_id, news_id, request).await?,
    ))
}

async fn update_admin_news_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(news_id): Path<u64>,
    Json(request): Json<UpdateAdminNewsStatusRequest>,
) -> AppResult<Json<AdminNewsItemResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_admin_news_status_use_case(state.mysql.clone(), admin_id, news_id, request).await?,
    ))
}

async fn list_assets(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAssetQuery>,
) -> AppResult<Json<AdminAssetsResponse>> {
    Ok(Json(
        list_assets_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_asset(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
) -> AppResult<Json<AdminAssetResponse>> {
    Ok(Json(
        get_asset_use_case(state.mysql.clone(), asset_id).await?,
    ))
}

async fn update_asset(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
    Json(request): Json<UpdateAssetRequest>,
) -> AppResult<Json<AdminAssetResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_asset_use_case(state.mysql.clone(), admin_id, asset_id, request).await?,
    ))
}

async fn create_asset(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateAssetRequest>,
) -> AppResult<Json<AdminAssetResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_asset_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn delete_asset(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(asset_id): Path<u64>,
    Json(request): Json<DeleteAssetRequest>,
) -> AppResult<StatusCode> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    delete_asset_use_case(state.mysql.clone(), admin_id, asset_id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_wallet_accounts(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletAccountQuery>,
) -> AppResult<Json<AdminWalletAccountsResponse>> {
    Ok(Json(
        list_wallet_accounts_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_wallet_ledger(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminWalletLedgerQuery>,
) -> AppResult<Json<AdminWalletLedgerResponseList>> {
    Ok(Json(
        list_wallet_ledger_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_deposit_network_configs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminDepositNetworkConfigQuery>,
) -> AppResult<Json<AdminDepositNetworkConfigResponseList>> {
    Ok(Json(
        list_deposit_network_configs_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn create_deposit_network_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateDepositNetworkConfigRequest>,
) -> AppResult<Json<AdminDepositNetworkConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_deposit_network_config_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_deposit_network_config(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(config_id): Path<u64>,
    Json(request): Json<UpdateDepositNetworkConfigRequest>,
) -> AppResult<Json<AdminDepositNetworkConfigResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_deposit_network_config_use_case(state.mysql.clone(), admin_id, config_id, request)
            .await?,
    ))
}

async fn list_deposit_address_pool(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminDepositAddressPoolQuery>,
) -> AppResult<Json<AdminDepositAddressPoolResponseList>> {
    Ok(Json(
        list_deposit_address_pool_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_deposit_address_pool(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(address_id): Path<u64>,
) -> AppResult<Json<AdminDepositAddressPoolResponse>> {
    Ok(Json(
        get_deposit_address_pool_use_case(state.mysql.clone(), address_id).await?,
    ))
}

async fn create_deposit_address_pool(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateDepositAddressPoolRequest>,
) -> AppResult<Json<AdminDepositAddressPoolResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_deposit_address_pool_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn create_deposit_address_pool_batch(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateDepositAddressPoolBatchRequest>,
) -> AppResult<Json<AdminDepositAddressPoolBatchResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_deposit_address_pool_batch_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_deposit_address_pool(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(address_id): Path<u64>,
    Json(request): Json<UpdateDepositAddressPoolRequest>,
) -> AppResult<Json<AdminDepositAddressPoolResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_deposit_address_pool_use_case(state.mysql.clone(), admin_id, address_id, request)
            .await?,
    ))
}

async fn reclaim_deposit_address_pool(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(address_id): Path<u64>,
    Json(request): Json<ReclaimDepositAddressPoolRequest>,
) -> AppResult<Json<AdminDepositAddressPoolResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        reclaim_deposit_address_pool_use_case(state.mysql.clone(), admin_id, address_id, request)
            .await?,
    ))
}

async fn list_risk_rules(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminRiskRuleQuery>,
) -> AppResult<Json<RiskRulesResponse>> {
    Ok(Json(
        list_risk_rules_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn create_risk_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateRiskRuleRequest>,
) -> AppResult<Json<RiskRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_risk_rule_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_risk_rule_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(rule_id): Path<u64>,
    Json(request): Json<UpdateRiskRuleStatusRequest>,
) -> AppResult<Json<RiskRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_risk_rule_status_use_case(state.mysql.clone(), admin_id, rule_id, request).await?,
    ))
}

async fn list_risk_events(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminRiskEventQuery>,
) -> AppResult<Json<RiskEventsResponse>> {
    Ok(Json(
        list_risk_events_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_trading_pairs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminTradingPairQuery>,
) -> AppResult<Json<AdminTradingPairsResponse>> {
    Ok(Json(
        list_trading_pairs_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_trading_pair(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    Ok(Json(
        get_trading_pair_use_case(state.mysql.clone(), pair_id).await?,
    ))
}

async fn update_trading_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateTradingPairRequest>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_trading_pair_use_case(state.mysql.clone(), admin_id, pair_id, request).await?,
    ))
}

async fn update_trading_pair_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateTradingPairStatusRequest>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_trading_pair_status_use_case(state.mysql.clone(), admin_id, pair_id, request)
            .await?,
    ))
}

async fn create_trading_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateTradingPairRequest>,
) -> AppResult<Json<AdminTradingPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_trading_pair_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn list_new_coin_projects(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinProjectQuery>,
) -> AppResult<Json<NewCoinProjectsResponse>> {
    Ok(Json(
        list_new_coin_projects_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_convert_pairs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminConvertPairQuery>,
) -> AppResult<Json<ConvertPairsResponse>> {
    Ok(Json(
        list_convert_pairs_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_convert_pair(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
) -> AppResult<Json<ConvertPairResponse>> {
    Ok(Json(
        get_convert_pair_use_case(state.mysql.clone(), pair_id).await?,
    ))
}

async fn list_convert_orders(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminConvertOrdersQuery>,
) -> AppResult<Json<ConvertOrdersResponse>> {
    Ok(Json(
        list_convert_orders_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_convert_order(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> AppResult<Json<ConvertOrderResponse>> {
    Ok(Json(
        get_convert_order_use_case(state.mysql.clone(), order_id).await?,
    ))
}

async fn list_market_strategies(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarketStrategyQuery>,
) -> AppResult<Json<AdminMarketStrategiesResponse>> {
    Ok(Json(
        list_market_strategies_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_admin_audit_logs(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminAuditLogsQuery>,
) -> AppResult<Json<AdminAuditLogsResponse>> {
    Ok(Json(
        list_admin_audit_logs_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn create_market_strategy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateMarketStrategyRequest>,
) -> AppResult<Json<AdminMarketStrategyResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_market_strategy_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_market_strategy(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Json(request): Json<UpdateMarketStrategyRequest>,
) -> AppResult<Json<AdminMarketStrategyResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_market_strategy_use_case(state.mysql.clone(), admin_id, strategy_id, request)
            .await?,
    ))
}

async fn update_market_strategy_status(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(strategy_id): Path<u64>,
    Json(request): Json<UpdateMarketStrategyStatusRequest>,
) -> AppResult<Json<AdminMarketStrategyResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_market_strategy_status_use_case(state.mysql.clone(), admin_id, strategy_id, request)
            .await?,
    ))
}

async fn list_margin_liquidations(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminMarginLiquidationQuery>,
) -> AppResult<Json<AdminMarginLiquidationsResponse>> {
    Ok(Json(
        list_margin_liquidations_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn get_margin_liquidation(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(liquidation_id): Path<u64>,
) -> AppResult<Json<AdminMarginLiquidationResponse>> {
    Ok(Json(
        get_margin_liquidation_use_case(state.mysql.clone(), liquidation_id).await?,
    ))
}

async fn list_new_coin_subscriptions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Query(query): Query<AdminNewCoinScopedListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    Ok(Json(
        list_new_coin_subscriptions_for_project_use_case(state.mysql.clone(), project_id, query)
            .await?,
    ))
}

async fn list_all_new_coin_subscriptions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinFlatListQuery>,
) -> AppResult<Json<NewCoinSubscriptionsResponse>> {
    Ok(Json(
        list_new_coin_subscriptions_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_new_coin_distributions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Query(query): Query<AdminNewCoinScopedListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    Ok(Json(
        list_new_coin_distributions_for_project_use_case(state.mysql.clone(), project_id, query)
            .await?,
    ))
}

async fn list_all_new_coin_distributions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinFlatListQuery>,
) -> AppResult<Json<NewCoinDistributionsResponse>> {
    Ok(Json(
        list_new_coin_distributions_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_new_coin_purchases(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinPurchaseQuery>,
) -> AppResult<Json<NewCoinPurchasesResponse>> {
    Ok(Json(
        list_new_coin_purchases_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_new_coin_lock_positions(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinLockPositionQuery>,
) -> AppResult<Json<NewCoinLockPositionsResponse>> {
    Ok(Json(
        list_new_coin_lock_positions_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn list_new_coin_unlocks(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<AdminNewCoinUnlockQuery>,
) -> AppResult<Json<NewCoinUnlocksResponse>> {
    Ok(Json(
        list_new_coin_unlocks_use_case(state.mysql.clone(), query).await?,
    ))
}

async fn create_new_coin_project(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateNewCoinProjectRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_new_coin_project_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_new_coin_lifecycle(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinLifecycleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_lifecycle_use_case(state.mysql.clone(), admin_id, project_id, request)
            .await?,
    ))
}

async fn update_new_coin_unlock_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinUnlockRuleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_unlock_rule_use_case(state.mysql.clone(), admin_id, project_id, request)
            .await?,
    ))
}

async fn update_new_coin_unlock_fee_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinUnlockFeeRuleRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_unlock_fee_rule_use_case(
            state.mysql.clone(),
            admin_id,
            project_id,
            request,
        )
        .await?,
    ))
}

async fn update_new_coin_post_listing_purchase(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<UpdateNewCoinPostListingPurchaseRequest>,
) -> AppResult<Json<NewCoinProjectResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_new_coin_post_listing_purchase_use_case(
            state.mysql.clone(),
            admin_id,
            project_id,
            request,
        )
        .await?,
    ))
}

async fn distribute_new_coin(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(project_id): Path<u64>,
    Json(request): Json<DistributeNewCoinRequest>,
) -> AppResult<Json<NewCoinDistributionResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        distribute_new_coin_use_case(state.mysql.clone(), admin_id, project_id, request).await?,
    ))
}

async fn upsert_new_coin_convert_rule(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<UpsertNewCoinConvertRuleRequest>,
) -> AppResult<Json<NewCoinConvertRuleResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        upsert_new_coin_convert_rule_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn create_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Json(request): Json<CreateConvertPairRequest>,
) -> AppResult<Json<ConvertPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        create_convert_pair_use_case(state.mysql.clone(), admin_id, request).await?,
    ))
}

async fn update_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<UpdateConvertPairRequest>,
) -> AppResult<Json<ConvertPairResponse>> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    Ok(Json(
        update_convert_pair_use_case(state.mysql.clone(), admin_id, pair_id, request).await?,
    ))
}

async fn delete_convert_pair(
    AdminAuth(claims): AdminAuth,
    State(state): State<AppState>,
    Path(pair_id): Path<u64>,
    Json(request): Json<DeleteConvertPairRequest>,
) -> AppResult<StatusCode> {
    let admin_id = admin_id_from_subject(&claims.sub)?;
    delete_convert_pair_use_case(state.mysql.clone(), admin_id, pair_id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_admin_routes_tests.rs"]
mod tests;
