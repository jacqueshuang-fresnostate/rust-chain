//! quick_recharge bounded context presentation layer.
//!
//! 表现层：定义快速充值对外的请求与响应 DTO，以及存储行到响应体的脱敏转换。
//! 最重要的边界在这里落地：`QuickRechargeConfigRow` 携带商户密钥密文，
//! 而它转成 `QuickRechargeConfigResponse` 时密文被彻底丢弃，只留下掩码和一个「是否已设置」的布尔量，
//! 因此任何走本层导出的响应都不可能泄露密钥。
//!
//! 金额一律通过自定义序列化输出为固定 18 位小数的字符串，而不是 JSON 数字。
//! 这样做是为了避免 JavaScript 客户端按双精度浮点解析导致金额失真，固定标度也让前端无需猜测小数位。
//! 时间字段统一序列化为毫秒时间戳，可空时间用专门的可选序列化模块处理。
//! 本文件不含业务规则、数据库访问与签名计算。

use super::repository::{QuickRechargeConfigRow, QuickRechargeOrderRow};
use crate::time::{option_unix_millis, unix_millis};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer};

/// 后台保存渠道配置的请求体，按全字段覆盖语义提交，前端必须回填完整配置而非增量补丁。
/// 密钥字段是唯一的例外：留空表示沿用当前密钥，只有填了非空值才会触发密钥更换。
#[derive(Debug, Deserialize)]
pub struct SaveQuickRechargeConfigRequest {
    /// 是否启用渠道；为真时服务端会额外要求 API 地址、商户号与回调地址齐备。
    pub(crate) enabled: bool,
    /// 支付方 API 根地址，只接受 http 或 https。
    pub(crate) api_base_url: Option<String>,
    /// 商户号，只允许字母数字与下划线。
    pub(crate) merchant_pid: Option<String>,
    /// 新的商户密钥明文；留空或纯空白表示不更换，服务端会保留既有密文与掩码。
    /// 该值只用于加密入库，不会被回显，也不会写入审计或日志。
    pub(crate) merchant_secret: Option<String>,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    pub(crate) notify_url: Option<String>,
    pub(crate) redirect_url: Option<String>,
    pub(crate) pc_app_redirect_url: Option<String>,
    pub(crate) mac_app_redirect_url: Option<String>,
    pub(crate) ios_app_redirect_url: Option<String>,
    pub(crate) android_app_redirect_url: Option<String>,
    pub(crate) mobile_web_redirect_url: Option<String>,
    pub(crate) desktop_web_redirect_url: Option<String>,
    /// 单笔最小充值金额，必须为正数。
    pub(crate) min_amount: BigDecimal,
    /// 单笔最大充值金额，给出时不得小于最小值，省略表示不限。
    pub(crate) max_amount: Option<BigDecimal>,
    /// 变更原因，字段可空但业务上必填，缺失会在校验阶段被拒绝。
    pub(crate) reason: Option<String>,
}

/// 后台连通性测试请求体，用一笔真实建单验证配置是否可用。
/// 金额同样受配置的上下限约束，建议填最小金额以降低误操作代价。
#[derive(Debug, Deserialize)]
pub struct TestQuickRechargeConfigRequest {
    /// 测试用法币金额，必须落在当前配置的金额区间内。
    pub(crate) amount: BigDecimal,
    /// 测试原因，业务上必填，与配置快照一并写入审计。
    pub(crate) reason: Option<String>,
}

/// 后台删除充值订单的请求体，仅承载审计原因。
/// DELETE 之所以带请求体，正是因为原因属于必填审计信息，无法通过路径或查询串表达。
#[derive(Debug, Deserialize)]
pub struct DeleteQuickRechargeOrderRequest {
    /// 删除原因，业务上必填。
    pub(crate) reason: Option<String>,
}

