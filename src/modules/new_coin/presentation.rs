//! new_coin bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 本文件定义新币模块对外的请求体、查询参数与响应体，以及从仓储只读模型到响应体的转换。
//! 转换一律是逐字段平移：不补默认值、不做单位换算、不聚合统计，也不访问任何存储。
//! 时间字段统一经 `unix_millis` 与 `option_unix_millis` 序列化为毫秒时间戳，
//! 空值保持为 null 而不是回退到零时刻，使前端能区分「无该时点」与「时点为纪元起始」。
//! 金额字段以 `BigDecimal` 承载并按其字符串形式输出，不截断小数位也不转成浮点，
//! 因此响应中的精度与数据库中存放的精度一致。
//! 可空字段的空值语义在转换中被完整保留，具体含义见各转换函数的说明。

use crate::{
    modules::new_coin::repository::{
        NewCoinDistributionRead, NewCoinProjectRead, NewCoinPurchaseRead, NewCoinSubscriptionRead,
        NewCoinUnlockRead,
    },
    time::{option_unix_millis, unix_millis},
};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinProjectResponse {
    pub(crate) id: u64,
    pub(crate) asset_id: u64,
    pub(crate) symbol: String,
    pub(crate) lifecycle_status: String,
    pub(crate) total_supply: BigDecimal,
    pub(crate) issue_price: BigDecimal,
    pub(crate) quote_asset_id: Option<u64>,
    pub(crate) reserved_supply: BigDecimal,
    pub(crate) allocated_supply: BigDecimal,
    pub(crate) remaining_supply: BigDecimal,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) listed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) unlock_type: String,
    #[serde(default, with = "option_unix_millis")]
    pub(crate) fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) relative_unlock_seconds: Option<u64>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) post_listing_purchase_enabled: bool,
    pub(crate) post_listing_pair_id: Option<u64>,
    pub(crate) status: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinProjectsResponse {
    pub(crate) projects: Vec<NewCoinProjectResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSubscriptionRequest {
    pub(crate) quote_asset_id: u64,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreatePurchaseRequest {
    pub(crate) pair_id: u64,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) idempotency_key: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinOrderCreationResponse {
    pub(crate) idempotency_key: String,
    pub(crate) status: String,
    pub(crate) lock_position_id: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinSubscriptionResponse {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) quote_asset: u64,
    pub(crate) issue_price: BigDecimal,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) requested_quantity: BigDecimal,
    pub(crate) allocated_quantity: BigDecimal,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinSubscriptionsResponse {
    pub(crate) subscriptions: Vec<NewCoinSubscriptionResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinDistributionResponse {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) subscription_id: Option<u64>,
    pub(crate) asset_id: u64,
    pub(crate) quantity: BigDecimal,
    pub(crate) lock_position_id: Option<u64>,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinDistributionsResponse {
    pub(crate) distributions: Vec<NewCoinDistributionResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinPurchaseResponse {
    pub(crate) id: u64,
    pub(crate) project_id: u64,
    pub(crate) user_id: u64,
    pub(crate) pair_id: u64,
    pub(crate) base_asset: u64,
    pub(crate) quote_asset: u64,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) quote_amount: BigDecimal,
    pub(crate) lock_position_id: Option<u64>,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinPurchasesResponse {
    pub(crate) purchases: Vec<NewCoinPurchaseResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinUnlockResponse {
    pub(crate) id: u64,
    pub(crate) user_id: u64,
    pub(crate) asset_id: u64,
    pub(crate) lock_position_id: u64,
    pub(crate) unlock_quantity: BigDecimal,
    pub(crate) unlock_price: Option<BigDecimal>,
    pub(crate) unlock_fee_enabled: bool,
    pub(crate) unlock_fee_rate: Option<BigDecimal>,
    pub(crate) unlock_fee_basis: Option<String>,
    pub(crate) unlock_fee_asset: Option<u64>,
    pub(crate) unlock_fee_amount: Option<BigDecimal>,
    pub(crate) fee_paid_status: String,
    pub(crate) status: String,
    pub(crate) idempotency_key: String,
    #[serde(with = "unix_millis")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct NewCoinUnlocksResponse {
    pub(crate) unlocks: Vec<NewCoinUnlockResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PayUnlockFeeRequest {
    pub(crate) payment_asset_id: u64,
    pub(crate) amount: BigDecimal,
}

#[derive(Debug, Serialize)]
pub(crate) struct PayUnlockFeeResponse {
    pub(crate) unlock_idempotency_key: String,
    pub(crate) paid: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReleaseUnlockResponse {
    pub(crate) unlock_idempotency_key: String,
    pub(crate) released: bool,
}

impl From<NewCoinProjectRead> for NewCoinProjectResponse {
    /// 把项目只读模型平移成对外响应，列表与详情两个端点共用同一份字段集合。
    /// 生命周期、解禁类型与计费基准均以原始字符串输出而非枚举序号，便于前端直接展示与灰度扩展。
    /// 解禁与收费相关字段保持可空：未上市项目无上市时间，非固定时点解禁无固定解锁时间，
    /// 未开启收费的项目无费率、基准与支付资产，转换不为它们编造零值或默认值。
    fn from(read: NewCoinProjectRead) -> Self {
        Self {
            id: read.id,
            asset_id: read.asset_id,
            symbol: read.symbol,
            lifecycle_status: read.lifecycle_status,
            total_supply: read.total_supply,
            issue_price: read.issue_price,
            quote_asset_id: read.quote_asset_id,
            reserved_supply: read.reserved_supply,
            allocated_supply: read.allocated_supply,
            remaining_supply: read.remaining_supply,
            listed_at: read.listed_at,
            unlock_type: read.unlock_type,
            fixed_unlock_at: read.fixed_unlock_at,
            relative_unlock_seconds: read.relative_unlock_seconds,
            unlock_fee_enabled: read.unlock_fee_enabled,
            unlock_fee_rate: read.unlock_fee_rate,
            unlock_fee_basis: read.unlock_fee_basis,
            unlock_fee_asset: read.unlock_fee_asset,
            post_listing_purchase_enabled: read.post_listing_purchase_enabled,
            post_listing_pair_id: read.post_listing_pair_id,
            status: read.status,
        }
    }
}

impl From<NewCoinSubscriptionRead> for NewCoinSubscriptionResponse {
    /// 把申购单只读模型平移成对外响应，同时保留申请数量与实际配额数量两个字段。
    /// 两者并存使前端能自行展示中签情况，转换本身不相除也不推断中签率。
    /// 幂等键随响应返回，供客户端在重试或对账时定位同一笔申购。
    fn from(read: NewCoinSubscriptionRead) -> Self {
        Self {
            id: read.id,
            project_id: read.project_id,
            user_id: read.user_id,
            quote_asset: read.quote_asset,
            issue_price: read.issue_price,
            quote_amount: read.quote_amount,
            requested_quantity: read.requested_quantity,
            allocated_quantity: read.allocated_quantity,
            status: read.status,
            idempotency_key: read.idempotency_key,
            created_at: read.created_at,
        }
    }
}

impl From<NewCoinDistributionRead> for NewCoinDistributionResponse {
    /// 把分发记录只读模型平移成对外响应，两个可空外键的业务含义原样透出。
    /// 关联申购单为空表示该笔分发不来自申购流程，锁仓位置为空表示资产当时直接进入了可用余额，
    /// 前端可据此区分「锁仓待解禁」与「已可动用」两种到账形态。
    fn from(read: NewCoinDistributionRead) -> Self {
        Self {
            id: read.id,
            project_id: read.project_id,
            user_id: read.user_id,
            subscription_id: read.subscription_id,
            asset_id: read.asset_id,
            quantity: read.quantity,
            lock_position_id: read.lock_position_id,
            status: read.status,
            idempotency_key: read.idempotency_key,
            created_at: read.created_at,
        }
    }
}

impl From<NewCoinPurchaseRead> for NewCoinPurchaseResponse {
    /// 把二级市场买入记录平移成对外响应，价格、数量与计价总额三者同时输出。
    /// 三者是下单当时固化的快照，转换不重新相乘校验，因此历史订单不会因后续改配置而变化。
    /// 交易对及其基础与计价资产以数值编号输出，不在此解析为符号，
    /// 前端需自行结合资产字典渲染；锁仓位置为空表示这笔买入无需锁仓。
    fn from(read: NewCoinPurchaseRead) -> Self {
        Self {
            id: read.id,
            project_id: read.project_id,
            user_id: read.user_id,
            pair_id: read.pair_id,
            base_asset: read.base_asset,
            quote_asset: read.quote_asset,
            price: read.price,
            quantity: read.quantity,
            quote_amount: read.quote_amount,
            lock_position_id: read.lock_position_id,
            status: read.status,
            idempotency_key: read.idempotency_key,
            created_at: read.created_at,
        }
    }
}

impl From<NewCoinUnlockRead> for NewCoinUnlockResponse {
    /// 把解禁记录平移成对外响应，输出解禁数量、解禁价格与该批次固化的整套收费口径。
    /// 收费字段可空以表示项目未配置收费，转换保留空值而不折叠为零，
    /// 避免前端把「未开启收费」误显示成「费率为零」。
    /// 缴费状态与释放状态分列两个字段且相互独立，前端需同时判断二者才能决定
    /// 该记录当前应展示缴费按钮还是释放按钮；转换不做任何一致性推断或状态合并。
    fn from(read: NewCoinUnlockRead) -> Self {
        Self {
            id: read.id,
            user_id: read.user_id,
            asset_id: read.asset_id,
            lock_position_id: read.lock_position_id,
            unlock_quantity: read.unlock_quantity,
            unlock_price: read.unlock_price,
            unlock_fee_enabled: read.unlock_fee_enabled,
            unlock_fee_rate: read.unlock_fee_rate,
            unlock_fee_basis: read.unlock_fee_basis,
            unlock_fee_asset: read.unlock_fee_asset,
            unlock_fee_amount: read.unlock_fee_amount,
            fee_paid_status: read.fee_paid_status,
            status: read.status,
            idempotency_key: read.idempotency_key,
            created_at: read.created_at,
        }
    }
}
