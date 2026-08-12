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
    error::{AppError, AppResult},
    modules::wallet::{
        MAX_ASSET_PRECISION_SCALE, amount_fits_asset_precision, truncate_amount_to_asset_precision,
    },
};
use bigdecimal::BigDecimal;
use uuid::Uuid;

pub(crate) const QUOTE_TTL_SECONDS: i64 = 30;

#[derive(Debug, Clone)]
pub(crate) struct ConvertQuoteAmounts {
    pub(crate) to_amount: BigDecimal,
    pub(crate) fee_amount: BigDecimal,
}

/// 只接受 `user:{u64}` 形式的鉴权 subject；格式错误返回未授权。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 严格把客户端 quote_id 解析为 UUID；非法值在读取 Redis 或 MySQL 前返回参数错误。
pub(crate) fn parse_quote_id(value: &str) -> AppResult<QuoteId> {
    Uuid::parse_str(value)
        .map(QuoteId)
        .map_err(|_| AppError::Validation("invalid quote_id".to_owned()))
}

/// 将闪兑列表数量规范为默认 50、最小 1、最大 100。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 裁剪可选查询文本并把空白值归一为 `None`；不校验订单状态枚举。
pub(crate) fn optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 把数据库交易对转换为请求方向的报价规则：正向沿用源侧限额，反向改用目标侧限额。
/// 反向固定计价使用 `1 / fixed_rate`；固定汇率非正时在报价计算前拒绝，不读取钱包或行情。
pub(crate) fn convert_pair_rule_from_record(
    row: ConvertPairRuleDbRecord,
    from_asset_id: u64,
    to_asset_id: u64,
) -> AppResult<ConvertPairRule> {
    let is_reverse = row.from_asset_id == to_asset_id && row.to_asset_id == from_asset_id;
    let fixed_rate = match (row.fixed_rate, is_reverse) {
        (Some(rate), true) => {
            if rate <= 0 {
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

/// 校验源金额为正、fee_rate 位于 `[0,1)`、金额落在请求方向限额内且计价模式为 fixed/market。
/// 该纯校验不处理小数位、不冻结 available，失败时不生成报价行或缓存。
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

/// 校验资产 precision_scale 位于钱包支持的 0..=18；损坏配置按内部错误处理。
pub(crate) fn ensure_asset_precision_scale(precision_scale: i32) -> AppResult<()> {
    if !(0..=MAX_ASSET_PRECISION_SCALE).contains(&precision_scale) {
        return Err(AppError::Internal(format!(
            "asset precision_scale is outside supported range: {precision_scale}"
        )));
    }
    Ok(())
}

/// 要求提交金额有效小数位不超过源资产 precision_scale，尾随零不计为额外精度。
/// 本函数拒绝超精度输入而不隐式截断，确保报价、订单和源资产流水金额一致。
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

/// 报价阶段要求源资产 available 覆盖完整 from_amount；locked 只用于错误上下文，不能参与消费。
/// 该快照校验不加钱包锁、不预留资金，确认事务仍会按最新 available 再次判定。
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

/// 按报价汇率、价差和费用计算目标到账额：`fee=from*fee_rate`，`to=(from-fee)*rate*(1-spread)`。
/// fee 按源资产精度向零截断，to_amount 按目标资产精度向零截断；净源额或目标额非正时拒绝报价。
/// 计算只返回快照，不扣 available；确认时源钱包仍扣完整 from_amount，费用不另生成钱包流水。
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
    if net_from_amount <= 0 {
        return Err(AppError::Validation(
            "convert amount must be greater than fee amount".to_owned(),
        ));
    }
    let raw_to_amount = net_from_amount * effective_rate;
    let to_amount = truncate_amount_to_asset_precision(&raw_to_amount, to_precision_scale);
    if to_amount <= 0 {
        return Err(AppError::Validation(
            "convert quote amount must be positive".to_owned(),
        ));
    }

    Ok(ConvertQuoteAmounts {
        to_amount,
        fee_amount,
    })
}

/// 返回交易对的固定汇率；缺失时拒绝报价，不以行情或客户端值兜底。
pub(crate) fn resolve_fixed_convert_rate(pair: &ConvertPairRule) -> AppResult<BigDecimal> {
    pair.fixed_rate.clone().ok_or_else(|| {
        AppError::Validation("convert quote requires active fixed pricing rule".to_owned())
    })
}

/// 读取市场计价所需的交易对符号、base 资产和 quote 资产标识。
/// 任一配置缺失即拒绝市场报价，不查询 Redis，也不猜测资产方向。
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

/// 请求方向与市场 base→quote 一致时使用原价，反向时使用 `1 / market_price`。
/// 两侧资产不匹配即拒绝；正价格校验由行情适配器负责，本函数不读取或更新行情缓存。
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

/// 将 Redis/MySQL 仓储错误统一映射为内部错误；不吞掉失败，也不改变已发生的存储副作用。
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
    /// 注入闪兑仓储端口供报价与结算规则协调使用；构造时不读报价、不锁钱包，也不创建订单。
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// 只读借用底层报价仓储，常用于检查内存适配器状态；本方法本身不读缓存。
    pub fn repository(&self) -> &R {
        &self.repository
    }

    /// 可变借用底层仓储；借用本身无副作用，后续操作语义由具体适配器决定。
    pub fn repository_mut(&mut self) -> &mut R {
        &mut self.repository
    }

    /// 消费服务并返还底层仓储所有权，不创建、确认报价或移动钱包余额。
    pub fn into_repository(self) -> R {
        self.repository
    }

    /// 依据调用方提供的余额快照校验 available，再构造报价 TTL 并写入报价仓储。
    /// 该通用服务不读取实时钱包、不冻结资金，且把 fee_rate/fee_amount 固定写为零；真实路由报价使用应用层费用计算。
    /// 仓储写入失败直接返回；是否具备事务或覆盖语义完全取决于注入的 `ConvertQuoteRepository`。
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
    /// 从报价仓储读取缓存快照，校验确认时刻早于 expires_at，再写入一次确认记录。
    /// 缓存缺失、过期或仓储失败均不产生新确认；仓储返回 Duplicate 时显式报告重复确认。
    /// 该通用服务不校验报价归属、不锁钱包也不结算资金；真实路由确认由应用层 MySQL 事务完成。
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