/// 用户发起充值的请求体，只需金额与来源端两项。
/// 请求体刻意不含用户编号、订单号、收款地址与币种：下单人由令牌决定，其余全部取自服务端配置，
/// 因此客户端无法指定入账对象或收款地址。
#[derive(Debug, Deserialize)]
pub struct CreateQuickRechargeOrderRequest {
    /// 充值法币金额，必须落在渠道配置的最小与最大金额之间。
    pub(crate) amount: BigDecimal,
    /// 发起端类型，用于选择支付完成后的回跳地址；省略时使用通用回跳地址。
    pub(crate) return_target: Option<QuickRechargeReturnTarget>,
}

/// 发起充值的客户端类型，决定支付完成后采用哪套同步回跳地址。
/// 四个客户端分支允许配置自定义 URL Scheme 以唤起本地应用，两个网页分支只允许 http 或 https。
/// 序列化采用蛇形命名，与数据库中存储的来源端文本保持一致。
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuickRechargeReturnTarget {
    /// Windows 等 PC 桌面客户端。
    PcApp,
    /// macOS 桌面客户端。
    MacApp,
    /// iOS 移动客户端。
    IosApp,
    /// Android 移动客户端。
    AndroidApp,
    /// 移动端浏览器访问的网页。
    MobileWeb,
    /// 桌面端浏览器访问的网页。
    DesktopWeb,
}

impl QuickRechargeReturnTarget {
    /// 返回该来源端的稳定文本标识，用于写入订单行的来源端字段。
    /// 取值与序列化使用的蛇形命名完全一致，保证库中存量数据能被反序列化回同一枚举分支。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PcApp => "pc_app",
            Self::MacApp => "mac_app",
            Self::IosApp => "ios_app",
            Self::AndroidApp => "android_app",
            Self::MobileWeb => "mobile_web",
            Self::DesktopWeb => "desktop_web",
        }
    }
}

/// 订单列表查询串，用户侧与后台侧共用同一结构。
/// 用户侧只消费 `status` 与 `limit`，其余字段即使传入也会被忽略，
/// 因为用户侧的用户维度只从令牌取，不存在通过 `user_id` 查他人订单的路径。
#[derive(Debug, Deserialize)]
pub struct QuickRechargeOrdersQuery {
    /// 按用户编号筛选，仅后台侧生效。
    pub(crate) user_id: Option<u64>,
    /// 按账号邮箱精确筛选，仅后台侧生效。
    pub(crate) email: Option<String>,
    /// 按订单状态筛选，非法状态会被拒绝而不是当成空结果。
    pub(crate) status: Option<String>,
    /// 按本地订单号精确筛选，仅后台侧生效。
    pub(crate) order_id: Option<String>,
    /// 按支付方交易号精确筛选，仅后台侧生效。
    pub(crate) provider_trade_id: Option<String>,
    /// 单页条数，缺省 50，归一后夹在 1 到 200。
    pub(crate) limit: Option<u32>,
    /// 分页偏移，仅后台侧生效，归一后截断到 100000。
    pub(crate) offset: Option<u32>,
}

/// 后台渠道配置响应，是配置存储行经脱敏后的对外视图。
/// 与存储行的关键差别是没有密钥密文字段，取而代之的是掩码和一个布尔量，
/// 前端据此既能显示「已配置密钥」，又拿不到任何可用于伪造签名的材料。
#[derive(Debug, Serialize, Clone)]
pub struct QuickRechargeConfigResponse {
    pub(crate) id: u64,
    pub(crate) name: String,
    /// 渠道商标识，当前固定为 GMPay。
    pub(crate) provider: String,
    pub(crate) enabled: bool,
    pub(crate) api_base_url: Option<String>,
    pub(crate) merchant_pid: Option<String>,
    /// 商户密钥掩码，供后台确认当前配的是哪把密钥，不可用于签名。
    pub(crate) merchant_secret_mask: Option<String>,
    /// 是否已设置商户密钥，由密文是否存在推导，前端据此提示是否需要首次录入。
    pub(crate) merchant_secret_set: bool,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    pub(crate) notify_url: Option<String>,
    pub(crate) redirect_url: Option<String>,
    pub(crate) pc_app_redirect_url: Option<String>,
    pub(crate) mac_app_redirect_url: Option<String>,
    pub(crate) ios_app_redirect_url: Option<String>,
    pub(crate) android_app_redirect_url: Option<String>,
    pub(crate) mobile_web_redirect_url: Option<String>,
    pub(crate) desktop_web_redirect_url: Option<String>,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) min_amount: BigDecimal,
    #[serde(serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) max_amount: Option<BigDecimal>,
    pub(crate) updated_by: Option<u64>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

