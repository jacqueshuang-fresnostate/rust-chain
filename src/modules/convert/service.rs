//! convert bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。

use super::repository::{
    ConvertOrderRepository, ConvertPairRule, ConvertPairRuleDbRecord, ConvertQuoteRepository,
    WalletBalanceRecord,
};
use super::{
    ConfirmConvertQuoteCommand, ConvertConfirmationInsert, ConvertConfirmationResult, ConvertQuote,
    ConvertQuoteCacheEntry, ConvertQuoteCommand, ConvertQuoteConfirmationRecord,
    ConvertQuoteCreated, ConvertRepositoryError, ConvertServiceError, QuoteId,
};
use crate::{
    architecture::ServiceLayer,
    error::{AppError, AppResult},
    modules::wallet::{
        MAX_ASSET_PRECISION_SCALE, amount_fits_asset_precision, truncate_amount_to_asset_precision,
    },
};
use bigdecimal::BigDecimal;
use uuid::Uuid;

#[derive(Debug)]
pub struct ServiceLayerMarker;

impl ServiceLayer for ServiceLayerMarker {}

pub(crate) const QUOTE_TTL_SECONDS: i64 = 30;

#[derive(Debug, Clone)]
pub(crate) struct ConvertQuoteAmounts {
    pub(crate) to_amount: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
}

pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

pub(crate) fn parse_quote_id(value: &str) -> AppResult<QuoteId> {
    Uuid::parse_str(value)
        .map(QuoteId)
        .map_err(|_| AppError::Validation("invalid quote_id".to_owned()))
}

pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

pub(crate) fn optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub(crate) fn convert_pair_rule_from_record(
    row: ConvertPairRuleDbRecord,
    from_asset_id: u64,
    to_asset_id: u64,
) -> AppResult<ConvertPairRule> {
    let is_reverse = row.from_asset_id == to_asset_id && row.to_asset_id == from_asset_id;
    let fixed_rate = match (row.fixed_rate, is_reverse) {
        (Some(rate), true) => {
            if rate <= BigDecimal::from(0) {
                return Err(AppError::Validation(
                    "convert reverse quote requires positive fixed pricing rule".to_owned(),
                ));
            }
            Some(BigDecimal::from(1) / rate)
        }
        (rate, _) => rate,
    };
    let (min_amount, max_amount) = if is_reverse {
        (row.target_min_amount, row.target_max_amount)
    } else {
        (row.min_amount, row.max_amount)
    };

    Ok(ConvertPairRule {
        id: row.id,
        from_asset_id,
        to_asset_id,
        pricing_mode: row.pricing_mode,
        spread_rate: row.spread_rate,
        fee_rate: row.fee_rate,
        min_amount,
        max_amount,
        fixed_rate,
        market_pair_symbol: row.market_pair_symbol,
        market_base_asset_id: row.market_base_asset_id,
        market_quote_asset_id: row.market_quote_asset_id,
    })
}

pub(crate) fn validate_quote_amount(amount: &BigDecimal, pair: &ConvertPairRule) -> AppResult<()> {
    if amount <= &BigDecimal::from(0) {
        return Err(AppError::Validation(
            "convert amount must be positive".to_owned(),
        ));
    }

    let zero = BigDecimal::from(0);
    let one = BigDecimal::from(1);
    if pair.fee_rate < zero || pair.fee_rate >= one {
        return Err(AppError::Validation(
            "convert fee_rate must be greater than or equal to 0 and less than 1".to_owned(),
        ));
    }
    if amount < &pair.min_amount {
        return Err(AppError::Validation(
            "convert amount is below pair minimum".to_owned(),
        ));
    }
    if let Some(max_amount) = &pair.max_amount
        && amount > max_amount
    {
        return Err(AppError::Validation(
            "convert amount exceeds pair maximum".to_owned(),
        ));
    }
    if !matches!(pair.pricing_mode.as_str(), "fixed" | "market") {
        return Err(AppError::Validation(
            "unsupported convert pricing_mode".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn ensure_asset_precision_scale(precision_scale: i32) -> AppResult<()> {
    if !(0..=MAX_ASSET_PRECISION_SCALE).contains(&precision_scale) {
        return Err(AppError::Internal(format!(
            "asset precision_scale is outside supported range: {precision_scale}"
        )));
    }
    Ok(())
}

pub(crate) fn ensure_convert_amount_precision(
    amount: &BigDecimal,
    precision_scale: i32,
    field: &str,
) -> AppResult<()> {
    // 资产精度校验必须在落库前完成，避免 BigDecimal 细度超限导致后续账务差分难以复现。
    if amount_fits_asset_precision(amount, precision_scale) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "{field} exceeds asset precision_scale {precision_scale}"
        )))
    }
}

pub(crate) fn ensure_sufficient_convert_balance(
    amount: &BigDecimal,
    balance: &WalletBalanceRecord,
) -> AppResult<()> {
    if balance.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for convert: requested {}, available {}, locked {}",
            amount, balance.available, balance.locked
        )));
    }

    Ok(())
}

pub(crate) fn convert_quote_amounts(
    from_amount: &BigDecimal,
    pair: &ConvertPairRule,
    rate: &BigDecimal,
    from_precision_scale: i32,
    to_precision_scale: i32,
) -> AppResult<ConvertQuoteAmounts> {
    let effective_rate = rate.clone() * (BigDecimal::from(1) - pair.spread_rate.clone());
    let raw_fee_amount = from_amount.clone() * pair.fee_rate.clone();
    let fee_amount = truncate_amount_to_asset_precision(&raw_fee_amount, from_precision_scale);
    let net_from_amount = from_amount.clone() - fee_amount.clone();
    if net_from_amount <= BigDecimal::from(0) {
        return Err(AppError::Validation(
            "convert amount must be greater than fee amount".to_owned(),
        ));
    }
    let raw_to_amount = net_from_amount * effective_rate;
    let to_amount = truncate_amount_to_asset_precision(&raw_to_amount, to_precision_scale);
    if to_amount <= BigDecimal::from(0) {
        return Err(AppError::Validation(
            "convert quote amount must be positive".to_owned(),
        ));
    }

    Ok(ConvertQuoteAmounts {
        to_amount,
        fee_amount,
    })
}

