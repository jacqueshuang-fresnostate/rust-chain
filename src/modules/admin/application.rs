//! admin bounded context application layer.
//!
//! 应用层：编排用例、事务边界和跨仓储协作。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use crate::{
    architecture::ApplicationLayer,
    error::{AppError, AppResult},
    infra::{
        email::{EmailMessage, EmailSender, SmtpEmailConfig},
        secrets::{encrypt_secret_field, mask_secret},
    },
    modules::admin::{
        infrastructure::{
            AdminAgentCommissionListFilter, AdminAgentCommissionRuleListFilter,
            AdminAgentCommissionRuleWrite, AdminAgentListFilter, AdminAssetInsert,
            AdminAssetListFilter, AdminAssetUpdate, AdminAuditLogEntry, AdminAuditLogListFilter,
            AdminConvertOrderListFilter, AdminConvertPairInsert, AdminConvertPairUpdate,
            AdminCountryInsert, AdminCountryListFilter, AdminCountryUpdate,
            AdminDepositAddressPoolListFilter, AdminDepositAddressPoolWrite,
            AdminDepositNetworkConfigListFilter, AdminDepositNetworkConfigWrite,
            AdminMarginLiquidationListFilter, AdminMarketStrategyInsert,
            AdminMarketStrategyListFilter, AdminMarketStrategyUpdate, AdminNewCoinConvertRuleWrite,
            AdminNewCoinFlatListFilter, AdminNewCoinLockPositionListFilter,
            AdminNewCoinProjectInsert, AdminNewCoinUnlockFeeRuleUpdate,
            AdminNewCoinUnlockListFilter, AdminNewCoinUnlockRuleUpdate, AdminNewsInsert,
            AdminNewsListFilter, AdminNewsStatusUpdate, AdminNewsUpdate, AdminRiskEventListFilter,
            AdminRiskRuleListFilter, AdminTradingPairInsert, AdminTradingPairListFilter,
            AdminTradingPairUpdate, AdminUserInsert, AdminUserListFilter,
            AdminWalletAccountListFilter, AdminWalletLedgerListFilter,
            activate_admin_new_coin_post_listing_pair_in_tx,
            admin_new_coin_idempotency_key_exists_in_tx, admin_smtp_config_name_exists_except,
            admin_smtp_email_config, admin_upload_config_response,
            apply_admin_new_coin_distribution_allocation_in_tx,
            apply_admin_new_coin_subscription_distribution_in_tx,
            count_admin_dashboard_actions_24h, create_user_invite_code_in_tx,
            create_wallet_accounts_for_asset_in_tx, credit_admin_wallet_available_in_tx,
            delete_admin_asset_in_tx, delete_admin_convert_pair_in_tx,
            delete_zero_balance_wallet_accounts_for_asset_in_tx,
            disable_admin_new_coin_post_listing_purchase_in_tx,
            enable_admin_new_coin_post_listing_purchase_in_tx,
            ensure_admin_new_coin_post_listing_pair_in_tx, ensure_admin_user_exists_in_tx,
            ensure_agent_exists_in_tx, ensure_asset_has_no_references_in_tx,
            ensure_asset_symbols_exist, ensure_convert_pair_has_no_references_in_tx,
            ensure_market_strategy_pair_in_tx, ensure_trading_pair_asset_in_tx,
            finalize_admin_agent_hierarchy_in_tx, insert_admin_agent_in_tx,
            insert_admin_asset_in_tx, insert_admin_audit_log_entry_in_tx,
            insert_admin_convert_pair_in_tx, insert_admin_country_in_tx,
            insert_admin_deposit_network_config_in_tx, insert_admin_market_strategy_in_tx,
            insert_admin_new_coin_convert_rule_in_tx, insert_admin_new_coin_distribution_in_tx,
            insert_admin_new_coin_lifecycle_event_in_tx, insert_admin_new_coin_project_in_tx,
            insert_admin_news_item_in_tx, insert_admin_smtp_config_in_tx,
            insert_admin_trading_pair_in_tx, insert_admin_upload_object, insert_admin_user_in_tx,
            insert_agent_admin_user_in_tx, insert_agent_commission_rule_in_tx,
            insert_deposit_address_pool_in_tx, insert_market_strategy_event_in_tx,
            insert_market_strategy_run_in_tx, insert_market_strategy_version_in_tx,
            insert_risk_rule_in_tx,
            list_admin_agent_commission_rules as list_admin_agent_commission_rules_from_store,
            list_admin_agent_commissions as list_admin_agent_commissions_from_store,
            list_admin_agent_users as list_admin_agent_users_from_store,
            list_admin_agents as list_admin_agents_from_store,
            list_admin_assets as list_admin_assets_from_store,
            list_admin_audit_logs as list_admin_audit_logs_from_store,
            list_admin_convert_orders as list_admin_convert_orders_from_store,
            list_admin_convert_pairs as list_admin_convert_pairs_from_store,
            list_admin_countries as list_admin_countries_from_store,
            list_admin_dashboard_latest_actions,
            list_admin_deposit_address_pool as list_admin_deposit_address_pool_from_store,
            list_admin_deposit_network_configs as list_admin_deposit_network_configs_from_store,
            list_admin_margin_liquidations as list_admin_margin_liquidations_from_store,
            list_admin_market_source_credentials as list_admin_market_source_credentials_from_store,
            list_admin_market_strategies as list_admin_market_strategies_from_store,
            list_admin_new_coin_distributions as list_admin_new_coin_distributions_from_store,
            list_admin_new_coin_lock_positions as list_admin_new_coin_lock_positions_from_store,
            list_admin_new_coin_projects as list_admin_new_coin_projects_from_store,
            list_admin_new_coin_purchases as list_admin_new_coin_purchases_from_store,
            list_admin_new_coin_subscriptions as list_admin_new_coin_subscriptions_from_store,
            list_admin_new_coin_unlocks as list_admin_new_coin_unlocks_from_store,
            list_admin_news_items as list_admin_news_items_from_store,
            list_admin_risk_events as list_admin_risk_events_from_store,
            list_admin_risk_rules as list_admin_risk_rules_from_store,
            list_admin_smtp_configs as list_admin_smtp_configs_from_store,
            list_admin_trading_pairs as list_admin_trading_pairs_from_store,
            list_admin_users as list_admin_users_from_store,
            list_admin_wallet_accounts as list_admin_wallet_accounts_from_store,
            list_admin_wallet_ledger as list_admin_wallet_ledger_from_store,
            load_active_asset_symbol_in_tx, load_admin_agent as load_admin_agent_from_store,
            load_admin_agent_in_tx, load_admin_asset as load_admin_asset_from_store,
            load_admin_asset_in_tx,
            load_admin_convert_order as load_admin_convert_order_from_store,
            load_admin_convert_pair as load_admin_convert_pair_from_store,
            load_admin_convert_pair_in_tx, load_admin_country_in_tx,
            load_admin_dashboard_market_counts, load_admin_dashboard_products_summary,
            load_admin_dashboard_risk_summary, load_admin_dashboard_trading_summary,
            load_admin_dashboard_users_summary, load_admin_dashboard_wallet_summary,
            load_admin_margin_liquidation as load_admin_margin_liquidation_from_store,
            load_admin_market_feed_config as load_admin_market_feed_config_from_store,
            load_admin_market_feed_config_in_tx, load_admin_market_source_credential_in_tx,
            load_admin_market_strategy_in_tx, load_admin_new_coin_convert_rule_in_tx,
            load_admin_new_coin_distribution_in_tx, load_admin_new_coin_project_in_tx,
            load_admin_news_item as load_admin_news_item_from_store, load_admin_news_item_in_tx,
            load_admin_smtp_config as load_admin_smtp_config_from_store,
            load_admin_smtp_config_by_id, load_admin_smtp_config_by_id_in_tx,
            load_admin_smtp_config_by_name_in_tx, load_admin_smtp_config_for_delivery,
            load_admin_smtp_delivery_settings,
            load_admin_trading_pair as load_admin_trading_pair_from_store,
            load_admin_trading_pair_in_tx,
            load_admin_upload_config as load_admin_upload_config_from_store,
            load_admin_upload_config_in_tx, load_admin_user as load_admin_user_from_store,
            load_admin_user_in_tx, load_admin_user_two_factor_in_tx, load_agent_commission_in_tx,
            load_agent_commission_payout_target_in_tx, load_agent_commission_rule_in_tx,
            load_deposit_address_pool, load_deposit_address_pool_in_tx,
            load_deposit_network_config_by_network, load_deposit_network_config_in_tx,
            load_enabled_admin_market_feed_config_for_bootstrap as load_enabled_admin_market_feed_config_for_bootstrap_from_store,
            load_enabled_admin_market_source_credential_secrets,
            load_enabled_admin_smtp_email_config, load_enabled_admin_upload_config,
            load_risk_rule_in_tx, load_user_referral_in_tx, lock_active_agent_hierarchy_node_in_tx,
            lock_admin_agent_in_tx, lock_admin_asset_in_tx, lock_admin_convert_pair_in_tx,
            lock_admin_country_in_tx, lock_admin_market_feed_config_in_tx,
            lock_admin_market_source_credential_in_tx, lock_admin_market_strategy_in_tx,
            lock_admin_new_coin_convert_rule_in_tx, lock_admin_new_coin_project_in_tx,
            lock_admin_news_item_in_tx, lock_admin_smtp_config_by_id_in_tx,
            lock_admin_smtp_config_by_name_in_tx, lock_admin_smtp_delivery_settings_in_tx,
            lock_admin_trading_pair_in_tx, lock_admin_upload_config_in_tx,
            lock_agent_commission_in_tx, lock_agent_commission_rule_in_tx,
            lock_deposit_address_pool_in_tx, lock_deposit_network_config_in_tx,
            lock_or_create_admin_wallet_row_in_tx, lock_risk_rule_in_tx, lock_user_referral_in_tx,
            mark_admin_market_feed_reload_failed, mark_admin_market_feed_reload_skipped,
            mark_admin_market_feed_reload_success, migrate_user_referral_descendants_in_tx,
            next_market_strategy_version_in_tx, reclaim_deposit_address_pool_in_tx,
            reset_admin_user_two_factor_in_tx, save_admin_security_policy_in_tx,
            update_admin_agent_status_in_tx, update_admin_asset_in_tx,
            update_admin_convert_pair_in_tx, update_admin_country_in_tx,
            update_admin_country_status_in_tx, update_admin_deposit_network_config_in_tx,
            update_admin_market_strategy_in_tx, update_admin_new_coin_convert_rule_in_tx,
            update_admin_new_coin_project_lifecycle_in_tx,
            update_admin_new_coin_project_unlock_fee_rule_in_tx,
            update_admin_new_coin_project_unlock_rule_in_tx, update_admin_news_item_in_tx,
            update_admin_news_status_in_tx, update_admin_smtp_config_in_tx,
            update_admin_trading_pair_in_tx, update_admin_trading_pair_status_in_tx,
            update_agent_admin_users_status_in_tx, update_agent_commission_rule_in_tx,
            update_agent_commission_status_in_tx, update_deposit_address_pool_in_tx,
            update_market_strategy_run_checkpoint_in_tx, update_market_strategy_run_status_in_tx,
            update_market_strategy_status_in_tx, update_risk_rule_status_in_tx,
            upload_admin_file_to_storage, upsert_admin_market_feed_config_in_tx,
            upsert_admin_market_source_credential_in_tx, upsert_admin_smtp_delivery_settings_in_tx,
            upsert_admin_upload_config_in_tx, upsert_default_admin_smtp_config_in_tx,
            upsert_user_agent_referral_in_tx,
        },
        presentation::{
            AdminAgentCommissionQuery, AdminAgentCommissionResponse, AdminAgentCommissionRuleQuery,
            AdminAgentCommissionRuleResponse, AdminAgentCommissionRulesResponse,
            AdminAgentCommissionsResponse, AdminAgentQuery, AdminAgentResponse,
            AdminAgentUsersResponse, AdminAgentsResponse, AdminAssetQuery, AdminAssetResponse,
            AdminAssetsResponse, AdminAuditLogsQuery, AdminAuditLogsResponse,
            AdminConvertOrdersQuery, AdminConvertPairQuery, AdminCountriesQuery,
            AdminCountriesResponse, AdminCountryResponse, AdminDashboardAuditSummary,
            AdminDashboardMarketSummary, AdminDashboardResponse,
            AdminDepositAddressPoolBatchResponse, AdminDepositAddressPoolQuery,
            AdminDepositAddressPoolResponse, AdminDepositAddressPoolResponseList,
            AdminDepositNetworkConfigQuery, AdminDepositNetworkConfigResponse,
            AdminDepositNetworkConfigResponseList, AdminKycSubmissionQuery,
            AdminMarginLiquidationQuery, AdminMarginLiquidationResponse,
            AdminMarginLiquidationsResponse, AdminMarketStrategiesResponse,
            AdminMarketStrategyQuery, AdminMarketStrategyResponse, AdminNewCoinFlatListQuery,
            AdminNewCoinLockPositionQuery, AdminNewCoinProjectQuery, AdminNewCoinPurchaseQuery,
            AdminNewCoinScopedListQuery, AdminNewCoinUnlockQuery, AdminNewsItemResponse,
            AdminNewsItemsResponse, AdminNewsQuery, AdminRiskEventQuery, AdminRiskRuleQuery,
            AdminTradingPairQuery, AdminTradingPairResponse, AdminTradingPairsResponse,
            AdminUserQuery, AdminUserRechargeRequest, AdminUserRechargeResponse,
            AdminUserReferralResponse, AdminUserResponse, AdminUserTwoFactorResetResponse,
            AdminUsersResponse, AdminWalletAccountQuery, AdminWalletAccountsResponse,
            AdminWalletLedgerQuery, AdminWalletLedgerResponseList, AssignUserAgentRequest,
            ConvertOrderResponse, ConvertOrdersResponse, ConvertPairResponse, ConvertPairsResponse,
            CreateAdminCountryRequest, CreateAdminNewsItemRequest, CreateAdminUserRequest,
            CreateAgentCommissionRuleRequest, CreateAgentRequest, CreateAssetRequest,
            CreateConvertPairRequest, CreateDepositAddressPoolBatchRequest,
            CreateDepositAddressPoolRequest, CreateDepositNetworkConfigRequest,
            CreateMarketStrategyRequest, CreateNewCoinProjectRequest, CreateRiskRuleRequest,
            CreateTradingPairRequest, DeleteAssetRequest, DeleteConvertPairRequest,
            DistributeNewCoinRequest, MarketFeedConfigResponse, MarketFeedStatusResponse,
            MarketSourceCredentialResponse, MarketSourceCredentialSecret,
            MarketSourceCredentialsResponse, NewCoinConvertRuleResponse,
            NewCoinDistributionResponse, NewCoinDistributionsResponse,
            NewCoinLockPositionsResponse, NewCoinProjectResponse, NewCoinProjectsResponse,
            NewCoinPurchasesResponse, NewCoinSubscriptionsResponse, NewCoinUnlocksResponse,
            ReclaimDepositAddressPoolRequest, ReloadMarketFeedRequest, ReloadMarketFeedResponse,
            ResetUserTwoFactorRequest, RiskEventsResponse, RiskRuleResponse, RiskRulesResponse,
            SaveMarketFeedConfigRequest, SaveSmtpConfigRequest, SaveSmtpDeliverySettingsRequest,
            SaveUploadConfigRequest, SendSmtpTestRequest, SendSmtpTestResponse,
            SmtpConfigListResponse, SmtpConfigResponse, SmtpDeliverySettingsResponse,
            UpdateAdminCountryRequest, UpdateAdminCountryStatusRequest, UpdateAdminNewsItemRequest,
            UpdateAdminNewsStatusRequest, UpdateAgentCommissionRuleRequest,
            UpdateAgentCommissionStatusRequest, UpdateAgentStatusRequest, UpdateAssetRequest,
            UpdateConvertPairRequest, UpdateDepositAddressPoolRequest,
            UpdateDepositNetworkConfigRequest, UpdateMarketStrategyRequest,
            UpdateMarketStrategyStatusRequest, UpdateNewCoinLifecycleRequest,
            UpdateNewCoinPostListingPurchaseRequest, UpdateNewCoinUnlockFeeRuleRequest,
            UpdateNewCoinUnlockRuleRequest, UpdateRiskRuleStatusRequest,
            UpdateSecurityPolicyRequest, UpdateTradingPairRequest, UpdateTradingPairStatusRequest,
            UploadConfigResponse, UploadFileInput, UploadImageResponse,
            UpsertMarketSourceCredentialRequest, UpsertNewCoinConvertRuleRequest,
        },
        repository::{
            AdminAgentAdminUserWrite, AdminAgentWrite, AdminMarketFeedConfigWrite,
            AdminMarketSourceCredentialRecord, AdminMarketSourceCredentialWrite,
            AdminNewCoinLedgerWrite, AdminSmtpConfigRecord, AdminSmtpConfigWrite,
            AdminUploadConfigWrite, AdminUploadObjectWrite, RiskRuleWrite, UploadObjectOwner,
            UserAgentReferralWrite,
        },
        service::{
            DEFAULT_SMTP_CONFIG_NAME, DEFAULT_SMTP_CONFIG_PRIORITY,
            MARKET_SOURCE_AUTH_TYPE_API_KEY, SMTP_DELIVERY_SETTINGS_ID, SmtpValidatedConfig,
            admin_news_item_audit_json, agent_audit_json, agent_commission_audit_json,
            agent_commission_rule_audit_json, agent_password_hash, asset_audit_json,
            convert_pair_audit_json, country_config_audit_json, deposit_address_pool_audit_json,
            deposit_network_config_audit_json, ensure_deposit_asset_symbols_allowed_by_network,
            ensure_distribution_lifecycle, ensure_post_listing_purchase_lifecycle,
            hash_admin_user_password, lifecycle_status_value, lock_positions_for_distribution,
            market_feed_config_audit_json, market_feed_config_response,
            market_feed_reload_audit_json, market_feed_runtime_config_from_response,
            market_source_credential_audit_json, market_source_credential_response,
            market_source_credential_target_id, market_strategy_audit_json,
            market_strategy_config_json, market_strategy_run_status,
            market_strategy_update_config_json, new_coin_convert_rule_audit_json,
            new_coin_distribution_audit_json, new_coin_project_audit_json, normalize_asset_symbol,
            normalize_asset_withdraw_fee_tiers, normalize_deposit_address_batch_entries,
            normalize_deposit_asset_symbols, normalize_deposit_network,
            normalize_news_country_code, normalize_optional_news_country_code,
            normalize_trading_pair_symbol, parse_lifecycle_status_from_db,
            parse_lifecycle_status_from_request, recharge_audit_json, required_admin_audit_reason,
            required_smtp_audit_reason, required_upload_audit_reason,
            resolve_deposit_address_group_code, risk_rule_audit_json, safe_upload_filename,
            security_policy_audit_json, smtp_config_audit_json, smtp_config_response,
            smtp_delivery_settings_audit_json, smtp_delivery_settings_response,
            smtp_request_has_new_secret, trading_pair_audit_json, two_factor_audit_json,
            upload_config_audit_json, upload_config_secret_destination_unchanged, user_audit_json,
            user_referral_audit_json, validate_address_group_code, validate_admin_user_recharge,
            validate_agent_commission_rate, validate_agent_commission_rule_product_type,
            validate_agent_commission_rule_status, validate_agent_commission_status,
            validate_agent_status, validate_asset_fee_settings, validate_asset_name,
            validate_asset_status, validate_asset_type, validate_convert_pair_values,
            validate_country_code, validate_country_locale_config, validate_country_name,
            validate_country_remark, validate_country_status, validate_create_admin_user_request,
            validate_create_agent_request, validate_create_asset_request,
            validate_create_convert_pair, validate_create_market_strategy,
            validate_create_new_coin_project, validate_create_risk_rule,
            validate_create_trading_pair_request, validate_deposit_address,
            validate_deposit_address_assignable_status, validate_deposit_address_status,
            validate_deposit_network_config_status, validate_deposit_network_display_name,
            validate_distribute_new_coin, validate_market_feed_intervals,
            validate_market_feed_providers, validate_market_feed_reason,
            validate_market_feed_symbols, validate_market_source_auth_type,
            validate_market_strategy_status, validate_new_coin_convert_rule,
            validate_news_category, validate_news_content_document, validate_news_locale,
            validate_news_status, validate_news_title, validate_optional_image_url,
            validate_optional_length, validate_security_policy, validate_smtp_delivery_strategy,
            validate_smtp_email, validate_smtp_save_request, validate_trading_pair_market_type,
            validate_trading_pair_status, validate_update_asset_request,
            validate_update_market_strategy, validate_update_new_coin_post_listing_purchase,
            validate_update_new_coin_unlock_fee_rule, validate_update_new_coin_unlock_rule,
            validate_update_trading_pair_request, validate_upload_config, validate_user_status,
        },
    },
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use serde_json::json;
use sqlx::{MySql, Pool, Transaction};
use std::sync::Arc;
use uuid::Uuid;