/// 用户侧渠道信息响应，是后台配置的最小可见子集。
/// 只回传下单所需的开关、币种、网络与金额区间，接入细节如 API 地址、商户号、回调地址一律不外泄。
#[derive(Debug, Serialize, Clone)]
pub struct UserQuickRechargeConfigResponse {
    /// 渠道是否开放充值；为假时前端应隐藏或禁用入口。
    pub(crate) enabled: bool,
    /// 计价法币代码。
    pub(crate) currency: String,
    /// 收款币种代码，此处已转为大写以匹配资产符号展示习惯。
    pub(crate) token: String,
    /// 收款链网络标识。
    pub(crate) network: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) min_amount: BigDecimal,
    #[serde(serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) max_amount: Option<BigDecimal>,
}

/// 充值订单的对外视图，用户侧与后台侧共用同一结构。
/// 多个字段随订单阶段逐步填充，为空只表示订单尚未走到该阶段而非数据缺失：
/// 建单后仅有法币金额，支付方回执后才有交易号、收款地址与收银台链接，回调确认后才有链上哈希与支付时间。
/// 不包含回调原始报文，那份数据只落库供事后复核，不对外返回。
#[derive(Debug, Serialize, Clone)]
pub struct QuickRechargeOrderResponse {
    pub(crate) id: u64,
    /// 对外订单号，用户、后台与支付方三方一致的订单标识。
    pub(crate) order_id: String,
    pub(crate) user_id: u64,
    pub(crate) user_email: Option<String>,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) fiat_amount: BigDecimal,
    #[serde(serialize_with = "serialize_optional_decimal_amount")]
    pub(crate) actual_amount: Option<BigDecimal>,
    pub(crate) provider_trade_id: Option<String>,
    pub(crate) receive_address: Option<String>,
    pub(crate) payment_url: Option<String>,
    pub(crate) return_target: Option<String>,
    pub(crate) redirect_url: Option<String>,
    pub(crate) expiration_time: Option<i64>,
    pub(crate) status: String,
    pub(crate) block_transaction_id: Option<String>,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) paid_at: Option<DateTime<Utc>>,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: DateTime<Utc>,
    #[serde(with = "unix_millis")]
    pub(crate) updated_at: DateTime<Utc>,
}

/// 后台连通性测试的响应，回传本次真实建单换回的收款信息。
/// 该订单只存在于支付方侧，本地不落充值订单，因此这些字段无法通过订单列表再次查到，
/// 需要留存时应参考同时写入的管理员审计记录。
#[derive(Debug, Serialize, Clone)]
pub struct TestQuickRechargeConfigResponse {
    /// 本次测试使用的一次性订单号。
    pub(crate) order_id: String,
    /// 支付方为该测试单分配的交易号。
    pub(crate) provider_trade_id: String,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) fiat_amount: BigDecimal,
    #[serde(serialize_with = "serialize_decimal_amount")]
    pub(crate) actual_amount: BigDecimal,
    pub(crate) receive_address: String,
    pub(crate) payment_url: String,
    pub(crate) expiration_time: Option<i64>,
    /// 本次测试的执行时刻，毫秒时间戳，由服务端在组装响应时取当前时间。
    pub(crate) tested_at: i64,
}

/// 用户侧订单列表响应，只含订单数组，不返回总数，因为该接口不支持偏移分页。
#[derive(Debug, Serialize)]
pub struct QuickRechargeOrdersResponse {
    /// 按创建时间倒序排列的本人充值订单。
    pub(crate) orders: Vec<QuickRechargeOrderResponse>,
}

