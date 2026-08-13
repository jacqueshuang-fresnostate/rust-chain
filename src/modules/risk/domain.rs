//! risk bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 本文件定义风控的判定内核：把已取得的请求事实与后台折算出的阈值对照，得出放行或带具体原因的拒绝。
//! 判定按操作黑名单、限频、金额上限、价格偏离的固定顺序短路，先命中者决定拒绝原因；
//! 任一维度缺少事实或未配置阈值即跳过该项，因此风控的缺省行为是放行。
//! 这里不读数据库、不累加计数、不写事件，限频计数与命中留痕都由应用层在调用前后完成。

use crate::architecture::DomainLayer;
use bigdecimal::BigDecimal;
use thiserror::Error;

/// 风控拒绝原因，取值顺序与评估短路顺序无关，一次评估至多产出其中一项。
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RiskReject {
    #[error("rate limit exceeded")]
    RateLimit,
    #[error("amount exceeds limit")]
    AmountLimit,
    #[error("price deviation exceeded")]
    PriceDeviation,
    #[error("operation is not allowed")]
    OperationNotAllowed,
}

impl RiskReject {
    /// 返回稳定风控错误码，供 HTTP 和审计层保持拒绝原因一致。
    /// 这些字符串是对外契约的一部分，客户端据此区分是频率、限额、价格还是操作禁用被拦，
    /// 风控事件表也直接落这个码，因此取值一旦发布不得再改名，新增拒绝类型只能追加新码。
    pub fn code(&self) -> &'static str {
        match self {
            Self::RateLimit => "risk_rate_limit",
            Self::AmountLimit => "risk_amount_limit",
            Self::PriceDeviation => "risk_price_deviation",
            Self::OperationNotAllowed => "risk_operation_not_allowed",
        }
    }

    /// 返回与风控拒绝类型对应的中文用户提示，不泄露内部规则配置。
    /// 文案刻意只说明被拦的大类，不带具体阈值、窗口长度或命中的规则作用域，
    /// 避免攻击者通过反复试探反推出限额边界；需要精确原因时由后台按错误码查风控事件记录。
    pub fn message(&self) -> &'static str {
        match self {
            Self::RateLimit => "操作过于频繁，请稍后再试",
            Self::AmountLimit => "金额超出风控限额",
            Self::PriceDeviation => "价格偏离市场价过大",
            Self::OperationNotAllowed => "该操作已被风控规则限制",
        }
    }
}

/// 风控评估结论，拒绝时附带具体原因供上层转成错误码与用户提示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskDecision {
    Approved,
    Rejected(RiskReject),
}

impl RiskDecision {
    /// 判断评估结果是否放行；该只读检查不改变限频计数或审计状态。
    /// 供应用层在评估之后分流：放行则继续原业务，否则据拒绝类型落风控事件并返回错误。
    /// 多次调用不产生任何副作用，判定结论此时已经固化在枚举里。
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved)
    }
}

/// 后台配置出的风控阈值；`None` 表示该维度没有规则，必须放行。
/// 操作维度用黑名单而非白名单：每条规则只拒绝自己列出的操作，叠加只会拒绝更多具名操作，
/// 不会像白名单取交集那样把两条各自合理的规则合成"全面拒绝"。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RiskRules {
    pub max_requests: Option<u32>,
    pub max_amount: Option<BigDecimal>,
    pub max_price_deviation_bps: Option<u32>,
    pub blocked_operations: Option<Vec<String>>,
}

impl DomainLayer for RiskRules {}

impl RiskRules {
    /// 判断所有阈值和黑名单均为空，用于在应用层跳过计数及审计 I/O。
    /// 四个维度全部未配置时评估必然放行，提前识别这一情况可以省掉 Redis 限频自增和后续留痕，
    /// 让未接入风控规则的业务路径不因风控检查产生额外开销。
    pub fn is_unrestricted(&self) -> bool {
        self.max_requests.is_none()
            && self.max_amount.is_none()
            && self.max_price_deviation_bps.is_none()
            && self.blocked_operations.is_none()
    }
}

/// 待评估的业务请求；`None` 表示该路径无法诚实取到对应事实，相应校验必须跳过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskRequest {
    pub operation: String,
    pub request_count: Option<u32>,
    pub amount: Option<BigDecimal>,
    pub price: Option<BigDecimal>,
    pub reference_price: Option<BigDecimal>,
}

impl DomainLayer for RiskRequest {}

/// 按操作黑名单、限频、金额上限和价格偏离的稳定优先级评估请求。
/// 评估仅消费已取得的事实；缺少某维度时跳过该规则，不执行 I/O 或资金副作用。
/// 四项判定短路执行，先命中者即为最终拒绝原因，因此黑名单必须排在最前，否则会被限频等次要维度盖掉。
/// 阈值比较一律为严格大于，等于上限视为合规；价格偏离先折算成基点再比较，币种价格量级不影响判定口径。
pub fn evaluate_risk(request: &RiskRequest, rules: &RiskRules) -> RiskDecision {
    // 被禁用的操作优先拒绝，拒绝原因才不会被限频等次要维度盖掉。
    if let Some(blocked_operations) = rules.blocked_operations.as_ref()
        && blocked_operations
            .iter()
            .any(|operation| operation.eq_ignore_ascii_case(&request.operation))
    {
        return RiskDecision::Rejected(RiskReject::OperationNotAllowed);
    }

    if let (Some(max_requests), Some(request_count)) = (rules.max_requests, request.request_count)
        && request_count > max_requests
    {
        return RiskDecision::Rejected(RiskReject::RateLimit);
    }

    if let (Some(max_amount), Some(amount)) = (rules.max_amount.as_ref(), request.amount.as_ref())
        && amount > max_amount
    {
        return RiskDecision::Rejected(RiskReject::AmountLimit);
    }

    // 价格偏离按基准价折算为 bps，避免不同币种价格精度影响风控阈值。
    if let (Some(max_deviation_bps), Some(price), Some(reference_price)) = (
        rules.max_price_deviation_bps,
        request.price.as_ref(),
        request.reference_price.as_ref(),
    ) && price_deviation_bps(price, reference_price) > max_deviation_bps
    {
        return RiskDecision::Rejected(RiskReject::PriceDeviation);
    }

    RiskDecision::Approved
}

/// 把委托价相对基准价的偏离折算成基点，取绝对值因而不区分高于还是低于基准。
/// 基准价为零时无法计算比例，直接返回 i64 上界作为哨兵，使任何有限阈值都被判为超限而拒绝该请求，
/// 这是宁可错杀的保守选择：拿不到有效市场价时不应放行价格敏感的下单。
/// 除法保持十进制运算不转浮点，避免不同币种价格量级差异造成的精度漂移。
fn price_deviation_bps(price: &BigDecimal, reference_price: &BigDecimal) -> BigDecimal {
    if reference_price == &BigDecimal::from(0) {
        return BigDecimal::from(i64::MAX);
    }

    let deviation = if price >= reference_price {
        price - reference_price
    } else {
        reference_price - price
    };

    deviation * BigDecimal::from(10_000) / reference_price.clone()
}