use crate::modules::agent::domain::{agent_path, derive_agent_placement};
use crate::modules::kyc::{
    KycConfigResponse, KycSubmissionResponse, KycSubmissionsResponse, ListKycSubmissionsFilter,
    ReviewKycSubmissionRequest, SaveKycConfigRequest, kyc_config_audit_json,
    kyc_submission_audit_json, list_kyc_submissions as list_kyc_submissions_from_kyc,
    load_kyc_config as load_kyc_config_from_kyc,
    load_kyc_submission as load_kyc_submission_from_kyc,
    review_kyc_submission_in_tx as review_kyc_submission_in_tx_from_kyc,
    save_kyc_config_in_tx as save_kyc_config_in_tx_from_kyc,
};
use crate::modules::new_coin::LifecycleStatus;
use crate::modules::platform::{
    PlatformBrandResponse, SavePlatformBrandRequest,
    load_platform_brand as load_platform_brand_from_platform, platform_brand_audit_json,
    save_platform_brand_in_tx as save_platform_brand_in_tx_from_platform,
};
use crate::modules::security::{UserSecurityPolicy, load_security_policy};
use crate::{state::AppState, workers::market_feed::MarketFeedRuntimeStatus};

#[derive(Debug)]
pub struct AdminCountryUseCases;

impl ApplicationLayer for AdminCountryUseCases {}

pub(crate) async fn get_admin_dashboard(
    pool: Option<Pool<MySql>>,
    runtime: MarketFeedRuntimeStatus,
) -> AppResult<AdminDashboardResponse> {
    let pool = admin_mysql_pool(pool)?;
    let generated_at = Utc::now();
    let users = load_admin_dashboard_users_summary(&pool).await?;
    let wallet = load_admin_dashboard_wallet_summary(&pool).await?;
    let market_counts = load_admin_dashboard_market_counts(&pool).await?;
    let saved_feed_config = load_admin_market_feed_config_from_store(&pool)
        .await?
        .map(market_feed_config_response);
    let feed_runtime_status = runtime
        .last_reload_status
        .clone()
        .unwrap_or_else(|| "not_started".to_owned());
    let market = AdminDashboardMarketSummary {
        active_pairs: market_counts.active_pairs,
        disabled_pairs: market_counts.disabled_pairs,
        external_pairs: market_counts.external_pairs,
        strategy_pairs: market_counts.strategy_pairs,
        feed_runtime_status,
        feed_needs_reload: saved_feed_config
            .as_ref()
            .is_some_and(|config| config.needs_reload),
        feed_symbols: runtime.symbols,
        feed_providers: runtime.providers,
    };
    let trading = load_admin_dashboard_trading_summary(&pool).await?;
    let products = load_admin_dashboard_products_summary(&pool).await?;
    let risk = load_admin_dashboard_risk_summary(&pool).await?;
    let admin_actions_24h = count_admin_dashboard_actions_24h(&pool).await?;
    let latest_actions = list_admin_dashboard_latest_actions(&pool).await?;

    // Dashboard 是跨多个后台子域的只读聚合，应用层负责组装，避免路由层重新耦合 SQL 细节。
    Ok(AdminDashboardResponse {
        generated_at,
        users,
        wallet,
        market,
        trading,
        products,
        risk,
        audit: AdminDashboardAuditSummary {
            admin_actions_24h,
            latest_actions,
        },
    })
}

pub(crate) async fn list_admin_audit_logs(
    pool: Option<Pool<MySql>>,
    query: AdminAuditLogsQuery,
) -> AppResult<AdminAuditLogsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let logs = list_admin_audit_logs_from_store(
        &pool,
        AdminAuditLogListFilter {
            admin_id: query.admin_id,
            action: query.action.and_then(optional_string),
            target_type: query.target_type.and_then(optional_string),
            target_id: query.target_id.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminAuditLogsResponse { logs })
}

pub(crate) async fn list_admin_margin_liquidations(
    pool: Option<Pool<MySql>>,
    query: AdminMarginLiquidationQuery,
) -> AppResult<AdminMarginLiquidationsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let liquidations = list_admin_margin_liquidations_from_store(
        &pool,
        AdminMarginLiquidationListFilter {
            user_id: query.user_id,
            email: query.email.and_then(optional_string),
            pair_id: query.pair_id,
            position_id: query.position_id,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminMarginLiquidationsResponse { liquidations })
}

pub(crate) async fn get_admin_margin_liquidation(
    pool: Option<Pool<MySql>>,
    liquidation_id: u64,
) -> AppResult<AdminMarginLiquidationResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_margin_liquidation_from_store(&pool, liquidation_id).await
}

pub(crate) async fn get_admin_smtp_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<Option<SmtpConfigResponse>> {
    let pool = admin_mysql_pool(pool)?;
    Ok(load_admin_smtp_config_from_store(&pool)
        .await?
        .map(smtp_config_response))
}

pub(crate) async fn list_admin_smtp_configs(
    pool: Option<Pool<MySql>>,
) -> AppResult<SmtpConfigListResponse> {
    let pool = admin_mysql_pool(pool)?;
    let configs = list_admin_smtp_configs_from_store(&pool)
        .await?
        .into_iter()
        .map(smtp_config_response)
        .collect();
    let delivery_settings =
        smtp_delivery_settings_response(load_admin_smtp_delivery_settings(&pool).await?);
    Ok(SmtpConfigListResponse {
        configs,
        delivery_settings,
    })
}

pub(crate) async fn create_admin_smtp_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    let reason = required_smtp_audit_reason(request.reason.clone())?;
    let config = validate_smtp_save_request(&request, None, Some(DEFAULT_SMTP_CONFIG_PRIORITY))?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    if load_admin_smtp_config_by_name_in_tx(&mut tx, &config.name)
        .await?
        .is_some()
    {
        return Err(AppError::Validation(
            "smtp config name already exists".to_owned(),
        ));
    }
    let (username_ciphertext, password_ciphertext, username_mask) =
        prepare_smtp_secret_fields(&request, None, key)?;
    let config_id = insert_admin_smtp_config_in_tx(
        &mut tx,
        smtp_config_write(
            config,
            username_ciphertext,
            password_ciphertext,
            username_mask,
            admin_id,
        ),
    )
    .await?;
    let after = load_admin_smtp_config_by_id_in_tx(&mut tx, config_id)
        .await?
        .ok_or(AppError::NotFound)?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_config.create",
            target_type: "smtp_config",
            target_id: after.id,
            before_json: None,
            after_json: Some(smtp_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(smtp_config_response(after))
}