/// 后台订单列表响应，附带与当前筛选条件一致的总数以支撑分页控件。
#[derive(Debug, Serialize)]
pub struct AdminQuickRechargeOrdersResponse {
    /// 当前页的订单列表。
    pub(crate) orders: Vec<QuickRechargeOrderResponse>,
    /// 符合筛选条件的订单总数，不受分页参数影响。
    pub(crate) total: i64,
}

impl From<QuickRechargeConfigRow> for QuickRechargeConfigResponse {
    /// 把配置存储行转成对外响应，是商户密钥脱敏的执行点。
    /// 密钥密文不被拷贝到响应结构，只把「密文是否存在」折算成布尔量，掩码原样带出。
    /// 其余字段直接搬运，金额与时间的对外格式由字段上的序列化属性统一处理。
    fn from(row: QuickRechargeConfigRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            provider: row.provider,
            enabled: row.enabled,
            api_base_url: row.api_base_url,
            merchant_pid: row.merchant_pid,
            merchant_secret_mask: row.merchant_secret_mask,
            merchant_secret_set: row.merchant_secret_ciphertext.is_some(),
            currency: row.currency,
            token: row.token,
            network: row.network,
            notify_url: row.notify_url,
            redirect_url: row.redirect_url,
            pc_app_redirect_url: row.pc_app_redirect_url,
            mac_app_redirect_url: row.mac_app_redirect_url,
            ios_app_redirect_url: row.ios_app_redirect_url,
            android_app_redirect_url: row.android_app_redirect_url,
            mobile_web_redirect_url: row.mobile_web_redirect_url,
            desktop_web_redirect_url: row.desktop_web_redirect_url,
            min_amount: row.min_amount,
            max_amount: row.max_amount,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<QuickRechargeOrderRow> for QuickRechargeOrderResponse {
    /// 把订单存储行转成对外视图，逐字段直接搬运，不做过滤也不隐藏字段。
    /// 订单本身不含敏感凭据，因此无需像配置那样脱敏；跨用户隔离由查询阶段的条件保证。
    /// 回调原始报文不在存储行中，自然也不会出现在响应里。
    fn from(row: QuickRechargeOrderRow) -> Self {
        Self {
            id: row.id,
            order_id: row.order_id,
            user_id: row.user_id,
            user_email: row.user_email,
            asset_id: row.asset_id,
            asset_symbol: row.asset_symbol,
            currency: row.currency,
            token: row.token,
            network: row.network,
            fiat_amount: row.fiat_amount,
            actual_amount: row.actual_amount,
            provider_trade_id: row.provider_trade_id,
            receive_address: row.receive_address,
            payment_url: row.payment_url,
            return_target: row.return_target,
            redirect_url: row.redirect_url,
            expiration_time: row.expiration_time,
            status: row.status,
            block_transaction_id: row.block_transaction_id,
            paid_at: row.paid_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// 把金额序列化为固定 18 位小数的字符串而非 JSON 数字。
/// 用字符串是为了避免 JavaScript 按双精度浮点解析导致大额或高精度金额失真；
/// 固定标度则让前端无需按币种猜小数位，任何金额的文本长度与格式都一致。
/// 注意此处不去尾随零，与签名口径使用的紧凑表示不同，两者用途不同不可互换。
fn serialize_decimal_amount<S>(amount: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{amount:.18}"))
}

/// 可空金额的序列化，有值时与必填版本采用完全相同的 18 位小数字符串格式。
/// 无值时输出 JSON null 而不是零或空串，使「未设置上限」与「上限为零」在客户端可区分。
fn serialize_optional_decimal_amount<S>(
    amount: &Option<BigDecimal>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match amount {
        Some(amount) => serializer.serialize_some(&format!("{amount:.18}")),
        None => serializer.serialize_none(),
    }
}