pub(crate) fn resolve_fixed_convert_rate(pair: &ConvertPairRule) -> AppResult<BigDecimal> {
    pair.fixed_rate.clone().ok_or_else(|| {
        AppError::Validation("convert quote requires active fixed pricing rule".to_owned())
    })
}

pub(crate) fn convert_market_pricing_source(pair: &ConvertPairRule) -> AppResult<(&str, u64, u64)> {
    let symbol = pair.market_pair_symbol.as_deref().ok_or_else(|| {
        AppError::Validation("convert market pricing requires active trading pair".to_owned())
    })?;
    let market_base_asset_id = pair.market_base_asset_id.ok_or_else(|| {
        AppError::Validation("convert market pricing requires active trading pair".to_owned())
    })?;
    let market_quote_asset_id = pair.market_quote_asset_id.ok_or_else(|| {
        AppError::Validation("convert market pricing requires active trading pair".to_owned())
    })?;

    Ok((symbol, market_base_asset_id, market_quote_asset_id))
}

pub(crate) fn resolve_market_convert_rate(
    pair: &ConvertPairRule,
    market_price: BigDecimal,
    market_base_asset_id: u64,
    market_quote_asset_id: u64,
) -> AppResult<BigDecimal> {
    if pair.from_asset_id == market_base_asset_id && pair.to_asset_id == market_quote_asset_id {
        return Ok(market_price);
    }
    if pair.from_asset_id == market_quote_asset_id && pair.to_asset_id == market_base_asset_id {
        return Ok(BigDecimal::from(1) / market_price);
    }

    Err(AppError::Validation(
        "convert market pricing trading pair does not match convert assets".to_owned(),
    ))
}

pub(crate) fn map_convert_repository_error(error: ConvertRepositoryError) -> AppError {
    AppError::Internal(format!("{error:?}"))
}

#[derive(Debug, Clone)]
pub struct ConvertService<R> {
    repository: R,
}

impl<R> ConvertService<R>
where
    R: ConvertQuoteRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// 测试与复用场景需要能直接访问底层仓储快照（如检查幂等写入结果）。
    pub fn repository(&self) -> &R {
        &self.repository
    }

    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }

    pub fn into_repository(self) -> R {
        self.repository
    }

    pub fn create_quote(
        &mut self,
        command: ConvertQuoteCommand,
    ) -> Result<ConvertQuoteCreated, ConvertServiceError> {
        if command.balance.available < command.from_amount {
            return Err(ConvertServiceError::InsufficientAvailableBalance {
                asset_id: command.from_asset,
                requested: Box::new(command.from_amount),
                available: Box::new(command.balance.available),
                locked: Box::new(command.balance.locked),
            });
        }

        let quote = ConvertQuote::new(
            command.quote_id.clone(),
            command.created_at,
            command.ttl_seconds,
        )?;
        quote.ensure_not_expired(command.created_at)?;

        let entry = ConvertQuoteCacheEntry {
            quote_id: command.quote_id.clone(),
            user_id: command.user_id,
            from_asset: command.from_asset,
            to_asset: command.to_asset,
            from_amount: command.from_amount,
            to_amount: command.to_amount,
            fee_rate: BigDecimal::from(0),
            fee_amount: BigDecimal::from(0),
            expires_at: quote.ttl().expires_at,
            redis_key: quote.idempotency_key().to_owned(),
            ttl_seconds: command.ttl_seconds,
        };

        self.repository.save_quote_ttl(entry)?;

        Ok(ConvertQuoteCreated {
            quote_id: command.quote_id,
            expires_at: quote.ttl().expires_at,
            redis_key: quote.idempotency_key().to_owned(),
            ttl_seconds: command.ttl_seconds,
        })
    }
}

impl<R> ConvertService<R>
where
    R: ConvertQuoteRepository + ConvertOrderRepository,
{
    pub fn confirm_quote(
        &mut self,
        command: ConfirmConvertQuoteCommand,
    ) -> Result<ConvertConfirmationResult, ConvertServiceError> {
        let entry = self
            .repository
            .get_quote_ttl(&command.quote_id)?
            .ok_or_else(|| ConvertServiceError::QuoteNotFound {
                quote_id: command.quote_id.clone(),
            })?;

        if command.confirmed_at >= entry.expires_at {
            return Err(ConvertServiceError::QuoteExpired {
                quote_id: command.quote_id,
            });
        }

        let record = ConvertQuoteConfirmationRecord {
            quote_id: command.quote_id.clone(),
            user_id: command.user_id,
            from_asset: entry.from_asset,
            to_asset: entry.to_asset,
            from_amount: entry.from_amount,
            to_amount: entry.to_amount,
            confirmed_at: command.confirmed_at,
        };

        match self.repository.insert_quote_confirmation(record)? {
            ConvertConfirmationInsert::Inserted => Ok(ConvertConfirmationResult {
                quote_id: command.quote_id,
                confirmed: true,
            }),
            ConvertConfirmationInsert::Duplicate => {
                Err(ConvertServiceError::DuplicateQuoteConfirmation {
                    quote_id: command.quote_id,
                })
            }
        }
    }
}
