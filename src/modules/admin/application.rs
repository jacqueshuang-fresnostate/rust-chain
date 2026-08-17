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
            AdminMarketStrategyListFilter, AdminMarketStrategyRecoveryJobClaim,
            AdminMarketStrategyRecoveryJobInsert, AdminMarketStrategyRecoveryJobListFilter,
            AdminMarketStrategyUpdate, AdminNewCoinConvertRuleWrite, AdminNewCoinFlatListFilter,
            AdminNewCoinLockPositionListFilter, AdminNewCoinProjectInsert,
            AdminNewCoinUnlockFeeRuleUpdate, AdminNewCoinUnlockListFilter,
            AdminNewCoinUnlockRuleUpdate, AdminNewsInsert, AdminNewsListFilter,
            AdminNewsStatusUpdate, AdminNewsUpdate, AdminRiskEventListFilter,
            AdminRiskRuleListFilter, AdminSyntheticStrategySnapshot, AdminTradingPairInsert,
            AdminTradingPairListFilter, AdminTradingPairUpdate, AdminUserInsert,
            AdminUserListFilter, AdminWalletAccountListFilter, AdminWalletLedgerListFilter,
            activate_admin_new_coin_post_listing_pair_in_tx,
            admin_new_coin_idempotency_key_exists_in_tx, admin_smtp_config_name_exists_except,
            admin_smtp_email_config, admin_upload_config_response,
            apply_admin_new_coin_distribution_allocation_in_tx,
            apply_admin_new_coin_subscription_distribution_in_tx,
            claim_market_strategy_recovery_job, complete_market_strategy_recovery_job_in_tx,
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
            fail_market_strategy_recovery_job_in_tx, finalize_admin_agent_hierarchy_in_tx,
            insert_admin_agent_in_tx, insert_admin_asset_in_tx, insert_admin_audit_log_entry_in_tx,
            insert_admin_convert_pair_in_tx, insert_admin_country_in_tx,
            insert_admin_deposit_network_config_in_tx, insert_admin_market_strategy_in_tx,
            insert_admin_new_coin_convert_rule_in_tx, insert_admin_new_coin_distribution_in_tx,
            insert_admin_new_coin_lifecycle_event_in_tx, insert_admin_new_coin_project_in_tx,
            insert_admin_news_item_in_tx, insert_admin_smtp_config_in_tx,
            insert_admin_trading_pair_in_tx, insert_admin_upload_object, insert_admin_user_in_tx,
            insert_agent_admin_user_in_tx, insert_agent_commission_rule_in_tx,
            insert_deposit_address_pool_in_tx, insert_market_strategy_event_in_tx,
            insert_market_strategy_nodes_in_tx, insert_market_strategy_recovery_job_in_tx,
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
            list_existing_one_minute_open_times, list_market_strategy_nodes_from_store,
            list_market_strategy_nodes_in_tx, list_market_strategy_recovery_jobs_from_store,
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
            load_admin_market_strategy_from_store, load_admin_market_strategy_in_tx,
            load_admin_new_coin_convert_rule_in_tx, load_admin_new_coin_distribution_in_tx,
            load_admin_new_coin_project_in_tx,
            load_admin_news_item as load_admin_news_item_from_store, load_admin_news_item_in_tx,
            load_admin_smtp_config as load_admin_smtp_config_from_store,
            load_admin_smtp_config_by_id, load_admin_smtp_config_by_id_in_tx,
            load_admin_smtp_config_by_name_in_tx, load_admin_smtp_config_for_delivery,
            load_admin_smtp_delivery_settings, load_admin_synthetic_strategy_snapshot,
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
            load_market_strategy_recovery_job_by_token_hash,
            load_market_strategy_recovery_job_from_store, load_market_strategy_recovery_job_in_tx,
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
            replace_market_strategy_nodes_in_tx, reset_admin_user_two_factor_in_tx,
            save_admin_security_policy_in_tx, update_admin_agent_status_in_tx,
            update_admin_asset_in_tx, update_admin_convert_pair_in_tx, update_admin_country_in_tx,
            update_admin_country_status_in_tx, update_admin_deposit_network_config_in_tx,
            update_admin_market_strategy_in_tx, update_admin_new_coin_convert_rule_in_tx,
            update_admin_new_coin_project_lifecycle_in_tx,
            update_admin_new_coin_project_unlock_fee_rule_in_tx,
            update_admin_new_coin_project_unlock_rule_in_tx, update_admin_news_item_in_tx,
            update_admin_news_status_in_tx, update_admin_smtp_config_in_tx,
            update_admin_trading_pair_in_tx, update_admin_trading_pair_status_in_tx,
            update_admin_user_status_in_tx, update_agent_admin_users_status_in_tx,
            update_agent_commission_rule_in_tx, update_agent_commission_status_in_tx,
            update_deposit_address_pool_in_tx, update_market_strategy_run_checkpoint_in_tx,
            update_market_strategy_run_status_in_tx, update_market_strategy_status_in_tx,
            update_risk_rule_status_in_tx, upload_admin_file_to_storage,
            upsert_admin_market_feed_config_in_tx, upsert_admin_market_source_credential_in_tx,
            upsert_admin_smtp_delivery_settings_in_tx, upsert_admin_upload_config_in_tx,
            upsert_default_admin_smtp_config_in_tx, upsert_user_agent_referral_in_tx,
        },
        presentation::{
            AdminAgentCommissionBatchStatusItemResponse, AdminAgentCommissionBatchStatusResponse,
            AdminAgentCommissionQuery, AdminAgentCommissionResponse, AdminAgentCommissionRuleQuery,
            AdminAgentCommissionRuleResponse, AdminAgentCommissionRulesResponse,
            AdminAgentCommissionsResponse, AdminAgentPasswordResetResponse, AdminAgentQuery,
            AdminAgentResponse, AdminAgentUsersQuery, AdminAgentUsersResponse, AdminAgentsResponse,
            AdminAssetQuery, AdminAssetResponse, AdminAssetsResponse, AdminAuditLogsQuery,
            AdminAuditLogsResponse, AdminConvertOrdersQuery, AdminConvertPairQuery,
            AdminCountriesQuery, AdminCountriesResponse, AdminCountryResponse,
            AdminDashboardAuditSummary, AdminDashboardMarketSummary, AdminDashboardResponse,
            AdminDepositAddressPoolBatchResponse, AdminDepositAddressPoolQuery,
            AdminDepositAddressPoolResponse, AdminDepositAddressPoolResponseList,
            AdminDepositNetworkConfigQuery, AdminDepositNetworkConfigResponse,
            AdminDepositNetworkConfigResponseList, AdminKycSubmissionQuery,
            AdminMarginLiquidationQuery, AdminMarginLiquidationResponse,
            AdminMarginLiquidationsResponse, AdminMarketStrategiesResponse,
            AdminMarketStrategyDetailResponse, AdminMarketStrategyNodeResponse,
            AdminMarketStrategyQuery, AdminMarketStrategyResponse, AdminNewCoinFlatListQuery,
            AdminNewCoinLockPositionQuery, AdminNewCoinProjectQuery, AdminNewCoinPurchaseQuery,
            AdminNewCoinScopedListQuery, AdminNewCoinUnlockQuery, AdminNewsItemResponse,
            AdminNewsItemsResponse, AdminNewsQuery, AdminRiskEventQuery, AdminRiskRuleQuery,
            AdminTradingPairQuery, AdminTradingPairResponse, AdminTradingPairsResponse,
            AdminUserQuery, AdminUserRechargeRequest, AdminUserRechargeResponse,
            AdminUserReferralResponse, AdminUserResponse, AdminUserTwoFactorResetResponse,
            AdminUsersResponse, AdminWalletAccountQuery, AdminWalletAccountsResponse,
            AdminWalletLedgerQuery, AdminWalletLedgerResponseList, AssignUserAgentRequest,
            BatchUpdateAgentCommissionStatusRequest, ConvertOrderResponse, ConvertOrdersResponse,
            ConvertPairResponse, ConvertPairsResponse, CreateAdminCountryRequest,
            CreateAdminNewsItemRequest, CreateAdminUserRequest, CreateAgentCommissionRuleRequest,
            CreateAgentRequest, CreateAssetRequest, CreateConvertPairRequest,
            CreateDepositAddressPoolBatchRequest, CreateDepositAddressPoolRequest,
            CreateDepositNetworkConfigRequest, CreateMarketStrategyRequest,
            CreateNewCoinProjectRequest, CreateRiskRuleRequest, CreateTradingPairRequest,
            DeleteAssetRequest, DeleteConvertPairRequest, DistributeNewCoinRequest,
            ExecuteMarketStrategyRecoveryRequest, MarketFeedConfigResponse,
            MarketFeedStatusResponse, MarketSourceCredentialResponse, MarketSourceCredentialSecret,
            MarketSourceCredentialsResponse, MarketStrategyGapQuery, MarketStrategyGapsResponse,
            MarketStrategyRecoveryJobResponse, MarketStrategyRecoveryJobsQuery,
            MarketStrategyRecoveryJobsResponse, MarketStrategyRecoveryPreviewResponse,
            MarketStrategyRecoverySampleResponse, NewCoinConvertRuleResponse,
            NewCoinDistributionResponse, NewCoinDistributionsResponse,
            NewCoinLockPositionsResponse, NewCoinProjectResponse, NewCoinProjectsResponse,
            NewCoinPurchasesResponse, NewCoinSubscriptionsResponse, NewCoinUnlocksResponse,
            PreviewMarketStrategyRecoveryRequest, ReclaimDepositAddressPoolRequest,
            ReloadMarketFeedRequest, ReloadMarketFeedResponse, ResetAgentPasswordRequest,
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
            UpdateUserStatusRequest, UploadConfigResponse, UploadFileInput, UploadImageResponse,
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
            MARKET_SOURCE_AUTH_TYPE_API_KEY, MarketStrategyPreviewTokenClaims,
            MarketStrategyPreviewTokenInput, SMTP_DELIVERY_SETTINGS_ID, SmtpValidatedConfig,
            admin_news_item_audit_json, agent_admin_password_hash, agent_audit_json,
            agent_commission_audit_json, agent_commission_rule_audit_json, agent_password_hash,
            agent_password_reset_audit_json, asset_audit_json, convert_pair_audit_json,
            country_config_audit_json, deposit_address_pool_audit_json,
            deposit_network_config_audit_json, ensure_deposit_asset_symbols_allowed_by_network,
            ensure_distribution_lifecycle, ensure_post_listing_purchase_lifecycle,
            hash_admin_user_password, issue_market_strategy_preview_token, lifecycle_status_value,
            lock_positions_for_distribution, market_feed_config_audit_json,
            market_feed_config_response, market_feed_reload_audit_json,
            market_feed_runtime_config_from_response, market_source_credential_audit_json,
            market_source_credential_response, market_source_credential_target_id,
            market_strategy_audit_json, market_strategy_config_json,
            market_strategy_recovery_aggregate_intervals, market_strategy_run_status,
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
            smtp_request_has_new_secret, summarize_market_strategy_gaps, trading_pair_audit_json,
            two_factor_audit_json, upload_config_audit_json,
            upload_config_secret_destination_unchanged, user_audit_json, user_referral_audit_json,
            validate_address_group_code, validate_admin_user_recharge,
            validate_agent_commission_batch_ids, validate_agent_commission_rate,
            validate_agent_commission_rule_product_type, validate_agent_commission_rule_status,
            validate_agent_commission_status, validate_agent_status, validate_asset_fee_settings,
            validate_asset_name, validate_asset_status, validate_asset_type,
            validate_convert_pair_values, validate_country_code, validate_country_locale_config,
            validate_country_name, validate_country_remark, validate_country_status,
            validate_create_admin_user_request, validate_create_agent_request,
            validate_create_asset_request, validate_create_convert_pair,
            validate_create_market_strategy, validate_create_new_coin_project,
            validate_create_risk_rule, validate_create_trading_pair_request,
            validate_deposit_address, validate_deposit_address_assignable_status,
            validate_deposit_address_status, validate_deposit_network_config_status,
            validate_deposit_network_display_name, validate_distribute_new_coin,
            validate_market_feed_intervals, validate_market_feed_providers,
            validate_market_feed_reason, validate_market_feed_symbols,
            validate_market_source_auth_type, validate_market_strategy_recovery_job_status,
            validate_market_strategy_recovery_range, validate_market_strategy_status,
            validate_new_coin_convert_rule, validate_news_category, validate_news_content_document,
            validate_news_locale, validate_news_status, validate_news_title,
            validate_optional_image_url, validate_optional_length, validate_security_policy,
            validate_smtp_delivery_strategy, validate_smtp_email, validate_smtp_save_request,
            validate_trading_pair_market_type, validate_trading_pair_status,
            validate_update_asset_request, validate_update_market_strategy,
            validate_update_new_coin_post_listing_purchase,
            validate_update_new_coin_unlock_fee_rule, validate_update_new_coin_unlock_rule,
            validate_update_trading_pair_request, validate_upload_config, validate_user_status,
            verify_market_strategy_preview_token,
        },
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{MySql, Pool, Transaction};
use std::sync::Arc;
use uuid::Uuid;