pub(crate) async fn update_admin_smtp_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    config_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    let reason = required_smtp_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_smtp_config_by_id_in_tx(&mut tx, config_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let config = validate_smtp_save_request(&request, Some(&before.name), Some(before.priority))?;
    if config.name != before.name
        && admin_smtp_config_name_exists_except(&mut tx, &config.name, config_id).await?
    {
        return Err(AppError::Validation(
            "smtp config name already exists".to_owned(),
        ));
    }
    let (username_ciphertext, password_ciphertext, username_mask) =
        prepare_smtp_secret_fields(&request, Some(&before), key)?;
    update_admin_smtp_config_in_tx(
        &mut tx,
        config_id,
        smtp_config_write(
            config,
            username_ciphertext,
            password_ciphertext,
            username_mask,
            admin_id,
        ),
    )
    .await?;
    let after = load_admin_smtp_config_by_id_in_tx(&mut tx, config_id)
        .await?
        .ok_or(AppError::NotFound)?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_config.update",
            target_type: "smtp_config",
            target_id: after.id,
            before_json: Some(smtp_config_audit_json(&before)),
            after_json: Some(smtp_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(smtp_config_response(after))
}

pub(crate) async fn save_admin_smtp_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveSmtpConfigRequest,
) -> AppResult<SmtpConfigResponse> {
    let reason = required_smtp_audit_reason(request.reason.clone())?;
    let config = validate_smtp_save_request(
        &request,
        Some(DEFAULT_SMTP_CONFIG_NAME),
        Some(DEFAULT_SMTP_CONFIG_PRIORITY),
    )?;
    let config_name = config.name.clone();
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_smtp_config_by_name_in_tx(&mut tx, DEFAULT_SMTP_CONFIG_NAME).await?;
    let (username_ciphertext, password_ciphertext, username_mask) =
        prepare_smtp_secret_fields(&request, before.as_ref(), key)?;

    // SMTP 默认配置和审计同事务提交，避免发信凭证已变化但后台没有操作者记录。
    upsert_default_admin_smtp_config_in_tx(
        &mut tx,
        smtp_config_write(
            config,
            username_ciphertext,
            password_ciphertext,
            username_mask,
            admin_id,
        ),
    )
    .await?;
    let after = load_admin_smtp_config_by_name_in_tx(&mut tx, &config_name)
        .await?
        .ok_or(AppError::NotFound)?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_config.save",
            target_type: "smtp_config",
            target_id: after.id,
            before_json: before.as_ref().map(smtp_config_audit_json),
            after_json: Some(smtp_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(smtp_config_response(after))
}

pub(crate) async fn save_admin_smtp_delivery_settings(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: SaveSmtpDeliverySettingsRequest,
) -> AppResult<SmtpDeliverySettingsResponse> {
    let reason = required_smtp_audit_reason(request.reason)?;
    let strategy = validate_smtp_delivery_strategy(&request.strategy)?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_smtp_delivery_settings_in_tx(&mut tx).await?;
    upsert_admin_smtp_delivery_settings_in_tx(&mut tx, &strategy, admin_id).await?;
    let after = lock_admin_smtp_delivery_settings_in_tx(&mut tx).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_delivery_settings.save",
            target_type: "smtp_delivery_settings",
            target_id: u64::from(SMTP_DELIVERY_SETTINGS_ID),
            before_json: Some(smtp_delivery_settings_audit_json(&before)),
            after_json: Some(smtp_delivery_settings_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(smtp_delivery_settings_response(after))
}

pub(crate) async fn send_admin_smtp_test(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    sender: Option<Arc<dyn EmailSender>>,
    request: SendSmtpTestRequest,
) -> AppResult<SendSmtpTestResponse> {
    let pool = admin_mysql_pool(pool)?;
    let sender =
        sender.ok_or_else(|| AppError::Internal("email sender is not configured".to_owned()))?;
    send_admin_smtp_test_with_sender(&pool, admin_id, key, sender.as_ref(), request).await
}

pub(crate) async fn send_admin_smtp_test_with_sender(
    pool: &Pool<MySql>,
    admin_id: u64,
    key: Option<&str>,
    sender: &dyn EmailSender,
    request: SendSmtpTestRequest,
) -> AppResult<SendSmtpTestResponse> {
    let reason = required_smtp_audit_reason(request.reason)?;
    let recipient = validate_smtp_email(&request.recipient, "recipient")?;
    let record = match request.config_id {
        Some(config_id) => load_admin_smtp_config_by_id(pool, config_id)
            .await?
            .ok_or(AppError::NotFound)?,
        None => load_admin_smtp_config_for_delivery(pool)
            .await?
            .ok_or(AppError::NotFound)?,
    };
    let smtp = admin_smtp_email_config(&record, key)?;
    let mut tx = pool.begin().await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "smtp_config.test",
            target_type: "smtp_config",
            target_id: record.id,
            before_json: Some(smtp_config_audit_json(&record)),
            after_json: Some(json!({
                "status": "attempted",
                "recipient": recipient.clone(),
                "config": smtp_config_audit_json(&record),
            })),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;

    sender
        .send(
            smtp,
            EmailMessage {
                to: recipient.clone(),
                subject: "SMTP test".to_owned(),
                text_body: "SMTP configuration test email.".to_owned(),
                html_body: None,
            },
        )
        .await?;

    Ok(SendSmtpTestResponse {
        sent: true,
        recipient,
        config_id: record.id,
        config_name: record.name,
    })
}

pub(crate) async fn load_enabled_admin_smtp_config(
    pool: &Pool<MySql>,
    key: Option<&str>,
) -> AppResult<Option<SmtpEmailConfig>> {
    load_enabled_admin_smtp_email_config(pool, key).await
}

pub async fn load_enabled_admin_market_feed_config(
    pool: &Pool<MySql>,
) -> AppResult<Option<MarketFeedConfigResponse>> {
    Ok(
        load_enabled_admin_market_feed_config_for_bootstrap_from_store(pool)
            .await?
            .map(market_feed_config_response),
    )
}

pub(crate) async fn get_admin_upload_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<Option<UploadConfigResponse>> {
    let pool = admin_mysql_pool(pool)?;
    Ok(load_admin_upload_config_from_store(&pool)
        .await?
        .map(admin_upload_config_response))
}

pub(crate) async fn save_admin_upload_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    request: SaveUploadConfigRequest,
) -> AppResult<UploadConfigResponse> {
    let reason = required_upload_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_upload_config_in_tx(&mut tx).await?;
    let config = validate_upload_config(&request)?;
    let existing_same_provider = before
        .as_ref()
        .filter(|record| record.provider == config.provider.code())
        .filter(|record| upload_config_secret_destination_unchanged(record, &config));

    let (bearer_token_ciphertext, bearer_token_mask) = if config.provider.uses_bearer() {
        let existing_ciphertext =
            existing_same_provider.and_then(|record| record.bearer_token_ciphertext.clone());
        let existing_mask =
            existing_same_provider.and_then(|record| record.bearer_token_mask.clone());
        let ciphertext = encrypt_optional_upload_secret(
            key,
            request.bearer_token.as_deref(),
            existing_ciphertext,
        )?;
        let mask = request
            .bearer_token
            .as_deref()
            .and_then(optional_str)
            .map(mask_secret)
            .or(existing_mask);
        if config.enabled && ciphertext.is_none() {
            return Err(AppError::Validation(
                "image bed bearer token is required".to_owned(),
            ));
        }
        (ciphertext, mask)
    } else {
        (None, None)
    };

    let (access_key_ciphertext, access_key_mask, secret_key_ciphertext) =
        if config.provider.uses_access_secret() {
            let existing_access_ciphertext =
                existing_same_provider.and_then(|record| record.access_key_ciphertext.clone());
            let existing_secret_ciphertext =
                existing_same_provider.and_then(|record| record.secret_key_ciphertext.clone());
            let existing_access_mask =
                existing_same_provider.and_then(|record| record.access_key_mask.clone());
            let access_ciphertext = encrypt_optional_upload_secret(
                key,
                request.access_key.as_deref(),
                existing_access_ciphertext,
            )?;
            let secret_ciphertext = encrypt_optional_upload_secret(
                key,
                request.secret_key.as_deref(),
                existing_secret_ciphertext,
            )?;
            let access_mask = request
                .access_key
                .as_deref()
                .and_then(optional_str)
                .map(mask_secret)
                .or(existing_access_mask);
            if config.enabled && (access_ciphertext.is_none() || secret_ciphertext.is_none()) {
                return Err(AppError::Validation(
                    "upload access key and secret key are required".to_owned(),
                ));
            }
            (access_ciphertext, access_mask, secret_ciphertext)
        } else {
            (None, None, None)
        };

    // 上传配置和审计必须同事务提交，避免存储凭证已变更但后台无法追踪操作者。
    upsert_admin_upload_config_in_tx(
        &mut tx,
        AdminUploadConfigWrite {
            provider: config.provider.code().to_owned(),
            endpoint: config.endpoint,
            file_field: config.file_field,
            bearer_token_ciphertext,
            bearer_token_mask,
            access_key_ciphertext,
            access_key_mask,
            secret_key_ciphertext,
            bucket: config.bucket,
            region: config.region,
            public_base_url: config.public_base_url,
            local_root: config.local_root,
            key_prefix: config.key_prefix,
            max_file_size_bytes: config.max_file_size_bytes,
            allowed_mime_types: config.allowed_mime_types,
            enabled: config.enabled,
            updated_by: admin_id,
        },
    )
    .await?;
    let after = load_admin_upload_config_in_tx(&mut tx).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "upload_storage_config.save",
            target_type: "upload_storage_config",
            target_id: after.id,
            before_json: before.as_ref().map(upload_config_audit_json),
            after_json: Some(upload_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(admin_upload_config_response(after))
}

pub(crate) async fn upload_admin_image(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    key: Option<&str>,
    input: UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let pool = admin_mysql_pool(pool)?;
    upload_image_for_owner(&pool, UploadObjectOwner::Admin(admin_id), key, input).await
}

pub(crate) async fn upload_image_for_owner(
    pool: &Pool<MySql>,
    owner: UploadObjectOwner,
    key: Option<&str>,
    input: UploadFileInput,
) -> AppResult<UploadImageResponse> {
    let config = load_enabled_admin_upload_config(pool)
        .await?
        .ok_or_else(|| AppError::Validation("upload storage is not enabled".to_owned()))?;
    let response = upload_admin_file_to_storage(&config, key, &input).await?;
    let original_filename =
        safe_upload_filename(input.original_filename.as_deref(), &input.mime_type);
    insert_admin_upload_object(
        pool,
        AdminUploadObjectWrite {
            provider: response.provider.clone(),
            object_key: response.object_key.clone(),
            public_url: response.download_url.clone(),
            share_url: response.share_url.clone(),
            delete_url: response.delete_url.clone(),
            mime_type: response.mime_type.clone(),
            size_bytes: response.size_bytes,
            original_filename,
            owner,
        },
    )
    .await?;
    Ok(response)
}

pub(crate) async fn get_admin_market_feed_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<Option<MarketFeedConfigResponse>> {
    let pool = admin_mysql_pool(pool)?;
    Ok(load_admin_market_feed_config_from_store(&pool)
        .await?
        .map(market_feed_config_response))
}

pub(crate) async fn save_admin_market_feed_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: SaveMarketFeedConfigRequest,
) -> AppResult<MarketFeedConfigResponse> {
    validate_market_feed_reason(request.reason.as_deref())?;
    let symbols = validate_market_feed_symbols(&request.symbols, request.enabled)?;
    let intervals = validate_market_feed_intervals(&request.intervals)?;
    let providers = validate_market_feed_providers(&request.providers)?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_feed_config_in_tx(&mut tx).await?;
    let version = before
        .as_ref()
        .map(|config| config.version + 1)
        .unwrap_or(1);

    // 行情配置的订阅集合和版本号必须同事务更新，避免 reload 读取到半更新状态。
    upsert_admin_market_feed_config_in_tx(
        &mut tx,
        AdminMarketFeedConfigWrite {
            symbols,
            intervals,
            providers,
            enabled: request.enabled,
            version,
            updated_by: admin_id,
        },
    )
    .await?;
    let after = load_admin_market_feed_config_in_tx(&mut tx).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "market_feed_config.save",
            target_type: "market_feed_config",
            target_id: after.id,
            before_json: before.as_ref().map(market_feed_config_audit_json),
            after_json: Some(market_feed_config_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(market_feed_config_response(after))
}

pub(crate) async fn get_admin_market_feed_status(
    pool: Option<Pool<MySql>>,
    runtime: MarketFeedRuntimeStatus,
) -> AppResult<MarketFeedStatusResponse> {
    let pool = admin_mysql_pool(pool)?;
    let saved_config = load_admin_market_feed_config_from_store(&pool)
        .await?
        .map(market_feed_config_response);
    Ok(MarketFeedStatusResponse {
        saved_config,
        runtime,
    })
}

pub(crate) async fn list_admin_market_feed_credentials(
    pool: Option<Pool<MySql>>,
) -> AppResult<MarketSourceCredentialsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let credentials = list_admin_market_source_credentials_from_store(&pool)
        .await?
        .into_iter()
        .map(market_source_credential_response)
        .collect();
    Ok(MarketSourceCredentialsResponse { credentials })
}

pub(crate) async fn upsert_admin_market_feed_credential(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    provider: String,
    key: Option<&str>,
    request: UpsertMarketSourceCredentialRequest,
) -> AppResult<MarketSourceCredentialResponse> {
    validate_market_feed_reason(Some(&request.reason))?;
    let provider = crate::modules::market::adapters::MarketFeedProvider::from_code(&provider)?
        .code()
        .to_owned();
    let auth_type = validate_market_source_auth_type(&request.auth_type)?;
    let pool = admin_mysql_pool(pool)?;
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_source_credential_in_tx(&mut tx, &provider).await?;
    let (api_key_ciphertext, api_secret_ciphertext, passphrase_ciphertext, api_key_mask) =
        prepare_market_source_credential_secret_fields(&request, before.as_ref(), &auth_type, key)?;
    upsert_admin_market_source_credential_in_tx(
        &mut tx,
        AdminMarketSourceCredentialWrite {
            provider: provider.clone(),
            auth_type,
            api_key_ciphertext,
            api_secret_ciphertext,
            passphrase_ciphertext,
            api_key_mask,
            enabled: request.enabled,
            updated_by: admin_id,
        },
    )
    .await?;
    let after = load_admin_market_source_credential_in_tx(&mut tx, &provider).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "market_source_credential.upsert",
            target_type: "market_source_credential",
            target_id: market_source_credential_target_id(&after.provider),
            before_json: before.as_ref().map(market_source_credential_audit_json),
            after_json: Some(market_source_credential_audit_json(&after)),
            reason: Some(request.reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(market_source_credential_response(after))
}

pub(crate) async fn reload_admin_market_feed_config(
    state: AppState,
    admin_id: u64,
    request: ReloadMarketFeedRequest,
) -> AppResult<ReloadMarketFeedResponse> {
    let reason = optional_string(request.reason)
        .ok_or_else(|| AppError::Validation("operation reason is required".to_owned()))?;
    let pool = admin_mysql_pool(state.mysql.clone())?;
    let config = load_admin_market_feed_config_from_store(&pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let config_response = market_feed_config_response(config.clone());
    let supervisor = state
        .market_feed_supervisor
        .clone()
        .ok_or_else(|| AppError::Internal("market feed supervisor is not configured".to_owned()))?;

    if !config_response.enabled {
        supervisor.stop().await;
        let config = mark_admin_market_feed_reload_skipped(&pool, config_response.version).await?;
        let config = market_feed_config_response(config);
        let runtime = supervisor.status().await;
        insert_admin_market_feed_reload_audit(&pool, admin_id, &config, &runtime, reason).await?;
        return Ok(ReloadMarketFeedResponse { config, runtime });
    }

    let credentials = load_enabled_admin_market_source_credential_secrets(
        &pool,
        &config_response.providers,
        state.settings.exposed_credential_encryption_key(),
    )
    .await?;
    validate_loaded_market_feed_credentials(&config_response.providers, &credentials)?;
    drop(credentials);

    let runtime_config =
        match market_feed_runtime_config_from_response(&state.settings, &config_response) {
            Ok(runtime_config) => runtime_config,
            Err(error) => {
                let config =
                    mark_admin_market_feed_reload_failed(&pool, &error.to_string()).await?;
                let config = market_feed_config_response(config);
                let runtime = supervisor.record_failure(error.to_string()).await;
                insert_admin_market_feed_reload_audit(&pool, admin_id, &config, &runtime, reason)
                    .await?;
                return Err(error);
            }
        };

    let runtime = match supervisor
        .reload(state.clone(), runtime_config, config_response.version)
        .await
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let config = mark_admin_market_feed_reload_failed(&pool, &error.to_string()).await?;
            let config = market_feed_config_response(config);
            let runtime = supervisor.record_failure(error.to_string()).await;
            insert_admin_market_feed_reload_audit(&pool, admin_id, &config, &runtime, reason)
                .await?;
            return Err(error);
        }
    };
    let config = mark_admin_market_feed_reload_success(&pool, config_response.version).await?;
    let config = market_feed_config_response(config);
    insert_admin_market_feed_reload_audit(&pool, admin_id, &config, &runtime, reason).await?;
    Ok(ReloadMarketFeedResponse { config, runtime })
}

pub(crate) async fn list_admin_countries(
    pool: &Pool<MySql>,
    query: AdminCountriesQuery,
) -> AppResult<AdminCountriesResponse> {
    let country_code = query
        .country_code
        .and_then(optional_string)
        .map(|value| validate_country_code(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_country_status(&value))
        .transpose()?;
    let countries = list_admin_countries_from_store(
        pool,
        AdminCountryListFilter {
            country_code,
            status,
            registration_enabled: query.registration_enabled,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminCountriesResponse { countries })
}

pub(crate) async fn create_admin_country(
    pool: &Pool<MySql>,
    admin_id: u64,
    request: CreateAdminCountryRequest,
) -> AppResult<AdminCountryResponse> {
    let country_code = validate_country_code(&request.country_code)?;
    let country_name = validate_country_name(&request.country_name)?;
    let remark = validate_country_remark(&request.remark)?;
    let (default_locale, supported_locales) =
        validate_country_locale_config(&request.default_locale, request.supported_locales)?;
    let status = request
        .status
        .as_deref()
        .map(validate_country_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let sort_order = request.sort_order.unwrap_or(0);

    // 国家配置和后台审计必须同事务提交，避免配置已生效但审计日志缺失。
    let mut tx = pool.begin().await?;
    let country_id = insert_admin_country_in_tx(
        &mut tx,
        AdminCountryInsert {
            country_code,
            country_name,
            remark,
            default_locale,
            supported_locales,
            registration_enabled: request.registration_enabled,
            status,
            sort_order,
        },
    )
    .await?;
    let country = load_admin_country_in_tx(&mut tx, country_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "country_config.create",
            target_type: "country_config",
            target_id: country.id,
            before_json: None,
            after_json: Some(country_config_audit_json(&country)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(country)
}

pub(crate) async fn update_admin_country(
    pool: &Pool<MySql>,
    admin_id: u64,
    country_id: u64,
    request: UpdateAdminCountryRequest,
) -> AppResult<AdminCountryResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let country_name = validate_country_name(&request.country_name)?;
    let remark = validate_country_remark(&request.remark)?;
    let (default_locale, supported_locales) =
        validate_country_locale_config(&request.default_locale, request.supported_locales)?;

    // 先锁定旧值再更新，确保审计 before/after 与本次写入完全对应。
    let mut tx = pool.begin().await?;
    let before = lock_admin_country_in_tx(&mut tx, country_id).await?;
    let sort_order = request.sort_order.unwrap_or(before.sort_order);
    update_admin_country_in_tx(
        &mut tx,
        country_id,
        AdminCountryUpdate {
            country_name,
            remark,
            default_locale,
            supported_locales,
            registration_enabled: request.registration_enabled,
            sort_order,
        },
    )
    .await?;
    let after = load_admin_country_in_tx(&mut tx, country_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "country_config.update",
            target_type: "country_config",
            target_id: after.id,
            before_json: Some(country_config_audit_json(&before)),
            after_json: Some(country_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_country_status(
    pool: &Pool<MySql>,
    admin_id: u64,
    country_id: u64,
    request: UpdateAdminCountryStatusRequest,
) -> AppResult<AdminCountryResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let status = validate_country_status(&request.status)?;

    // 状态切换也写审计，后台可追踪每一次启用/禁用操作。
    let mut tx = pool.begin().await?;
    let before = lock_admin_country_in_tx(&mut tx, country_id).await?;
    update_admin_country_status_in_tx(&mut tx, country_id, &status).await?;
    let after = load_admin_country_in_tx(&mut tx, country_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "country_config.status.update",
            target_type: "country_config",
            target_id: after.id,
            before_json: Some(country_config_audit_json(&before)),
            after_json: Some(country_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_news_items(
    pool: Option<Pool<MySql>>,
    query: AdminNewsQuery,
) -> AppResult<AdminNewsItemsResponse> {
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_news_status(&value))
        .transpose()?;
    let category = query
        .category
        .and_then(optional_string)
        .map(|value| validate_news_category(&value))
        .transpose()?;
    let country_code = query
        .country_code
        .and_then(optional_string)
        .map(|value| normalize_news_country_code(&value))
        .transpose()?;
    let locale = query
        .locale
        .and_then(optional_string)
        .map(|value| validate_news_locale(&value))
        .transpose()?;
    let keyword = query.q.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let news = list_admin_news_items_from_store(
        &pool,
        AdminNewsListFilter {
            status,
            category,
            country_code,
            locale,
            keyword,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminNewsItemsResponse { news })
}

pub(crate) async fn get_admin_news_item(
    pool: Option<Pool<MySql>>,
    news_id: u64,
) -> AppResult<AdminNewsItemResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_news_item_from_store(&pool, news_id).await
}

pub(crate) async fn create_admin_news_item(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAdminNewsItemRequest,
) -> AppResult<AdminNewsItemResponse> {
    let title = validate_news_title(&request.title)?;
    let banner_url = validate_optional_image_url(request.banner_url, "news banner_url")?;
    let small_logo_url =
        validate_optional_image_url(request.small_logo_url, "news small_logo_url")?;
    let category = validate_news_category(&request.category)?;
    let status = request
        .status
        .as_deref()
        .map(validate_news_status)
        .transpose()?
        .unwrap_or_else(|| "draft".to_owned());
    let country_code = normalize_optional_news_country_code(request.country_code)?;
    let default_locale = validate_news_locale(&request.default_locale)?;
    let content_json = validate_news_content_document(request.content_json, &default_locale)?;
    let published_at = (status == "published").then(Utc::now);
    let pool = admin_mysql_pool(pool)?;

    // 新闻正文、发布状态和审计日志必须同事务提交，避免后台显示与审计记录不一致。
    let mut tx = pool.begin().await?;
    let news_id = insert_admin_news_item_in_tx(
        &mut tx,
        AdminNewsInsert {
            title,
            banner_url,
            small_logo_url,
            category,
            status,
            country_code,
            default_locale,
            content_json,
            published_at,
            admin_id,
        },
    )
    .await?;
    let news = load_admin_news_item_in_tx(&mut tx, news_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "admin_news_item.create",
            target_type: "admin_news_item",
            target_id: news.id,
            before_json: None,
            after_json: Some(admin_news_item_audit_json(&news)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(news)
}

pub(crate) async fn update_admin_news_item(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    news_id: u64,
    request: UpdateAdminNewsItemRequest,
) -> AppResult<AdminNewsItemResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let title = validate_news_title(&request.title)?;
    let banner_url = validate_optional_image_url(request.banner_url, "news banner_url")?;
    let small_logo_url =
        validate_optional_image_url(request.small_logo_url, "news small_logo_url")?;
    let category = validate_news_category(&request.category)?;
    let country_code = normalize_optional_news_country_code(request.country_code)?;
    let default_locale = validate_news_locale(&request.default_locale)?;
    let content_json = validate_news_content_document(request.content_json, &default_locale)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定旧新闻再写入，审计 before/after 才能精确反映本次编辑。
    let mut tx = pool.begin().await?;
    let before = lock_admin_news_item_in_tx(&mut tx, news_id).await?;
    update_admin_news_item_in_tx(
        &mut tx,
        news_id,
        AdminNewsUpdate {
            title,
            banner_url,
            small_logo_url,
            category,
            country_code,
            default_locale,
            content_json,
            admin_id,
        },
    )
    .await?;
    let after = load_admin_news_item_in_tx(&mut tx, news_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "admin_news_item.update",
            target_type: "admin_news_item",
            target_id: after.id,
            before_json: Some(admin_news_item_audit_json(&before)),
            after_json: Some(admin_news_item_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_news_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    news_id: u64,
    request: UpdateAdminNewsStatusRequest,
) -> AppResult<AdminNewsItemResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let status = validate_news_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;

    // 首次发布才补 published_at；归档或重复发布保留原发布时间。
    let mut tx = pool.begin().await?;
    let before = lock_admin_news_item_in_tx(&mut tx, news_id).await?;
    let published_at = if status == "published" && before.published_at.is_none() {
        Some(Utc::now())
    } else {
        before.published_at
    };
    update_admin_news_status_in_tx(
        &mut tx,
        news_id,
        AdminNewsStatusUpdate {
            status,
            published_at,
            admin_id,
        },
    )
    .await?;
    let after = load_admin_news_item_in_tx(&mut tx, news_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "admin_news_item.status.update",
            target_type: "admin_news_item",
            target_id: after.id,
            before_json: Some(admin_news_item_audit_json(&before)),
            after_json: Some(admin_news_item_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_assets(
    pool: Option<Pool<MySql>>,
    query: AdminAssetQuery,
) -> AppResult<AdminAssetsResponse> {
    let symbol = query
        .symbol
        .and_then(optional_string)
        .map(|value| normalize_asset_symbol(&value))
        .transpose()?;
    let asset_type = query
        .asset_type
        .and_then(optional_string)
        .map(|value| validate_asset_type(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_asset_status(&value))
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;
    let assets = list_admin_assets_from_store(
        &pool,
        AdminAssetListFilter {
            symbol,
            asset_type,
            status,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminAssetsResponse { assets })
}

pub(crate) async fn get_admin_asset(
    pool: Option<Pool<MySql>>,
    asset_id: u64,
) -> AppResult<AdminAssetResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_asset_from_store(&pool, asset_id).await
}

pub(crate) async fn create_admin_asset(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAssetRequest,
) -> AppResult<AdminAssetResponse> {
    validate_create_asset_request(&request)?;
    let symbol = normalize_asset_symbol(&request.symbol)?;
    let name = validate_asset_name(&request.name)?;
    let logo_url = validate_optional_image_url(request.logo_url, "asset logo_url")?;
    let asset_type = request
        .asset_type
        .as_deref()
        .map(validate_asset_type)
        .transpose()?
        .unwrap_or_else(|| "coin".to_owned());
    let status = request
        .status
        .as_deref()
        .map(validate_asset_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let deposit_enabled = request.deposit_enabled.unwrap_or(true);
    let withdraw_enabled = request.withdraw_enabled.unwrap_or(true);
    let min_deposit_amount = request
        .min_deposit_amount
        .unwrap_or_else(|| BigDecimal::from(0));
    let deposit_fee = request.deposit_fee.unwrap_or_else(|| BigDecimal::from(0));
    let withdraw_fee = request.withdraw_fee.unwrap_or_else(|| BigDecimal::from(0));
    let withdraw_fee_tiers =
        normalize_asset_withdraw_fee_tiers(request.withdraw_fee_tiers.unwrap_or_default())?;
    validate_asset_fee_settings(&min_deposit_amount, &deposit_fee, &withdraw_fee)?;
    let pool = admin_mysql_pool(pool)?;

    // 资产创建和钱包账户初始化必须同事务提交，避免用户缺少新资产账户。
    let mut tx = pool.begin().await?;
    let asset_id = insert_admin_asset_in_tx(
        &mut tx,
        AdminAssetInsert {
            symbol,
            name,
            logo_url,
            precision_scale: request.precision_scale,
            asset_type,
            status,
            deposit_enabled,
            withdraw_enabled,
            min_deposit_amount,
            deposit_fee,
            withdraw_fee,
            withdraw_fee_tiers,
        },
    )
    .await?;
    let asset = load_admin_asset_in_tx(&mut tx, asset_id).await?;
    create_wallet_accounts_for_asset_in_tx(&mut tx, asset.id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "asset.create",
            target_type: "asset",
            target_id: asset.id,
            before_json: None,
            after_json: Some(asset_audit_json(&asset)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(asset)
}

pub(crate) async fn update_admin_asset(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    asset_id: u64,
    request: UpdateAssetRequest,
) -> AppResult<AdminAssetResponse> {
    validate_update_asset_request(&request)?;
    let name = validate_asset_name(&request.name)?;
    let asset_type = validate_asset_type(&request.asset_type)?;
    let status = validate_asset_status(&request.status)?;
    let logo_url = validate_optional_image_url(request.logo_url, "asset logo_url")?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定资产旧值再更新，确保资产配置和审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_admin_asset_in_tx(&mut tx, asset_id).await?;
    let deposit_enabled = request.deposit_enabled.unwrap_or(before.deposit_enabled);
    let withdraw_enabled = request.withdraw_enabled.unwrap_or(before.withdraw_enabled);
    let min_deposit_amount = request
        .min_deposit_amount
        .unwrap_or_else(|| before.min_deposit_amount.clone());
    let deposit_fee = request
        .deposit_fee
        .unwrap_or_else(|| before.deposit_fee.clone());
    let withdraw_fee = request
        .withdraw_fee
        .unwrap_or_else(|| before.withdraw_fee.clone());
    let withdraw_fee_tiers = match request.withdraw_fee_tiers {
        Some(tiers) => normalize_asset_withdraw_fee_tiers(tiers)?,
        None => before.withdraw_fee_tiers.0.clone(),
    };
    validate_asset_fee_settings(&min_deposit_amount, &deposit_fee, &withdraw_fee)?;
    update_admin_asset_in_tx(
        &mut tx,
        asset_id,
        AdminAssetUpdate {
            name,
            logo_url,
            precision_scale: request.precision_scale,
            asset_type,
            status,
            deposit_enabled,
            withdraw_enabled,
            min_deposit_amount,
            deposit_fee,
            withdraw_fee,
            withdraw_fee_tiers,
        },
    )
    .await?;
    let after = load_admin_asset_in_tx(&mut tx, asset_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "asset.config.update",
            target_type: "asset",
            target_id: after.id,
            before_json: Some(asset_audit_json(&before)),
            after_json: Some(asset_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn delete_admin_asset(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    asset_id: u64,
    request: DeleteAssetRequest,
) -> AppResult<()> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 删除前先清理零余额钱包账户，再检查引用，避免仅由空钱包账户阻止资产退场。
    let mut tx = pool.begin().await?;
    let before = lock_admin_asset_in_tx(&mut tx, asset_id).await?;
    if before.status != "disabled" {
        return Err(AppError::Validation(
            "asset must be disabled before deletion".to_owned(),
        ));
    }
    delete_zero_balance_wallet_accounts_for_asset_in_tx(&mut tx, asset_id).await?;
    ensure_asset_has_no_references_in_tx(&mut tx, asset_id).await?;
    delete_admin_asset_in_tx(&mut tx, asset_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "asset.delete",
            target_type: "asset",
            target_id: asset_id,
            before_json: Some(asset_audit_json(&before)),
            after_json: None,
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn list_admin_users(
    pool: Option<Pool<MySql>>,
    query: AdminUserQuery,
) -> AppResult<AdminUsersResponse> {
    let email = query.email.and_then(optional_string);
    let status = query.status.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let users = list_admin_users_from_store(
        &pool,
        AdminUserListFilter {
            user_id: query.user_id,
            email,
            status,
            include_internal: query.include_internal.unwrap_or(false),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminUsersResponse { users })
}

pub(crate) async fn get_admin_user(
    pool: Option<Pool<MySql>>,
    user_id: u64,
) -> AppResult<AdminUserResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_user_from_store(&pool, user_id).await
}

pub(crate) async fn create_admin_user(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAdminUserRequest,
) -> AppResult<AdminUserResponse> {
    validate_create_admin_user_request(&request)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let email = request.email.and_then(optional_string);
    let phone = request.phone.and_then(optional_string);
    let status = request
        .status
        .as_deref()
        .map(validate_user_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let kyc_level = request.kyc_level.unwrap_or(0);
    let password_hash = hash_admin_user_password(&request.password)?;
    let pool = admin_mysql_pool(pool)?;

    // 用户创建、邀请码生成和后台审计同事务提交，避免出现无邀请码或无审计的新用户。
    let mut tx = pool.begin().await?;
    let user_id = insert_admin_user_in_tx(
        &mut tx,
        AdminUserInsert {
            email,
            phone,
            password_hash,
            status,
            kyc_level,
        },
    )
    .await?;
    create_user_invite_code_in_tx(&mut tx, user_id).await?;
    let user = load_admin_user_in_tx(&mut tx, user_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "user.create",
            target_type: "user",
            target_id: user.id,
            before_json: None,
            after_json: Some(user_audit_json(&user)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(user)
}

pub(crate) async fn recharge_admin_user_wallet(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    user_id: u64,
    request: AdminUserRechargeRequest,
) -> AppResult<AdminUserRechargeResponse> {
    validate_admin_user_recharge(&request)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;
    let recharge_id = Uuid::now_v7().to_string();

    // 后台人工充值必须把余额更新、钱包流水和审计写入放在同一事务中。
    let mut tx = pool.begin().await?;
    ensure_admin_user_exists_in_tx(&mut tx, user_id).await?;
    let asset = load_active_asset_symbol_in_tx(&mut tx, request.asset_id).await?;
    credit_admin_wallet_available_in_tx(
        &mut tx,
        user_id,
        request.asset_id,
        &request.amount,
        "admin_recharge",
        "admin_recharge",
        &recharge_id,
    )
    .await?;
    let wallet = lock_or_create_admin_wallet_row_in_tx(&mut tx, user_id, request.asset_id).await?;
    let response = AdminUserRechargeResponse {
        recharge_id,
        user_id,
        asset_id: request.asset_id,
        asset_symbol: asset.symbol,
        amount: request.amount,
        available: wallet.available,
        frozen: wallet.frozen,
        locked: wallet.locked,
    };
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "wallet.recharge",
            target_type: "wallet_account",
            target_id: user_id,
            before_json: None,
            after_json: Some(recharge_audit_json(&response)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(response)
}

pub(crate) async fn list_admin_agents(
    pool: Option<Pool<MySql>>,
    query: AdminAgentQuery,
) -> AppResult<AdminAgentsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let agents = list_admin_agents_from_store(
        &pool,
        AdminAgentListFilter {
            agent_id: query.agent_id,
            user_id: query.user_id,
            parent_agent_id: query.parent_agent_id,
            root_agent_id: query.root_agent_id,
            level: query.level,
            agent_code: query.agent_code.and_then(optional_string),
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminAgentsResponse { agents })
}

pub(crate) async fn get_admin_agent(
    pool: Option<Pool<MySql>>,
    agent_id: u64,
) -> AppResult<AdminAgentResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_agent_from_store(&pool, agent_id).await
}

pub(crate) async fn create_admin_agent(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAgentRequest,
) -> AppResult<AdminAgentResponse> {
    validate_create_agent_request(&request)?;
    let admin_password_hash = agent_password_hash(&request)?;
    let CreateAgentRequest {
        user_id,
        parent_agent_id,
        agent_code,
        admin_username,
        level,
        reason,
        ..
    } = request;
    let agent_code = optional_string(agent_code).expect("agent_code validated");
    let admin_username = optional_string(admin_username).expect("admin_username validated");
    let pool = admin_mysql_pool(pool)?;

    // 创建代理主表、代理后台账号和审计日志必须同事务提交，避免半成品代理账号。
    let mut tx = pool.begin().await?;
    ensure_admin_user_exists_in_tx(&mut tx, user_id).await?;
    let parent = match parent_agent_id {
        Some(parent_agent_id) => {
            Some(lock_active_agent_hierarchy_node_in_tx(&mut tx, parent_agent_id).await?)
        }
        None => None,
    };
    let placement = derive_agent_placement(parent.as_ref(), level)?;
    let agent_id = insert_admin_agent_in_tx(
        &mut tx,
        AdminAgentWrite {
            user_id,
            parent_agent_id: placement.parent_agent_id,
            root_agent_id: placement.root_agent_id,
            agent_code,
            level: placement.level,
        },
    )
    .await?;
    let root_agent_id = placement.root_agent_id.unwrap_or(agent_id);
    let hierarchy_path = agent_path(placement.path_prefix.as_deref(), agent_id);
    finalize_admin_agent_hierarchy_in_tx(&mut tx, agent_id, root_agent_id, &hierarchy_path).await?;
    insert_agent_admin_user_in_tx(
        &mut tx,
        AdminAgentAdminUserWrite {
            agent_id,
            username: admin_username,
            password_hash: admin_password_hash,
        },
    )
    .await?;
    let after = load_admin_agent_in_tx(&mut tx, agent_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent.create",
            target_type: "agent",
            target_id: agent_id,
            before_json: None,
            after_json: Some(agent_audit_json(&after)),
            reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_agent_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    agent_id: u64,
    request: UpdateAgentStatusRequest,
) -> AppResult<AdminAgentResponse> {
    let status = validate_agent_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;

    // 锁定代理行后同步代理后台账号状态，确保审计 before/after 与实际可登录状态一致。
    let mut tx = pool.begin().await?;
    let before = lock_admin_agent_in_tx(&mut tx, agent_id).await?;
    update_admin_agent_status_in_tx(&mut tx, agent_id, &status).await?;
    update_agent_admin_users_status_in_tx(&mut tx, agent_id, &status).await?;
    let after = load_admin_agent_in_tx(&mut tx, agent_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent.status.update",
            target_type: "agent",
            target_id: agent_id,
            before_json: Some(agent_audit_json(&before)),
            after_json: Some(agent_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_agent_users(
    pool: Option<Pool<MySql>>,
    agent_id: u64,
    limit: Option<u32>,
) -> AppResult<AdminAgentUsersResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_agent_from_store(&pool, agent_id).await?;
    let users = list_admin_agent_users_from_store(&pool, agent_id, route_limit(limit)).await?;
    Ok(AdminAgentUsersResponse { users })
}

pub(crate) async fn assign_admin_user_agent(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    user_id: u64,
    request: AssignUserAgentRequest,
) -> AppResult<AdminUserReferralResponse> {
    if request.agent_id == 0 {
        return Err(AppError::Validation("agent_id is required".to_owned()));
    }
    let pool = admin_mysql_pool(pool)?;

    // 改派用户代理归属时同时锁定用户、代理和既有邀请关系，防止并发覆盖团队树。
    let mut tx = pool.begin().await?;
    ensure_admin_user_exists_in_tx(&mut tx, user_id).await?;
    lock_active_agent_hierarchy_node_in_tx(&mut tx, request.agent_id).await?;
    let agent = lock_admin_agent_in_tx(&mut tx, request.agent_id).await?;
    if agent.status != "active" {
        return Err(AppError::Conflict(
            "only active agents can receive assigned users".to_owned(),
        ));
    }
    let before = lock_user_referral_in_tx(&mut tx, user_id).await?;
    let previous_tree = before.as_ref().map(|referral| {
        (
            referral.path.clone(),
            referral.depth,
            referral.root_agent_id,
        )
    });
    let path = format!("/{}/{}/{}", request.agent_id, request.agent_id, user_id);
    upsert_user_agent_referral_in_tx(
        &mut tx,
        UserAgentReferralWrite {
            user_id,
            agent_id: request.agent_id,
            path: path.clone(),
        },
    )
    .await?;
    if let Some((old_path, old_depth, old_root_agent_id)) = previous_tree.as_ref() {
        migrate_user_referral_descendants_in_tx(
            &mut tx,
            user_id,
            old_path,
            *old_depth,
            *old_root_agent_id,
            request.agent_id,
            &path,
        )
        .await?;
    }
    let after = load_user_referral_in_tx(&mut tx, user_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "user_referral.assign_agent",
            target_type: "user_referral",
            target_id: user_id,
            before_json: before.as_ref().map(user_referral_audit_json),
            after_json: Some(user_referral_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn get_admin_security_policy(
    pool: Option<Pool<MySql>>,
) -> AppResult<UserSecurityPolicy> {
    let pool = admin_mysql_pool(pool)?;
    load_security_policy(&pool).await
}

pub(crate) async fn update_admin_security_policy(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: UpdateSecurityPolicyRequest,
) -> AppResult<UserSecurityPolicy> {
    let reason = required_admin_audit_reason(request.reason)?;
    validate_security_policy(&request.payment_policies)?;
    let after = UserSecurityPolicy {
        login_2fa_mode: request.login_2fa_mode,
        registration_invite_required: request.registration_invite_required,
        username_login_enabled: request.username_login_enabled,
        payment_policies: request.payment_policies,
        third_party_bindings: request.third_party_bindings,
    };
    let pool = admin_mysql_pool(pool)?;
    let before = load_security_policy(&pool).await?;

    // 安全策略配置和后台审计必须同事务提交，避免策略变更缺少可追溯记录。
    let mut tx = pool.begin().await?;
    save_admin_security_policy_in_tx(&mut tx, &after, admin_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "security_policy.update",
            target_type: "security_policy",
            target_id: 0,
            before_json: Some(security_policy_audit_json(&before)?),
            after_json: Some(security_policy_audit_json(&after)?),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn get_admin_kyc_config(
    pool: Option<Pool<MySql>>,
) -> AppResult<KycConfigResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_kyc_config_from_kyc(&pool).await
}

pub(crate) async fn save_admin_kyc_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: SaveKycConfigRequest,
) -> AppResult<KycConfigResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // KYC 配置变更和后台审计同事务提交，避免审核规则生效后缺少追溯记录。
    let mut tx = pool.begin().await?;
    let change = save_kyc_config_in_tx_from_kyc(&mut tx, admin_id, request).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "kyc.config.update",
            target_type: "kyc_config",
            target_id: change.after.id,
            before_json: Some(kyc_config_audit_json(&change.before)),
            after_json: Some(kyc_config_audit_json(&change.after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(change.after)
}

pub(crate) async fn list_admin_kyc_submissions(
    pool: Option<Pool<MySql>>,
    query: AdminKycSubmissionQuery,
) -> AppResult<KycSubmissionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let submissions = list_kyc_submissions_from_kyc(
        &pool,
        ListKycSubmissionsFilter {
            user_id: query.user_id,
            email: query.email,
            status: query.status,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(KycSubmissionsResponse { submissions })
}

pub(crate) async fn get_admin_kyc_submission(
    pool: Option<Pool<MySql>>,
    submission_id: u64,
) -> AppResult<KycSubmissionResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_kyc_submission_from_kyc(&pool, submission_id).await
}

pub(crate) async fn review_admin_kyc_submission(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    submission_id: u64,
    request: ReviewKycSubmissionRequest,
) -> AppResult<KycSubmissionResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // 审核结果、用户 KYC 等级和后台审计必须同事务完成，避免审批状态与用户等级不一致。
    let mut tx = pool.begin().await?;
    let change =
        review_kyc_submission_in_tx_from_kyc(&mut tx, submission_id, admin_id, request).await?;
    let action = if change.after.status == "approved" {
        "kyc.submission.approve"
    } else {
        "kyc.submission.reject"
    };
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action,
            target_type: "user_kyc_submission",
            target_id: submission_id,
            before_json: Some(kyc_submission_audit_json(&change.before)),
            after_json: Some(kyc_submission_audit_json(&change.after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(change.after)
}

pub(crate) async fn get_admin_platform_brand(
    pool: Option<Pool<MySql>>,
) -> AppResult<PlatformBrandResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_platform_brand_from_platform(&pool).await
}

pub(crate) async fn save_admin_platform_brand(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: SavePlatformBrandRequest,
) -> AppResult<PlatformBrandResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // 平台品牌配置会影响前后台展示，配置变更和后台审计需要同事务提交。
    let mut tx = pool.begin().await?;
    let change = save_platform_brand_in_tx_from_platform(&mut tx, admin_id, request).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "platform_brand.update",
            target_type: "platform_brand_config",
            target_id: change.after.id,
            before_json: Some(platform_brand_audit_json(&change.before)),
            after_json: Some(platform_brand_audit_json(&change.after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(change.after)
}

pub(crate) async fn reset_admin_user_two_factor(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    user_id: u64,
    request: ResetUserTwoFactorRequest,
) -> AppResult<AdminUserTwoFactorResetResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 用户存在校验、2FA 重置和后台审计同事务完成，避免审计记录与实际重置状态不一致。
    let mut tx = pool.begin().await?;
    ensure_admin_user_exists_in_tx(&mut tx, user_id).await?;
    let before = load_admin_user_two_factor_in_tx(&mut tx, user_id).await?;
    let after = reset_admin_user_two_factor_in_tx(&mut tx, user_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "user_2fa.reset",
            target_type: "user_two_factor",
            target_id: user_id,
            before_json: Some(two_factor_audit_json(&before)),
            after_json: Some(two_factor_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(AdminUserTwoFactorResetResponse {
        user_id,
        totp_enabled: after.totp_enabled,
        login_2fa_enabled: after.login_2fa_enabled,
    })
}

pub(crate) async fn list_admin_wallet_accounts(
    pool: Option<Pool<MySql>>,
    query: AdminWalletAccountQuery,
) -> AppResult<AdminWalletAccountsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let accounts = list_admin_wallet_accounts_from_store(
        &pool,
        AdminWalletAccountListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            include_empty: query.include_empty.unwrap_or(false),
            include_internal: query.include_internal.unwrap_or(false),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminWalletAccountsResponse { accounts })
}

pub(crate) async fn list_admin_wallet_ledger(
    pool: Option<Pool<MySql>>,
    query: AdminWalletLedgerQuery,
) -> AppResult<AdminWalletLedgerResponseList> {
    let pool = admin_mysql_pool(pool)?;
    let ledger = list_admin_wallet_ledger_from_store(
        &pool,
        AdminWalletLedgerListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            change_type: query.change_type,
            ref_type: query.ref_type,
            include_internal: query.include_internal.unwrap_or(false),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminWalletLedgerResponseList { ledger })
}

pub(crate) async fn list_admin_risk_rules(
    pool: Option<Pool<MySql>>,
    query: AdminRiskRuleQuery,
) -> AppResult<RiskRulesResponse> {
    let pool = admin_mysql_pool(pool)?;
    let rules = list_admin_risk_rules_from_store(
        &pool,
        AdminRiskRuleListFilter {
            rule_type: query.rule_type.and_then(optional_string),
            target_type: query.target_type.and_then(optional_string),
            enabled: query.enabled,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(RiskRulesResponse { rules })
}

pub(crate) async fn create_admin_risk_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateRiskRuleRequest,
) -> AppResult<RiskRuleResponse> {
    validate_create_risk_rule(&request)?;
    let CreateRiskRuleRequest {
        rule_type,
        target_type,
        target_id,
        config_json,
        enabled,
        reason,
    } = request;
    let rule_type = optional_string(rule_type).expect("risk rule type validated");
    let target_type = optional_string(target_type).expect("risk target type validated");
    let target_id = target_id.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;

    // 风控规则变更和后台审计必须同事务提交，避免规则已生效但操作来源不可追踪。
    let mut tx = pool.begin().await?;
    let rule_id = insert_risk_rule_in_tx(
        &mut tx,
        RiskRuleWrite {
            rule_type,
            target_type,
            target_id,
            config_json,
            enabled: enabled.unwrap_or(true),
            created_by: admin_id,
        },
    )
    .await?;
    let rule = load_risk_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "risk_rule.create",
            target_type: "risk_rule",
            target_id: rule_id,
            before_json: None,
            after_json: Some(risk_rule_audit_json(&rule)),
            reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(rule)
}

pub(crate) async fn update_admin_risk_rule_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    rule_id: u64,
    request: UpdateRiskRuleStatusRequest,
) -> AppResult<RiskRuleResponse> {
    let pool = admin_mysql_pool(pool)?;

    // 先锁定旧规则再更新状态，确保审计 before/after 对应同一次状态切换。
    let mut tx = pool.begin().await?;
    let before = lock_risk_rule_in_tx(&mut tx, rule_id).await?;
    update_risk_rule_status_in_tx(&mut tx, rule_id, request.enabled).await?;
    let after = load_risk_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "risk_rule.status.update",
            target_type: "risk_rule",
            target_id: rule_id,
            before_json: Some(risk_rule_audit_json(&before)),
            after_json: Some(risk_rule_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_risk_events(
    pool: Option<Pool<MySql>>,
    query: AdminRiskEventQuery,
) -> AppResult<RiskEventsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let events = list_admin_risk_events_from_store(
        &pool,
        AdminRiskEventListFilter {
            user_id: query.user_id,
            email: query.email,
            decision: query.decision.and_then(optional_string),
            risk_level: query.risk_level.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(RiskEventsResponse { events })
}

pub(crate) async fn list_admin_agent_commission_rules(
    pool: Option<Pool<MySql>>,
    query: AdminAgentCommissionRuleQuery,
) -> AppResult<AdminAgentCommissionRulesResponse> {
    let product_type = query.product_type.and_then(optional_string);
    let status = query.status.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let rules = list_admin_agent_commission_rules_from_store(
        &pool,
        AdminAgentCommissionRuleListFilter {
            agent_id: query.agent_id,
            product_type,
            status,
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminAgentCommissionRulesResponse { rules })
}

pub(crate) async fn create_admin_agent_commission_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateAgentCommissionRuleRequest,
) -> AppResult<AdminAgentCommissionRuleResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let product_type = validate_agent_commission_rule_product_type(&request.product_type)?;
    let status = request
        .status
        .as_deref()
        .map(validate_agent_commission_rule_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    validate_agent_commission_rate(&request.commission_rate)?;
    if request.agent_id == 0 {
        return Err(AppError::Validation("agent_id is required".to_owned()));
    }
    let pool = admin_mysql_pool(pool)?;

    // 代理存在性检查、佣金规则写入和后台审计必须同事务提交，避免孤立规则或缺失审计。
    let mut tx = pool.begin().await?;
    ensure_agent_exists_in_tx(&mut tx, request.agent_id).await?;
    let rule_id = insert_agent_commission_rule_in_tx(
        &mut tx,
        AdminAgentCommissionRuleWrite {
            agent_id: request.agent_id,
            product_type,
            commission_rate: request.commission_rate,
            status,
        },
    )
    .await?;
    let after = load_agent_commission_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent_commission_rule.create",
            target_type: "agent_commission_rule",
            target_id: rule_id,
            before_json: None,
            after_json: Some(agent_commission_rule_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_agent_commission_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    rule_id: u64,
    request: UpdateAgentCommissionRuleRequest,
) -> AppResult<AdminAgentCommissionRuleResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let commission_rate = if let Some(commission_rate) = request.commission_rate {
        validate_agent_commission_rate(&commission_rate)?;
        Some(commission_rate)
    } else {
        None
    };
    let status = request
        .status
        .as_deref()
        .map(validate_agent_commission_rule_status)
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定旧规则再更新，确保代理佣金规则审计 before/after 与本次事务一致。
    let mut tx = pool.begin().await?;
    let before = lock_agent_commission_rule_in_tx(&mut tx, rule_id).await?;
    update_agent_commission_rule_in_tx(
        &mut tx,
        rule_id,
        commission_rate.as_ref(),
        status.as_deref(),
    )
    .await?;
    let after = load_agent_commission_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent_commission_rule.update",
            target_type: "agent_commission_rule",
            target_id: rule_id,
            before_json: Some(agent_commission_rule_audit_json(&before)),
            after_json: Some(agent_commission_rule_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_agent_commissions(
    pool: Option<Pool<MySql>>,
    query: AdminAgentCommissionQuery,
) -> AppResult<AdminAgentCommissionsResponse> {
    let status = query.status.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let commissions = list_admin_agent_commissions_from_store(
        &pool,
        AdminAgentCommissionListFilter {
            agent_id: query.agent_id,
            user_id: query.user_id,
            email: query.email,
            status,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminAgentCommissionsResponse { commissions })
}

pub(crate) async fn update_admin_agent_commission_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    commission_id: u64,
    request: UpdateAgentCommissionStatusRequest,
) -> AppResult<AdminAgentCommissionResponse> {
    let status = validate_agent_commission_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;

    // 锁定佣金记录后只允许 pending 进入结算/拒绝，防止重复给代理钱包入账。
    let mut tx = pool.begin().await?;
    let before = lock_agent_commission_in_tx(&mut tx, commission_id).await?;
    if before.status != "pending" {
        return Err(AppError::Conflict(
            "agent commission status can only be updated from pending".to_owned(),
        ));
    }
    if status == "settled" {
        settle_agent_commission_payout_in_tx(&mut tx, &before).await?;
    }
    update_agent_commission_status_in_tx(&mut tx, commission_id, &status).await?;
    let after = load_agent_commission_in_tx(&mut tx, commission_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "agent_commission.status.update",
            target_type: "agent_commission",
            target_id: commission_id,
            before_json: Some(agent_commission_audit_json(&before)),
            after_json: Some(agent_commission_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

async fn settle_agent_commission_payout_in_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    commission: &AdminAgentCommissionResponse,
) -> AppResult<()> {
    let target = load_agent_commission_payout_target_in_tx(tx, commission.id)
        .await
        .map_err(|error| match error {
            AppError::NotFound => AppError::Conflict(
                "agent commission source cannot be settled without payout support".to_owned(),
            ),
            other => other,
        })?;
    credit_admin_wallet_available_in_tx(
        tx,
        target.agent_user_id,
        target.asset_id,
        &commission.commission_amount,
        "agent_commission_payout",
        "agent_commission",
        &commission.id.to_string(),
    )
    .await
}

pub(crate) async fn list_admin_deposit_network_configs(
    pool: Option<Pool<MySql>>,
    query: AdminDepositNetworkConfigQuery,
) -> AppResult<AdminDepositNetworkConfigResponseList> {
    let network = query
        .network
        .and_then(optional_string)
        .map(|value| normalize_deposit_network(&value))
        .transpose()?;
    let address_group_code = query
        .address_group_code
        .and_then(optional_string)
        .map(|value| validate_address_group_code(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_deposit_network_config_status(&value))
        .transpose()?;
    let asset_symbol = query
        .asset_symbol
        .and_then(optional_string)
        .map(|value| normalize_asset_symbol(&value))
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;
    let configs = list_admin_deposit_network_configs_from_store(
        &pool,
        AdminDepositNetworkConfigListFilter {
            network,
            address_group_code,
            status,
            asset_symbol,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminDepositNetworkConfigResponseList { configs })
}

pub(crate) async fn create_admin_deposit_network_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateDepositNetworkConfigRequest,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let display_name = validate_deposit_network_display_name(&request.display_name)?;
    let address_group_code = validate_address_group_code(&request.address_group_code)?;
    let address_group_name =
        validate_optional_length(request.address_group_name, "address_group_name", 128)?;
    let asset_symbols = normalize_deposit_asset_symbols(None, request.asset_symbols)?;
    let status = request
        .status
        .as_deref()
        .map(validate_deposit_network_config_status)
        .transpose()?
        .unwrap_or_else(|| "active".to_owned());
    let sort_order = request.sort_order.unwrap_or(0);
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;

    // 网络配置写入与审计同事务提交，避免充值地址池读取到无审计的配置变更。
    let mut tx = pool.begin().await?;
    let config_id = insert_admin_deposit_network_config_in_tx(
        &mut tx,
        AdminDepositNetworkConfigWrite {
            network,
            display_name,
            address_group_code,
            address_group_name,
            asset_symbols,
            status,
            sort_order,
        },
    )
    .await?;
    let created = load_deposit_network_config_in_tx(&mut tx, config_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_network_config.create",
            target_type: "deposit_network_config",
            target_id: created.id,
            before_json: None,
            after_json: Some(deposit_network_config_audit_json(&created)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(created)
}

pub(crate) async fn update_admin_deposit_network_config(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    config_id: u64,
    request: UpdateDepositNetworkConfigRequest,
) -> AppResult<AdminDepositNetworkConfigResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let display_name = validate_deposit_network_display_name(&request.display_name)?;
    let address_group_code = validate_address_group_code(&request.address_group_code)?;
    let address_group_name =
        validate_optional_length(request.address_group_name, "address_group_name", 128)?;
    let asset_symbols = normalize_deposit_asset_symbols(None, request.asset_symbols)?;
    let status = validate_deposit_network_config_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;

    // 先锁定旧网络配置再更新，确保充值网络配置审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_deposit_network_config_in_tx(&mut tx, config_id).await?;
    update_admin_deposit_network_config_in_tx(
        &mut tx,
        config_id,
        AdminDepositNetworkConfigWrite {
            network,
            display_name,
            address_group_code,
            address_group_name,
            asset_symbols,
            status,
            sort_order: request.sort_order,
        },
    )
    .await?;
    let after = load_deposit_network_config_in_tx(&mut tx, config_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_network_config.update",
            target_type: "deposit_network_config",
            target_id: after.id,
            before_json: Some(deposit_network_config_audit_json(&before)),
            after_json: Some(deposit_network_config_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    query: AdminDepositAddressPoolQuery,
) -> AppResult<AdminDepositAddressPoolResponseList> {
    let network = query
        .network
        .and_then(optional_string)
        .map(|value| normalize_deposit_network(&value))
        .transpose()?;
    let address_group_code = query
        .address_group_code
        .and_then(optional_string)
        .map(|value| validate_address_group_code(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_deposit_address_status(&value))
        .transpose()?;
    let asset_symbol = query
        .asset_symbol
        .and_then(optional_string)
        .map(|value| normalize_asset_symbol(&value))
        .transpose()?;
    let address = query.address.and_then(optional_string);
    let pool = admin_mysql_pool(pool)?;
    let addresses = list_admin_deposit_address_pool_from_store(
        &pool,
        AdminDepositAddressPoolListFilter {
            network,
            address_group_code,
            status,
            asset_symbol,
            assigned_user_id: query.assigned_user_id,
            email: query.email,
            address,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminDepositAddressPoolResponseList { addresses })
}

pub(crate) async fn get_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    address_id: u64,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_deposit_address_pool(&pool, address_id).await
}

pub(crate) async fn create_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateDepositAddressPoolRequest,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let address = validate_deposit_address(&request.address)?;
    let asset_symbols =
        normalize_deposit_asset_symbols(request.asset_symbol, request.asset_symbols)?;
    let status = request
        .status
        .as_deref()
        .map(validate_deposit_address_assignable_status)
        .transpose()?
        .unwrap_or_else(|| "available".to_owned());
    let memo = validate_optional_length(request.memo, "memo", 255)?;
    let remark = validate_optional_length(request.remark, "remark", 512)?;
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;
    let network_config = load_deposit_network_config_by_network(&pool, &network).await?;
    ensure_deposit_asset_symbols_allowed_by_network(&asset_symbols, &network_config)?;
    let address_group_code =
        resolve_deposit_address_group_code(request.address_group_code, &network_config)?;

    // 地址入池和审计同事务提交，确保后台地址池变更可追踪。
    let mut tx = pool.begin().await?;
    let created = insert_deposit_address_pool_in_tx(
        &mut tx,
        AdminDepositAddressPoolWrite {
            network,
            address_group_code,
            address,
            asset_symbols,
            status,
            memo,
            remark,
        },
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_address_pool.create",
            target_type: "deposit_address_pool",
            target_id: created.id,
            before_json: None,
            after_json: Some(deposit_address_pool_audit_json(&created)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(created)
}

pub(crate) async fn create_admin_deposit_address_pool_batch(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateDepositAddressPoolBatchRequest,
) -> AppResult<AdminDepositAddressPoolBatchResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let asset_symbols =
        normalize_deposit_asset_symbols(request.asset_symbol, request.asset_symbols)?;
    let status = request
        .status
        .as_deref()
        .map(validate_deposit_address_assignable_status)
        .transpose()?
        .unwrap_or_else(|| "available".to_owned());
    let entries = normalize_deposit_address_batch_entries(request.entries)?;
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;
    let network_config = load_deposit_network_config_by_network(&pool, &network).await?;
    ensure_deposit_asset_symbols_allowed_by_network(&asset_symbols, &network_config)?;
    let address_group_code =
        resolve_deposit_address_group_code(request.address_group_code, &network_config)?;

    // 批量入池逐条写审计，保持每个地址都有独立后台操作轨迹。
    let mut tx = pool.begin().await?;
    let mut addresses = Vec::with_capacity(entries.len());
    for entry in entries {
        let created = insert_deposit_address_pool_in_tx(
            &mut tx,
            AdminDepositAddressPoolWrite {
                network: network.clone(),
                address_group_code: address_group_code.clone(),
                address: entry.address,
                asset_symbols: asset_symbols.clone(),
                status: status.clone(),
                memo: entry.memo,
                remark: entry.remark,
            },
        )
        .await?;
        insert_admin_audit_log_entry_in_tx(
            &mut tx,
            admin_id,
            AdminAuditLogEntry {
                action: "deposit_address_pool.create",
                target_type: "deposit_address_pool",
                target_id: created.id,
                before_json: None,
                after_json: Some(deposit_address_pool_audit_json(&created)),
                reason: Some(reason.clone()),
            },
        )
        .await?;
        addresses.push(created);
    }
    tx.commit().await?;
    Ok(AdminDepositAddressPoolBatchResponse { addresses })
}

pub(crate) async fn update_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    address_id: u64,
    request: UpdateDepositAddressPoolRequest,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let network = normalize_deposit_network(&request.network)?;
    let address = validate_deposit_address(&request.address)?;
    let asset_symbols =
        normalize_deposit_asset_symbols(request.asset_symbol, request.asset_symbols)?;
    let status = validate_deposit_address_assignable_status(&request.status)?;
    let memo = validate_optional_length(request.memo, "memo", 255)?;
    let remark = validate_optional_length(request.remark, "remark", 512)?;
    let pool = admin_mysql_pool(pool)?;
    ensure_asset_symbols_exist(&pool, &asset_symbols).await?;
    let network_config = load_deposit_network_config_by_network(&pool, &network).await?;
    ensure_deposit_asset_symbols_allowed_by_network(&asset_symbols, &network_config)?;
    let address_group_code =
        resolve_deposit_address_group_code(request.address_group_code, &network_config)?;

    // 已分配地址必须先回收再编辑，避免用户充值地址被后台直接改写。
    let mut tx = pool.begin().await?;
    let before = lock_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    if before.status == "assigned" {
        return Err(AppError::Validation(
            "assigned deposit address must be reclaimed before editing".to_owned(),
        ));
    }
    update_deposit_address_pool_in_tx(
        &mut tx,
        address_id,
        AdminDepositAddressPoolWrite {
            network,
            address_group_code,
            address,
            asset_symbols,
            status,
            memo,
            remark,
        },
    )
    .await?;
    let after = load_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_address_pool.update",
            target_type: "deposit_address_pool",
            target_id: after.id,
            before_json: Some(deposit_address_pool_audit_json(&before)),
            after_json: Some(deposit_address_pool_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn reclaim_admin_deposit_address_pool(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    address_id: u64,
    request: ReclaimDepositAddressPoolRequest,
) -> AppResult<AdminDepositAddressPoolResponse> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 回收操作只清分配字段，不改地址自身配置，保证地址可重新进入可分配池。
    let mut tx = pool.begin().await?;
    let before = lock_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    if before.status != "assigned" {
        return Err(AppError::Validation(
            "only assigned deposit address can be reclaimed".to_owned(),
        ));
    }
    reclaim_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    let after = load_deposit_address_pool_in_tx(&mut tx, address_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "deposit_address_pool.reclaim",
            target_type: "deposit_address_pool",
            target_id: after.id,
            before_json: Some(deposit_address_pool_audit_json(&before)),
            after_json: Some(deposit_address_pool_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_trading_pairs(
    pool: Option<Pool<MySql>>,
    query: AdminTradingPairQuery,
) -> AppResult<AdminTradingPairsResponse> {
    let symbol = query
        .symbol
        .and_then(optional_string)
        .map(|value| normalize_trading_pair_symbol(&value))
        .transpose()?;
    let status = query
        .status
        .and_then(optional_string)
        .map(|value| validate_trading_pair_status(&value))
        .transpose()?;
    let market_type = query
        .market_type
        .and_then(optional_string)
        .map(|value| validate_trading_pair_market_type(&value))
        .transpose()?;
    let pool = admin_mysql_pool(pool)?;
    let pairs = list_admin_trading_pairs_from_store(
        &pool,
        AdminTradingPairListFilter {
            symbol,
            status,
            market_type,
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminTradingPairsResponse { pairs })
}

pub(crate) async fn get_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    pair_id: u64,
) -> AppResult<AdminTradingPairResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_trading_pair_from_store(&pool, pair_id).await
}

pub(crate) async fn create_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateTradingPairRequest,
) -> AppResult<AdminTradingPairResponse> {
    validate_create_trading_pair_request(&request)?;
    let symbol = normalize_trading_pair_symbol(&request.symbol)?;
    let logo_url = validate_optional_image_url(request.logo_url, "trading pair logo_url")?;
    let status = request
        .status
        .as_deref()
        .map(validate_trading_pair_status)
        .transpose()?
        .unwrap_or_else(|| "disabled".to_owned());
    let market_type = request
        .market_type
        .as_deref()
        .map(validate_trading_pair_market_type)
        .transpose()?
        .unwrap_or_else(|| "external".to_owned());
    let pool = admin_mysql_pool(pool)?;

    // 创建交易对前锁定两个启用资产，避免资产状态变更与交易对创建竞态。
    let mut tx = pool.begin().await?;
    ensure_trading_pair_asset_in_tx(&mut tx, request.base_asset_id).await?;
    ensure_trading_pair_asset_in_tx(&mut tx, request.quote_asset_id).await?;
    let pair_id = insert_admin_trading_pair_in_tx(
        &mut tx,
        AdminTradingPairInsert {
            base_asset_id: request.base_asset_id,
            quote_asset_id: request.quote_asset_id,
            symbol,
            logo_url,
            price_precision: request.price_precision,
            qty_precision: request.qty_precision,
            min_order_value: request.min_order_value,
            status,
            market_type,
        },
    )
    .await?;
    let pair = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "trading_pair.create",
            target_type: "trading_pair",
            target_id: pair.id,
            before_json: None,
            after_json: Some(trading_pair_audit_json(&pair)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(pair)
}

pub(crate) async fn update_admin_trading_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: UpdateTradingPairRequest,
) -> AppResult<AdminTradingPairResponse> {
    validate_update_trading_pair_request(&request)?;
    let status = validate_trading_pair_status(&request.status)?;
    let market_type = validate_trading_pair_market_type(&request.market_type)?;
    let logo_url = validate_optional_image_url(request.logo_url, "trading pair logo_url")?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定交易对旧值再更新，确保后台审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    update_admin_trading_pair_in_tx(
        &mut tx,
        pair_id,
        AdminTradingPairUpdate {
            logo_url,
            price_precision: request.price_precision,
            qty_precision: request.qty_precision,
            min_order_value: request.min_order_value,
            status,
            market_type,
        },
    )
    .await?;
    let after = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "trading_pair.config.update",
            target_type: "trading_pair",
            target_id: after.id,
            before_json: Some(trading_pair_audit_json(&before)),
            after_json: Some(trading_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_trading_pair_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: UpdateTradingPairStatusRequest,
) -> AppResult<AdminTradingPairResponse> {
    let status = validate_trading_pair_status(&request.status)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定交易对旧值再更新，确保后台审计 before/after 对应同一次事务。
    let mut tx = pool.begin().await?;
    let before = lock_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    update_admin_trading_pair_status_in_tx(&mut tx, pair_id, &status).await?;
    let after = load_admin_trading_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "trading_pair.status.update",
            target_type: "trading_pair",
            target_id: after.id,
            before_json: Some(trading_pair_audit_json(&before)),
            after_json: Some(trading_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn list_admin_market_strategies(
    pool: Option<Pool<MySql>>,
    query: AdminMarketStrategyQuery,
) -> AppResult<AdminMarketStrategiesResponse> {
    let pool = admin_mysql_pool(pool)?;
    let strategies = list_admin_market_strategies_from_store(
        &pool,
        AdminMarketStrategyListFilter {
            pair_id: query.pair_id,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(AdminMarketStrategiesResponse { strategies })
}

pub(crate) async fn create_admin_market_strategy(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateMarketStrategyRequest,
) -> AppResult<AdminMarketStrategyResponse> {
    validate_create_market_strategy(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 策略主表、运行检查点、版本快照和审计事件必须同事务提交，避免策略可见但调度状态缺失。
    let mut tx = pool.begin().await?;
    let market_type = ensure_market_strategy_pair_in_tx(&mut tx, request.pair_id).await?;
    let status = request
        .status
        .as_deref()
        .map(validate_market_strategy_status)
        .transpose()?
        .unwrap_or_else(|| "draft".to_owned());
    let strategy_type = optional_string(request.strategy_type.clone()).unwrap();
    let strategy_id = insert_admin_market_strategy_in_tx(
        &mut tx,
        AdminMarketStrategyInsert {
            pair_id: request.pair_id,
            strategy_type,
            start_price: request.start_price.clone(),
            target_price: request.target_price.clone(),
            start_time: request.start_time,
            end_time: request.end_time,
            volatility: request.volatility.clone(),
            volume_min: request.volume_min.clone(),
            volume_max: request.volume_max.clone(),
            status: status.clone(),
        },
    )
    .await?;
    insert_market_strategy_run_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&status),
        &request.start_price,
        request.start_time,
    )
    .await?;
    insert_market_strategy_version_in_tx(
        &mut tx,
        strategy_id,
        1,
        request.start_time,
        market_strategy_config_json(&request, &status, &market_type),
        Uuid::now_v7().to_string(),
        admin_id,
    )
    .await?;
    let strategy = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.create",
        None,
        Some(&strategy),
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(strategy)
}

pub(crate) async fn update_admin_market_strategy(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    strategy_id: u64,
    request: UpdateMarketStrategyRequest,
) -> AppResult<AdminMarketStrategyResponse> {
    validate_update_market_strategy(&request)?;
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // 更新策略配置时先锁定旧值，再重置运行检查点并追加版本快照，保证审计和调度状态一致。
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    if before.status == "active" {
        return Err(AppError::Conflict(
            "active market strategy must be paused or disabled before update".to_owned(),
        ));
    }
    let strategy_type = optional_string(request.strategy_type.clone()).unwrap();
    update_admin_market_strategy_in_tx(
        &mut tx,
        strategy_id,
        AdminMarketStrategyUpdate {
            strategy_type,
            start_price: request.start_price.clone(),
            target_price: request.target_price.clone(),
            start_time: request.start_time,
            end_time: request.end_time,
            volatility: request.volatility.clone(),
            volume_min: request.volume_min.clone(),
            volume_max: request.volume_max.clone(),
        },
    )
    .await?;
    update_market_strategy_run_checkpoint_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&before.status),
        &request.start_price,
        request.start_time,
    )
    .await?;
    let next_version = next_market_strategy_version_in_tx(&mut tx, strategy_id).await?;
    let after = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    insert_market_strategy_version_in_tx(
        &mut tx,
        strategy_id,
        next_version,
        request.start_time,
        market_strategy_update_config_json(&request, &after.status, &after.market_type),
        Uuid::now_v7().to_string(),
        admin_id,
    )
    .await?;
    record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.update",
        Some(&before),
        Some(&after),
        Some(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_market_strategy_status(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    strategy_id: u64,
    request: UpdateMarketStrategyStatusRequest,
) -> AppResult<AdminMarketStrategyResponse> {
    let status = validate_market_strategy_status(&request.status)?;
    let pool = admin_mysql_pool(pool)?;

    // 状态和运行状态一起更新；如果运行检查点缺失，整个状态变更回滚。
    let mut tx = pool.begin().await?;
    let before = lock_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    update_market_strategy_status_in_tx(&mut tx, strategy_id, &status).await?;
    update_market_strategy_run_status_in_tx(
        &mut tx,
        strategy_id,
        market_strategy_run_status(&status),
    )
    .await?;
    let after = load_admin_market_strategy_in_tx(&mut tx, strategy_id).await?;
    record_admin_market_strategy_change_in_tx(
        &mut tx,
        admin_id,
        strategy_id,
        "market_strategy.status.update",
        Some(&before),
        Some(&after),
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

async fn record_admin_market_strategy_change_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    strategy_id: u64,
    action: &'static str,
    before: Option<&AdminMarketStrategyResponse>,
    after: Option<&AdminMarketStrategyResponse>,
    reason: Option<String>,
) -> AppResult<()> {
    let before_json = before.map(market_strategy_audit_json);
    let after_json = after.map(market_strategy_audit_json);
    insert_market_strategy_event_in_tx(
        tx,
        strategy_id,
        action,
        json!({
            "before": before_json,
            "after": after_json,
        }),
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        tx,
        admin_id,
        AdminAuditLogEntry {
            action,
            target_type: "market_strategy",
            target_id: strategy_id,
            before_json,
            after_json,
            reason,
        },
    )
    .await
}

pub(crate) async fn list_admin_new_coin_projects(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinProjectQuery,
) -> AppResult<NewCoinProjectsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let projects = list_admin_new_coin_projects_from_store(&pool, route_limit(query.limit)).await?;
    Ok(NewCoinProjectsResponse { projects })
}

pub(crate) async fn list_admin_new_coin_subscriptions(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinFlatListQuery,
) -> AppResult<NewCoinSubscriptionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let subscriptions = list_admin_new_coin_subscriptions_from_store(
        &pool,
        AdminNewCoinFlatListFilter {
            project_id: query.project_id,
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinSubscriptionsResponse { subscriptions })
}

/// 组装项目过滤列表参数：由路由层传入的子查询参数统一补齐项目ID。
fn build_new_coin_scoped_list_query(
    project_id: u64,
    query: AdminNewCoinScopedListQuery,
) -> AdminNewCoinFlatListQuery {
    AdminNewCoinFlatListQuery {
        project_id: Some(project_id),
        user_id: query.user_id,
        email: query.email,
        status: query.status,
        limit: query.limit,
    }
}

/// 查询某个项目的认购列表。
pub(crate) async fn list_admin_new_coin_subscriptions_for_project(
    pool: Option<Pool<MySql>>,
    project_id: u64,
    query: AdminNewCoinScopedListQuery,
) -> AppResult<NewCoinSubscriptionsResponse> {
    let query = build_new_coin_scoped_list_query(project_id, query);
    list_admin_new_coin_subscriptions(pool, query).await
}

pub(crate) async fn list_admin_new_coin_distributions(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinFlatListQuery,
) -> AppResult<NewCoinDistributionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let distributions = list_admin_new_coin_distributions_from_store(
        &pool,
        AdminNewCoinFlatListFilter {
            project_id: query.project_id,
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinDistributionsResponse { distributions })
}

/// 查询某个项目的分配列表。
pub(crate) async fn list_admin_new_coin_distributions_for_project(
    pool: Option<Pool<MySql>>,
    project_id: u64,
    query: AdminNewCoinScopedListQuery,
) -> AppResult<NewCoinDistributionsResponse> {
    let query = build_new_coin_scoped_list_query(project_id, query);
    list_admin_new_coin_distributions(pool, query).await
}

pub(crate) async fn list_admin_new_coin_purchases(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinPurchaseQuery,
) -> AppResult<NewCoinPurchasesResponse> {
    let pool = admin_mysql_pool(pool)?;
    let purchases = list_admin_new_coin_purchases_from_store(
        &pool,
        AdminNewCoinFlatListFilter {
            project_id: query.project_id,
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinPurchasesResponse { purchases })
}

pub(crate) async fn list_admin_new_coin_lock_positions(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinLockPositionQuery,
) -> AppResult<NewCoinLockPositionsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let lock_positions = list_admin_new_coin_lock_positions_from_store(
        &pool,
        AdminNewCoinLockPositionListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinLockPositionsResponse { lock_positions })
}

pub(crate) async fn list_admin_new_coin_unlocks(
    pool: Option<Pool<MySql>>,
    query: AdminNewCoinUnlockQuery,
) -> AppResult<NewCoinUnlocksResponse> {
    let pool = admin_mysql_pool(pool)?;
    let unlocks = list_admin_new_coin_unlocks_from_store(
        &pool,
        AdminNewCoinUnlockListFilter {
            user_id: query.user_id,
            email: query.email,
            asset_id: query.asset_id,
            status: query.status.and_then(optional_string),
            fee_paid_status: query.fee_paid_status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(NewCoinUnlocksResponse { unlocks })
}

pub(crate) async fn create_admin_new_coin_project(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateNewCoinProjectRequest,
) -> AppResult<NewCoinProjectResponse> {
    validate_create_new_coin_project(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 新币项目创建、生命周期事件和后台审计必须同事务提交，避免项目已开放但缺少追踪记录。
    let mut tx = pool.begin().await?;
    let project_id = insert_admin_new_coin_project_in_tx(
        &mut tx,
        AdminNewCoinProjectInsert {
            asset_id: request.asset_id,
            symbol: request.symbol.trim().to_owned(),
            lifecycle_status: request.lifecycle_status.trim().to_owned(),
            total_supply: request.total_supply,
            issue_price: request.issue_price,
            listed_at: request.listed_at,
            unlock_type: request.unlock_type.trim().to_owned(),
            fixed_unlock_at: request.fixed_unlock_at,
            relative_unlock_seconds: request.relative_unlock_seconds,
            unlock_fee_enabled: request.unlock_fee_enabled.unwrap_or(false),
            unlock_fee_rate: request.unlock_fee_rate,
            unlock_fee_basis: request
                .unlock_fee_basis
                .as_deref()
                .map(str::trim)
                .map(str::to_owned),
            unlock_fee_asset: request.unlock_fee_asset,
        },
    )
    .await?;
    let project = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    let event_payload = new_coin_project_audit_json(&project);
    insert_admin_new_coin_lifecycle_event_in_tx(
        &mut tx,
        project.id,
        "new_coin_project.create",
        event_payload.clone(),
        admin_id,
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "new_coin_project.create",
            target_type: "new_coin_project",
            target_id: project.id,
            before_json: None,
            after_json: Some(event_payload),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(project)
}

pub(crate) async fn update_admin_new_coin_lifecycle(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: UpdateNewCoinLifecycleRequest,
) -> AppResult<NewCoinProjectResponse> {
    let target_status = parse_lifecycle_status_from_request(&request.lifecycle_status)?;
    let pool = admin_mysql_pool(pool)?;

    // 生命周期流转必须先锁定项目行，再校验当前状态到目标状态的单向流转规则。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    let current_status = parse_lifecycle_status_from_db(&before.lifecycle_status)?;
    current_status
        .transition_to(target_status)
        .map_err(|_| AppError::Validation("invalid new coin lifecycle transition".to_owned()))?;
    let listed_at = if target_status == LifecycleStatus::Listed {
        Some(request.listed_at.unwrap_or_else(Utc::now))
    } else {
        before.listed_at
    };
    update_admin_new_coin_project_lifecycle_in_tx(
        &mut tx,
        project_id,
        lifecycle_status_value(target_status),
        listed_at,
    )
    .await?;
    let after = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    record_admin_new_coin_project_change_in_tx(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.lifecycle.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_new_coin_unlock_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: UpdateNewCoinUnlockRuleRequest,
) -> AppResult<NewCoinProjectResponse> {
    validate_update_new_coin_unlock_rule(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 锁定项目后再更新规则，避免后台并发修改导致审计 before/after 失真。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    let unlock_type = request.unlock_type.trim().to_owned();
    let listed_at = if unlock_type == "immediate_on_listing" {
        request.listed_at
    } else {
        before.listed_at
    };
    update_admin_new_coin_project_unlock_rule_in_tx(
        &mut tx,
        project_id,
        AdminNewCoinUnlockRuleUpdate {
            unlock_type,
            listed_at,
            fixed_unlock_at: request.fixed_unlock_at,
            relative_unlock_seconds: request.relative_unlock_seconds,
        },
    )
    .await?;
    let after = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    record_admin_new_coin_project_change_in_tx(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.unlock_rule.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_new_coin_unlock_fee_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: UpdateNewCoinUnlockFeeRuleRequest,
) -> AppResult<NewCoinProjectResponse> {
    validate_update_new_coin_unlock_fee_rule(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 矿工费关闭时同步清空费率、计费依据和费用资产，避免旧配置被后续解禁误用。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    update_admin_new_coin_project_unlock_fee_rule_in_tx(
        &mut tx,
        project_id,
        AdminNewCoinUnlockFeeRuleUpdate {
            unlock_fee_enabled: request.unlock_fee_enabled,
            unlock_fee_rate: request
                .unlock_fee_enabled
                .then_some(request.unlock_fee_rate)
                .flatten(),
            unlock_fee_basis: if request.unlock_fee_enabled {
                request
                    .unlock_fee_basis
                    .as_deref()
                    .map(str::trim)
                    .map(str::to_owned)
            } else {
                None
            },
            unlock_fee_asset: request
                .unlock_fee_enabled
                .then_some(request.unlock_fee_asset)
                .flatten(),
        },
    )
    .await?;
    let after = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    record_admin_new_coin_project_change_in_tx(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.unlock_fee_rule.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn update_admin_new_coin_post_listing_purchase(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: UpdateNewCoinPostListingPurchaseRequest,
) -> AppResult<NewCoinProjectResponse> {
    validate_update_new_coin_post_listing_purchase(&request)?;
    let pool = admin_mysql_pool(pool)?;

    // 锁定新币项目和目标交易对，确保认购开关、交易对启用和审计一致提交。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    ensure_post_listing_purchase_lifecycle(&before)?;
    if request.enabled {
        let pair_id = request.pair_id.ok_or_else(|| {
            AppError::Validation(
                "pair_id is required when post-listing purchase is enabled".to_owned(),
            )
        })?;
        ensure_admin_new_coin_post_listing_pair_in_tx(&mut tx, pair_id, before.asset_id).await?;
        activate_admin_new_coin_post_listing_pair_in_tx(&mut tx, pair_id).await?;
        enable_admin_new_coin_post_listing_purchase_in_tx(&mut tx, project_id, pair_id).await?;
    } else {
        disable_admin_new_coin_post_listing_purchase_in_tx(&mut tx, project_id).await?;
    }
    let after = load_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    record_admin_new_coin_project_change_in_tx(
        &mut tx,
        admin_id,
        project_id,
        "new_coin_project.post_listing_purchase.update",
        &before,
        &after,
        request.reason,
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn distribute_admin_new_coin(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    project_id: u64,
    request: DistributeNewCoinRequest,
) -> AppResult<NewCoinDistributionResponse> {
    validate_distribute_new_coin(&request)?;
    let idempotency_key = request.idempotency_key.trim().to_owned();
    let pool = admin_mysql_pool(pool)?;

    // 派发会同时影响申购单、钱包余额、锁仓明细、生命周期事件和后台审计，必须放入同一事务。
    let mut tx = pool.begin().await?;
    let project = lock_admin_new_coin_project_in_tx(&mut tx, project_id).await?;
    ensure_distribution_lifecycle(&project)?;
    if admin_new_coin_idempotency_key_exists_in_tx(
        &mut tx,
        "new_coin_distributions",
        &idempotency_key,
    )
    .await?
    {
        return Err(AppError::Conflict(
            "new coin distribution has already been created".to_owned(),
        ));
    }
    if let Some(subscription_id) = request.subscription_id {
        apply_admin_new_coin_subscription_distribution_in_tx(
            &mut tx,
            subscription_id,
            project_id,
            request.user_id,
            &request.quantity,
        )
        .await?;
    }

    let source_time = Utc::now();
    let lock_positions = lock_positions_for_distribution(
        &project,
        request.user_id,
        project.asset_id,
        &idempotency_key,
        request.quantity.clone(),
        source_time,
    )?;
    let lock_position_id = apply_admin_new_coin_distribution_allocation_in_tx(
        &mut tx,
        request.user_id,
        project.asset_id,
        &request.quantity,
        &lock_positions,
        AdminNewCoinLedgerWrite {
            change_type: "new_coin_distribution_lock",
            ref_type: "new_coin_distribution",
            ref_id: &idempotency_key,
        },
    )
    .await?;
    let status = if lock_position_id.is_some() {
        "locked"
    } else {
        "completed"
    };
    let distribution_id = insert_admin_new_coin_distribution_in_tx(
        &mut tx,
        project_id,
        request.user_id,
        request.subscription_id,
        project.asset_id,
        &request.quantity,
        lock_position_id,
        status,
        &idempotency_key,
    )
    .await?;
    let distribution = load_admin_new_coin_distribution_in_tx(&mut tx, distribution_id).await?;
    let distribution_json = new_coin_distribution_audit_json(&distribution);
    insert_admin_new_coin_lifecycle_event_in_tx(
        &mut tx,
        project_id,
        "new_coin_distribution.create",
        json!({ "distribution": distribution_json.clone() }),
        admin_id,
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "new_coin_distribution.create",
            target_type: "new_coin_distribution",
            target_id: distribution.id,
            before_json: None,
            after_json: Some(distribution_json),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(distribution)
}

pub(crate) async fn upsert_admin_new_coin_convert_rule(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: UpsertNewCoinConvertRuleRequest,
) -> AppResult<NewCoinConvertRuleResponse> {
    validate_new_coin_convert_rule(&request)?;
    let status = request
        .status
        .clone()
        .and_then(optional_string)
        .unwrap_or_else(|| "active".to_owned());
    let pool = admin_mysql_pool(pool)?;

    // 同一 convert_pair 只允许一条新币兑换规则，先按 pair 锁定旧记录再 upsert。
    let mut tx = pool.begin().await?;
    let before = lock_admin_new_coin_convert_rule_in_tx(&mut tx, request.convert_pair_id).await?;
    let write = AdminNewCoinConvertRuleWrite {
        convert_pair_id: request.convert_pair_id,
        rate_source: request.rate_source.trim().to_owned(),
        fixed_rate: request.fixed_rate,
        floating_rate_json: request.floating_rate_json,
        status,
        admin_id,
    };
    let rule_id = if let Some(before) = before.as_ref() {
        update_admin_new_coin_convert_rule_in_tx(&mut tx, before.id, &write).await?;
        before.id
    } else {
        insert_admin_new_coin_convert_rule_in_tx(&mut tx, &write).await?
    };
    let after = load_admin_new_coin_convert_rule_in_tx(&mut tx, rule_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: if before.is_some() {
                "new_coin_convert_rule.update"
            } else {
                "new_coin_convert_rule.create"
            },
            target_type: "new_coin_convert_rule",
            target_id: after.id,
            before_json: before.as_ref().map(new_coin_convert_rule_audit_json),
            after_json: Some(new_coin_convert_rule_audit_json(&after)),
            reason: request.reason,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

async fn record_admin_new_coin_project_change_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    project_id: u64,
    action: &'static str,
    before: &NewCoinProjectResponse,
    after: &NewCoinProjectResponse,
    reason: Option<String>,
) -> AppResult<()> {
    let before_json = new_coin_project_audit_json(before);
    let after_json = new_coin_project_audit_json(after);
    insert_admin_new_coin_lifecycle_event_in_tx(
        tx,
        project_id,
        action,
        json!({
            "before": before_json,
            "after": after_json,
        }),
        admin_id,
    )
    .await?;
    insert_admin_audit_log_entry_in_tx(
        tx,
        admin_id,
        AdminAuditLogEntry {
            action,
            target_type: "new_coin_project",
            target_id: project_id,
            before_json: Some(before_json),
            after_json: Some(after_json),
            reason,
        },
    )
    .await
}

pub(crate) async fn list_admin_convert_pairs(
    pool: Option<Pool<MySql>>,
    query: AdminConvertPairQuery,
) -> AppResult<ConvertPairsResponse> {
    let pool = admin_mysql_pool(pool)?;
    let pairs = list_admin_convert_pairs_from_store(&pool, route_limit(query.limit)).await?;
    Ok(ConvertPairsResponse { pairs })
}

pub(crate) async fn get_admin_convert_pair(
    pool: Option<Pool<MySql>>,
    pair_id: u64,
) -> AppResult<ConvertPairResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_convert_pair_from_store(&pool, pair_id).await
}

pub(crate) async fn list_admin_convert_orders(
    pool: Option<Pool<MySql>>,
    query: AdminConvertOrdersQuery,
) -> AppResult<ConvertOrdersResponse> {
    let pool = admin_mysql_pool(pool)?;
    let orders = list_admin_convert_orders_from_store(
        &pool,
        AdminConvertOrderListFilter {
            user_id: query.user_id,
            email: query.email,
            status: query.status.and_then(optional_string),
            limit: route_limit(query.limit),
        },
    )
    .await?;
    Ok(ConvertOrdersResponse { orders })
}

pub(crate) async fn get_admin_convert_order(
    pool: Option<Pool<MySql>>,
    order_id: u64,
) -> AppResult<ConvertOrderResponse> {
    let pool = admin_mysql_pool(pool)?;
    load_admin_convert_order_from_store(&pool, order_id).await
}

pub(crate) async fn create_admin_convert_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    request: CreateConvertPairRequest,
) -> AppResult<ConvertPairResponse> {
    validate_create_convert_pair(&request)?;
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;
    let enabled = request.enabled.unwrap_or(true);
    let fee_rate = request
        .fee_rate
        .clone()
        .unwrap_or_else(|| BigDecimal::from(0));
    let target_min_amount = request
        .target_min_amount
        .clone()
        .unwrap_or_else(|| request.min_amount.clone());
    let target_max_amount = request
        .target_max_amount
        .clone()
        .or_else(|| request.max_amount.clone());

    // 换币交易对写入和后台审计同事务提交，避免配置生效但缺少可追溯记录。
    let mut tx = pool.begin().await?;
    let pair_id = insert_admin_convert_pair_in_tx(
        &mut tx,
        AdminConvertPairInsert {
            from_asset_id: request.from_asset_id,
            to_asset_id: request.to_asset_id,
            pricing_mode: request.pricing_mode.trim().to_owned(),
            spread_rate: request.spread_rate,
            fee_rate,
            min_amount: request.min_amount,
            max_amount: request.max_amount,
            target_min_amount,
            target_max_amount,
            enabled,
        },
    )
    .await?;
    let pair = load_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "convert_pair.create",
            target_type: "convert_pair",
            target_id: pair.id,
            before_json: None,
            after_json: Some(convert_pair_audit_json(&pair)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(pair)
}

pub(crate) async fn update_admin_convert_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: UpdateConvertPairRequest,
) -> AppResult<ConvertPairResponse> {
    let reason = required_admin_audit_reason(request.reason.clone())?;
    let pool = admin_mysql_pool(pool)?;

    // 先锁定旧配置，再按请求字段合并新配置，确保审计 before/after 对应同一次写入。
    let mut tx = pool.begin().await?;
    let before = lock_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    let from_asset_id = request.from_asset_id.unwrap_or(before.from_asset_id);
    let to_asset_id = request.to_asset_id.unwrap_or(before.to_asset_id);
    let pricing_mode = request
        .pricing_mode
        .as_deref()
        .unwrap_or(&before.pricing_mode)
        .trim()
        .to_owned();
    let spread_rate = request
        .spread_rate
        .clone()
        .unwrap_or_else(|| before.spread_rate.clone());
    let fee_rate = request
        .fee_rate
        .clone()
        .unwrap_or_else(|| before.fee_rate.clone());
    let min_amount = request
        .min_amount
        .clone()
        .unwrap_or_else(|| before.min_amount.clone());
    let max_amount = request
        .max_amount
        .clone()
        .unwrap_or_else(|| before.max_amount.clone());
    let target_min_amount = request
        .target_min_amount
        .clone()
        .unwrap_or_else(|| before.target_min_amount.clone());
    let target_max_amount = request
        .target_max_amount
        .clone()
        .unwrap_or_else(|| before.target_max_amount.clone());
    let enabled = request.enabled.unwrap_or(before.enabled);
    let updates_config = request.from_asset_id.is_some()
        || request.to_asset_id.is_some()
        || request.pricing_mode.is_some()
        || request.spread_rate.is_some()
        || request.fee_rate.is_some()
        || request.min_amount.is_some()
        || request.max_amount.is_some()
        || request.target_min_amount.is_some()
        || request.target_max_amount.is_some();

    validate_convert_pair_values(
        from_asset_id,
        to_asset_id,
        &pricing_mode,
        &spread_rate,
        &fee_rate,
        &min_amount,
        max_amount.as_ref(),
        &target_min_amount,
        target_max_amount.as_ref(),
    )?;

    update_admin_convert_pair_in_tx(
        &mut tx,
        pair_id,
        AdminConvertPairUpdate {
            from_asset_id,
            to_asset_id,
            pricing_mode,
            spread_rate,
            fee_rate,
            min_amount,
            max_amount,
            target_min_amount,
            target_max_amount,
            enabled,
        },
    )
    .await?;
    let after = load_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: if updates_config {
                "convert_pair.update"
            } else {
                "convert_pair.update_status"
            },
            target_type: "convert_pair",
            target_id: pair_id,
            before_json: Some(convert_pair_audit_json(&before)),
            after_json: Some(convert_pair_audit_json(&after)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}

pub(crate) async fn delete_admin_convert_pair(
    pool: Option<Pool<MySql>>,
    admin_id: u64,
    pair_id: u64,
    request: DeleteConvertPairRequest,
) -> AppResult<()> {
    let reason = required_admin_audit_reason(request.reason)?;
    let pool = admin_mysql_pool(pool)?;

    // 删除前锁定交易对并确认无报价、订单和新币兑换规则引用，避免悬挂外键语义。
    let mut tx = pool.begin().await?;
    let before = lock_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    if before.enabled {
        return Err(AppError::Validation(
            "convert pair must be disabled before deletion".to_owned(),
        ));
    }
    ensure_convert_pair_has_no_references_in_tx(&mut tx, pair_id).await?;
    delete_admin_convert_pair_in_tx(&mut tx, pair_id).await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "convert_pair.delete",
            target_type: "convert_pair",
            target_id: pair_id,
            before_json: Some(convert_pair_audit_json(&before)),
            after_json: None,
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

fn encrypt_optional_upload_secret(
    key: Option<&str>,
    new_value: Option<&str>,
    existing_ciphertext: Option<String>,
) -> AppResult<Option<String>> {
    if new_value.and_then(optional_str).is_some() {
        let key = key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?;
        encrypt_secret_field(key, new_value, existing_ciphertext)
    } else {
        Ok(existing_ciphertext)
    }
}

fn prepare_market_source_credential_secret_fields(
    request: &UpsertMarketSourceCredentialRequest,
    before: Option<&AdminMarketSourceCredentialRecord>,
    auth_type: &str,
    key: Option<&str>,
) -> AppResult<(
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
)> {
    if auth_type != MARKET_SOURCE_AUTH_TYPE_API_KEY {
        return Ok((None, None, None, None));
    }

    let key = key.ok_or_else(|| {
        AppError::Internal("credential encryption key is not configured".to_owned())
    })?;
    let api_key_ciphertext = encrypt_secret_field(
        key,
        request.api_key.as_deref(),
        before.and_then(|record| record.api_key_ciphertext.clone()),
    )?;
    let api_secret_ciphertext = encrypt_secret_field(
        key,
        request.api_secret.as_deref(),
        before.and_then(|record| record.api_secret_ciphertext.clone()),
    )?;
    let passphrase_ciphertext = encrypt_secret_field(
        key,
        request.passphrase.as_deref(),
        before.and_then(|record| record.passphrase_ciphertext.clone()),
    )?;
    let api_key_mask = request
        .api_key
        .as_deref()
        .map(mask_secret)
        .or_else(|| before.and_then(|record| record.api_key_mask.clone()));

    Ok((
        api_key_ciphertext,
        api_secret_ciphertext,
        passphrase_ciphertext,
        api_key_mask,
    ))
}

async fn insert_admin_market_feed_reload_audit(
    pool: &Pool<MySql>,
    admin_id: u64,
    config: &MarketFeedConfigResponse,
    runtime: &MarketFeedRuntimeStatus,
    reason: String,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    insert_admin_audit_log_entry_in_tx(
        &mut tx,
        admin_id,
        AdminAuditLogEntry {
            action: "market_feed_config.reload",
            target_type: "market_feed_config",
            target_id: config.id,
            before_json: None,
            after_json: Some(market_feed_reload_audit_json(config, runtime)),
            reason: Some(reason),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

fn prepare_smtp_secret_fields(
    request: &SaveSmtpConfigRequest,
    before: Option<&AdminSmtpConfigRecord>,
    key: Option<&str>,
) -> AppResult<(Option<String>, Option<String>, Option<String>)> {
    let needs_key = smtp_request_has_new_secret(request);
    let key = if needs_key {
        Some(key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?)
    } else {
        key
    };

    let username_ciphertext = match key {
        Some(key) => encrypt_secret_field(
            key,
            request.username.as_deref(),
            before.and_then(|record| record.username_ciphertext.clone()),
        )?,
        None => before.and_then(|record| record.username_ciphertext.clone()),
    };
    let password_ciphertext = match key {
        Some(key) => encrypt_secret_field(
            key,
            request.password.as_deref(),
            before.and_then(|record| record.password_ciphertext.clone()),
        )?,
        None => before.and_then(|record| record.password_ciphertext.clone()),
    };
    if username_ciphertext.is_some() != password_ciphertext.is_some() {
        return Err(AppError::Validation(
            "smtp username and password must be configured together".to_owned(),
        ));
    }
    let username_mask = request
        .username
        .as_deref()
        .and_then(optional_str)
        .map(mask_secret)
        .or_else(|| before.and_then(|record| record.username_mask.clone()));

    Ok((username_ciphertext, password_ciphertext, username_mask))
}

fn smtp_config_write(
    config: SmtpValidatedConfig,
    username_ciphertext: Option<String>,
    password_ciphertext: Option<String>,
    username_mask: Option<String>,
    admin_id: u64,
) -> AdminSmtpConfigWrite {
    AdminSmtpConfigWrite {
        name: config.name,
        host: config.host,
        port: config.port,
        security: config.security,
        username_ciphertext,
        password_ciphertext,
        username_mask,
        from_email: config.from_email,
        from_name: config.from_name,
        verification_code_template_html: config.verification_code_template_html,
        verification_code_templates: config.verification_code_templates,
        enabled: config.enabled,
        priority: config.priority,
        updated_by: admin_id,
    }
}

fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0)
}

fn validate_loaded_market_feed_credentials(
    providers: &[String],
    credentials: &[MarketSourceCredentialSecret],
) -> AppResult<()> {
    for provider in providers {
        let missing_api_key = credentials
            .iter()
            .find(|credential| credential.provider == *provider)
            .is_some_and(|credential| {
                credential.auth_type == "api_key"
                    && credential.api_key.as_deref().unwrap_or("").is_empty()
            });
        if missing_api_key {
            return Err(AppError::Validation(format!(
                "market feed provider {provider} api_key is required"
            )));
        }
    }
    Ok(())
}

fn optional_string(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn optional_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

pub(crate) fn admin_mysql_pool(pool: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for admin convert routes".to_owned())
    })
}

/// 从应用状态中获取 admin 路由使用的 MySQL 连接池。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    admin_mysql_pool(state.mysql.clone())
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_admin_application_tests.rs"]
mod tests;
