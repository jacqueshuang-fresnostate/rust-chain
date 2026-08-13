//! convert bounded context service layer.
//!
//! 服务层：封装可复用业务服务和跨实体业务规则。
//!
//! 本文件分为两部分。前半部分是一组无 I/O 的纯函数，承担闪兑的全部判定口径：
//! 请求方向归一化、金额与精度校验、汇率解析、目标额与手续费的计算与截断方向。
//! 后半部分的 `ConvertService` 是围绕仓储端口的通用编排壳，供内存适配器和单测使用，
//! 它不读实时钱包、不锁行、不结算资金；线上真实资金路径走应用层的 MySQL 事务。

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

/// 报价金额计算的输出对，两项都已按各自资产精度向零截断，可直接落库。
#[derive(Debug, Clone)]
pub(crate) struct ConvertQuoteAmounts {
    /// 目标资产到账数量，已扣手续费并叠加价差。
    pub(crate) to_amount: BigDecimal,
    /// 以源资产计价的手续费，已从净额中扣除，不再单独生成钱包流水。
    pub(crate) fee_amount: BigDecimal,
}

/// 只接受 `user:{u64}` 形式的鉴权 subject，其余任何写法都视为未授权而非参数错误。
/// 前缀缺失或数字段溢出 u64 都走同一失败分支，避免向调用方泄露主体格式细节。
/// 闪兑所有涉及用户资金的入口都必须先过这一步，用户维度只能来自 JWT，不能来自请求体。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 严格把客户端 quote_id 解析为 UUID；非法值在读取 Redis 或 MySQL 前返回参数错误。
/// 提前拦截可以避免用任意字符串拼出缓存键去探测 Redis，也省掉一次无谓的数据库往返。
/// 解析成功不代表报价存在或归属调用者，归属与有效期在确认流程中另行判定。
pub(crate) fn parse_quote_id(value: &str) -> AppResult<QuoteId> {
    Uuid::parse_str(value)
        .map(QuoteId)
        .map_err(|_| AppError::Validation("invalid quote_id".to_owned()))
}

/// 将闪兑列表数量规范为默认 50、最小 1、最大 100，交易对与订单两个列表接口共用该口径。
/// 越界值被夹紧而不是报错，因此客户端传 0 或超大值都能拿到结果，不会因分页参数失败。
/// 上限用于防止单次查询拉走整表，调用方不能绕过本函数直接把原始 limit 拼进 SQL。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 裁剪可选查询文本并把纯空白归一为 `None`，使 `status=` 与不传该参数得到相同的不过滤语义。
/// 不校验订单状态枚举，未知状态词会照常拼进 SQL 条件并自然查不到数据。
/// 裁剪后的值仍以绑定参数方式下推，本函数不承担任何注入防护职责。
pub(crate) fn optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 把数据库交易对行归一化为「按用户请求方向」的报价规则，屏蔽配置中资产的原始排列顺序。
/// 反向判定条件是配置的 from/to 与请求的 to/from 恰好互换；只要不满足就按正向处理。
/// 正向沿用 min_amount/max_amount，反向改用 target_min_amount/target_max_amount 作为限额。
/// 反向固定计价取 `1 / fixed_rate`，因此固定汇率必须为正，非正时在报价计算前直接拒绝。
/// 价差、费率与市场计价线索原样透传，本函数不读取钱包和行情，也不产生任何持久化副作用。
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

/// 在真正计价前逐项否决非法报价请求，四项检查按固定次序执行且任一不通过即返回参数错误。
/// 依次校验源金额严格为正、费率落在 `[0,1)`、金额不低于该方向最小额、不超过可为空的最大额，
/// 最后要求计价模式是 fixed 或 market 之一，未知模式在此拦下而不是留到汇率解析阶段。
/// 费率上界取开区间是因为费率达到 1 会把净额吃光，后续目标额必然非正。
/// 该纯校验不处理小数位、不读钱包、不冻结 available，失败时不生成报价行也不写缓存。
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

/// 校验资产 precision_scale 落在钱包统一支持的 0..=18 区间，上界与账本列的小数位一致。
/// 越界只可能来自资产表被写坏，属于配置故障而非用户输入问题，因此归为内部错误而不是参数错误。
/// 校验通过只说明精度值可用于截断计算，不代表该资产已开通钱包账户或已配置闪兑规则。
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

/// 取出 fixed 计价模式所需的固定汇率，该值来自状态为 active 的新币兑换规则联表结果。
/// 规则下线或未配置时字段为空，此处直接拒绝报价，不退化到市场行情也不接受客户端传入汇率。
/// 若请求方向与配置相反，取到的已是服务层换算过的倒数值，本函数不再做方向判断。
pub(crate) fn resolve_fixed_convert_rate(pair: &ConvertPairRule) -> AppResult<BigDecimal> {
    pair.fixed_rate.clone().ok_or_else(|| {
        AppError::Validation("convert quote requires active fixed pricing rule".to_owned())
    })
}

/// 一次性取齐市场计价所需的三项配置：现货交易对符号、base 资产编号和 quote 资产编号。
/// 三者要么同时存在要么同时缺失，缺失说明该闪兑对没有关联到状态为 active 的现货交易对。
/// 任一项为空即拒绝市场报价并返回相同的参数错误，不查询 Redis，也不猜测资产方向。
/// 返回的符号只是行情缓存键的来源，方向匹配由 `resolve_market_convert_rate` 负责判定。
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

/// 把现货行情价换算成闪兑请求方向上的汇率，只承认两种精确匹配，不做任何跨对路由。
/// 请求方向与市场 base 到 quote 完全一致时直接使用原价，恰好相反时使用 `1 / market_price`。
/// 两侧资产与行情交易对对不上就拒绝，避免把无关交易对的价格误用为兑换比例。
/// 价格为正由行情适配器在读取缓存时保证，本函数据此直接取倒数而不再复核除零风险。
/// 返回值尚未叠加价差与手续费，两者在目标额计算环节统一折算。
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

/// 把仓储层的存储与序列化故障统一收敛为内部错误，对客户端不区分是 Redis 还是 MySQL 出问题。
/// 刻意不映射成参数错误或未找到，避免把缓存不可用伪装成报价不存在而诱导客户端重复下单。
/// 原始错误以调试格式保留在消息里供日志排查；本函数不重试，也不撤销已经发生的存储副作用。
pub(crate) fn map_convert_repository_error(error: ConvertRepositoryError) -> AppError {
    AppError::Internal(format!("{error:?}"))
}

/// 围绕报价仓储端口的通用闪兑编排壳，泛型参数决定实际存储行为。
/// 它只覆盖有效期与确认幂等两条规则，不涉及实时钱包读写，线上资金路径不经过这里。
#[derive(Debug, Clone)]
pub struct ConvertService<R> {
    repository: R,
}

impl<R> ConvertService<R>
where
    R: ConvertQuoteRepository,
{
    /// 注入闪兑仓储端口供报价与确认规则复用，所有权由服务持有直到显式取回。
    /// 构造阶段不发起任何读写：不读报价、不锁钱包、不创建订单，也不校验仓储是否可用。
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

    /// 消费服务并把仓储所有权交还调用方，常用于测试收尾时直接断言内存适配器的最终状态。
    /// 移交过程不创建报价、不确认订单、不移动钱包余额，也不清理已经写入的缓存条目。
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