use crate::modules::agent::domain::{agent_path, derive_agent_placement};
use crate::modules::agent::infrastructure::{
    revoke_agent_admin_refresh_tokens_in_tx as revoke_agent_admin_refresh_tokens_in_tx_from_agent,
    update_agent_admin_password_in_tx as update_agent_admin_password_in_tx_from_agent,
};
use crate::modules::auth::{
    ActorType, AuthActor, infrastructure::revoke_user_refresh_tokens_in_tx,
    revoke_actor_auth_sessions,
};
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

mod agents;
mod convert;
mod dashboard_audit;
mod margin;
mod market;
mod market_feed;
mod market_settings;
mod new_coin;
mod news;
mod risk_security;
mod system_config;
mod users;
mod wallet_assets;

pub(crate) use self::agents::*;
pub(crate) use self::convert::*;
pub(crate) use self::dashboard_audit::*;
pub(crate) use self::margin::*;
pub(crate) use self::market::*;
pub use self::market_feed::*;
pub(crate) use self::market_settings::*;
pub(crate) use self::new_coin::*;
pub(crate) use self::news::*;
pub(crate) use self::risk_security::*;
pub(crate) use self::system_config::*;
pub(crate) use self::users::*;
pub(crate) use self::wallet_assets::*;

#[derive(Debug)]
pub struct AdminCountryUseCases;

impl ApplicationLayer for AdminCountryUseCases {}

const MAX_ROUTE_OFFSET: u32 = 100_000;

/// 裁剪后台列表的每页条数：缺省 50，并强制收敛到 1 到 100 之间。
/// 采用夹取而非报错，因此前端传 0 或超大值都能拿到结果而不是校验失败，代价是分页参数写错时不会被察觉。
/// 该上限是所有后台列表共用的口径，避免单次查询拉走过多行。
fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 偏移同样设上限：超大 offset 会让日志类大表退化为全表扫描加文件排序。
/// 缺省为 0，超过十万的偏移被截到十万，因此深翻页会停在同一页而不是继续后移。
/// 与条数一样采用夹取而非报错，需要访问更靠后的数据时应改用筛选条件而非加大偏移。
fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(MAX_ROUTE_OFFSET)
}

/// 把字符串去空白后归一为可选值，纯空白折叠成 None 表示该筛选条件未提供。
/// 与服务层同名工具的差别是这里接收的是必有值的字符串而非 Option，供已解包的查询字段直接使用。
fn optional_string(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

/// 借用版的空白归一工具，去空白后为空则返回 None，避免为判空而额外分配字符串。
/// 用于只需读取而不需要持有结果的场景，语义与持有版完全一致。
fn optional_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

/// 从可选应用状态中取得后台用例所需的 MySQL 连接池句柄。
/// 连接池未配置时返回内部错误；本函数不执行查询、不加锁，也不创建事务。
pub(crate) fn admin_mysql_pool(pool: Option<Pool<MySql>>) -> AppResult<Pool<MySql>> {
    pool.ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for admin convert routes".to_owned())
    })
}

/// 从应用状态中取得 Admin 用例使用的 MySQL 连接池。
/// 缺少连接池时返回内部配置错误；本函数只提取句柄，不开启事务或执行查询。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    admin_mysql_pool(state.mysql.clone())
}

#[cfg(test)]
#[path = "../../../tests/unit_src/src_modules_admin_application_tests.rs"]
mod tests;
