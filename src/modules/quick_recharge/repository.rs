//! quick_recharge bounded context repository layer.
//!
//! 仓储层：定义快速充值与 MySQL 之间的读写契约，只描述数据形状，不含 SQL、校验与外部调用。
//! 类型按用途分三组：`Filter` 是已归一化的查询条件，`Write`/`Update` 是各阶段的写入参数，
//! `Row` 是查询结果快照。
//! 订单在生命周期中要经历建单、支付方回执补齐、回调确认三次写入，因此写入参数被拆成三个独立结构，
//! 各自只携带对应阶段真正需要落库的字段，避免用一个大结构承载全部可空字段而丢失阶段语义。
//! 金额统一用 `BigDecimal`，商户密钥只以密文与掩码两种形态出现，本层任何类型都不持有密钥明文。

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde_json::Value;

/// 用户侧订单查询条件，用户编号为必填且直接来自令牌，因此查询天然限定在本人范围内。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeUserOrderFilter {
    /// 订单归属用户编号，由鉴权令牌解析而来。
    pub(crate) user_id: u64,
    /// 可选的订单状态筛选，已按本地状态机校验过取值合法性。
    pub(crate) status: Option<String>,
    /// 单次返回条数上限，已归一化；用户侧不支持偏移分页。
    pub(crate) limit: u32,
}

/// 后台订单查询条件，五个筛选维度全部可选，同时给出时按 AND 叠加。
/// 其中订单号与支付方交易号是掉单排查的主要入口，均为精确匹配。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeAdminOrderFilter {
    /// 按下单用户编号筛选。
    pub(crate) user_id: Option<u64>,
    /// 按账号邮箱精确匹配，空白已被裁剪为 `None`。
    pub(crate) email: Option<String>,
    /// 按订单状态筛选，取值已校验。
    pub(crate) status: Option<String>,
    /// 按本地对外订单号精确匹配。
    pub(crate) order_id: Option<String>,
    /// 按支付方交易号精确匹配，用于从渠道侧凭证反查本地订单。
    pub(crate) provider_trade_id: Option<String>,
    /// 单页条数，已夹取到 1 至 200。
    pub(crate) limit: u32,
    /// 分页偏移，已截断到 100000 以内。
    pub(crate) offset: u32,
}

/// 建单阶段的订单写入参数，落库后订单状态为 `created`，此时尚未联系支付方。
/// 币种、资产与回跳地址在此刻快照，后续渠道配置变更不影响已建订单。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeOrderCreateWrite {
    /// 服务端生成的对外订单号，同时作为发给支付方的商户订单号。
    pub(crate) order_id: String,
    /// 下单用户编号。
    pub(crate) user_id: u64,
    /// 下单时的用户邮箱冗余快照，用户后续改邮箱不影响历史订单。
    pub(crate) user_email: Option<String>,
    /// 到账资产编号，由配置中的收款币种解析而来。
    pub(crate) asset_id: u64,
    /// 到账资产符号快照。
    pub(crate) asset_symbol: String,
    /// 计价法币代码。
    pub(crate) currency: String,
    /// 收款币种代码。
    pub(crate) token: String,
    /// 收款链网络标识。
    pub(crate) network: String,
    /// 用户申请充值的法币金额，也是后续回调金额一致性比对的基准。
    pub(crate) fiat_amount: BigDecimal,
    /// 下单来源端标识，用于记录本单选用了哪套回跳地址。
    pub(crate) return_target: Option<String>,
    /// 本单实际使用的同步回跳地址，按来源端选定后固化。
    pub(crate) redirect_url: Option<String>,
}

/// 支付方建单成功后的回执写入参数，把订单从 `created` 推进到 `pending`。
/// `pending` 只表示收款信息已就绪可供用户付款，不代表已到账，此阶段不产生任何资金变动。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeOrderProviderUpdate {
    /// 目标订单的对外订单号。
    pub(crate) order_id: String,
    /// 支付方分配的交易号，回调时用于比对是否为同一笔交易。
    pub(crate) provider_trade_id: String,
    /// 支付方按当时汇率折算出的应付加密货币数量。
    pub(crate) actual_amount: BigDecimal,
    /// 用户应付款的收款地址。
    pub(crate) receive_address: String,
    /// 支付方托管的收银台地址。
    pub(crate) payment_url: String,
    /// 收款地址失效时间戳，为空表示支付方未给出有效期。
    pub(crate) expiration_time: Option<i64>,
    /// 支付方回执中的法币币种，会被转小写后覆盖写回订单。
    pub(crate) currency: String,
    /// 支付方回执中的收款币种，同样转小写后覆盖写回。
    pub(crate) token: String,
}

/// 回调确认后的订单写入参数，把订单推进到终态 `paid`，是资金入账事务的一部分。
/// 全部字段都来自已验签并逐项比对过的回调报文，未验签的数据不得构造本结构。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeOrderPaidUpdate {
    /// 目标订单的对外订单号。
    pub(crate) order_id: String,
    /// 回调携带的支付方交易号，仅在订单原本没有交易号时才会写入。
    pub(crate) provider_trade_id: String,
    /// 回调确认的实际到账数量，也是给用户钱包入账的金额。
    pub(crate) actual_amount: BigDecimal,
    /// 回调携带的收款地址，为空时保留订单上原有地址不覆盖。
    pub(crate) receive_address: Option<String>,
    /// 链上转账哈希，为空表示支付方未提供，可用于事后链上核对。
    pub(crate) block_transaction_id: Option<String>,
    /// 回调原始报文，整体存档以便日后复核验签与争议处理。
    pub(crate) callback_payload_json: Value,
}

/// 渠道单例配置的写入参数，按全字段覆盖语义使用。
/// 密钥密文与掩码必须成对给出：本次不换密钥时应回填旧值，留空会把已生效的密钥清掉。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeConfigWrite {
    /// 渠道是否启用；为真时上层已断言基础字段齐备。
    pub(crate) enabled: bool,
    pub(crate) api_base_url: Option<String>,
    pub(crate) merchant_pid: Option<String>,
    /// 商户密钥密文，`None` 表示尚未设置密钥，启用渠道时不允许为空。
    pub(crate) merchant_secret_ciphertext: Option<String>,
    /// 商户密钥掩码，仅供后台展示，与密文同步更新。
    pub(crate) merchant_secret_mask: Option<String>,
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
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    /// 本次变更的操作管理员编号，会同时写入配置行与审计日志。
    pub(crate) updated_by: u64,
}

/// 渠道单例配置的读取快照，是本层唯一携带密钥密文的类型。
/// 该类型不可直接序列化返回，必须先经表现层转换剥离密文只保留掩码。
/// 解密只在服务层构造运行时配置时发生，其余任何路径都应把密文视为不透明字符串。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeConfigRow {
    /// 配置行主键，用作审计记录的目标编号。
    pub(crate) id: u64,
    /// 配置名称，固定为单例名，用于按名寻址。
    pub(crate) name: String,
    /// 渠道商标识，当前固定为 GMPay。
    pub(crate) provider: String,
    pub(crate) enabled: bool,
    pub(crate) api_base_url: Option<String>,
    pub(crate) merchant_pid: Option<String>,
    /// 商户密钥密文，只允许传给解密函数，禁止进入响应体、审计与日志。
    pub(crate) merchant_secret_ciphertext: Option<String>,
    /// 商户密钥掩码，是密钥唯一可对外展示的形态。
    pub(crate) merchant_secret_mask: Option<String>,
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
    pub(crate) min_amount: BigDecimal,
    pub(crate) max_amount: Option<BigDecimal>,
    /// 最后一次修改配置的管理员编号，初始建行前为空。
    pub(crate) updated_by: Option<u64>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

/// 充值订单的读取快照，用户列表、后台列表、详情与加锁读取共用同一结构。
/// 多个字段随订单阶段逐步填充：建单时只有法币金额，支付方回执后补上交易号与收款信息，
/// 回调确认后才有链上哈希与支付时间，因此这些字段为空只代表订单尚未走到该阶段。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeOrderRow {
    /// 自增主键，仅内部使用，删除订单时按它定位。
    pub(crate) id: u64,
    /// 对外订单号，也是发给支付方的商户订单号，用户与后台看到的都是它。
    pub(crate) order_id: String,
    pub(crate) user_id: u64,
    /// 邮箱，优先取订单冗余值，缺失时回落到用户表当前邮箱。
    pub(crate) user_email: Option<String>,
    pub(crate) asset_id: u64,
    pub(crate) asset_symbol: String,
    pub(crate) currency: String,
    pub(crate) token: String,
    pub(crate) network: String,
    /// 用户申请的法币金额，回调金额必须与之精确相等才允许入账。
    pub(crate) fiat_amount: BigDecimal,
    /// 应付或已付的加密货币数量，建单阶段为空，回执后填入并可能被回调值覆盖。
    pub(crate) actual_amount: Option<BigDecimal>,
    /// 支付方交易号，建单阶段为空。
    pub(crate) provider_trade_id: Option<String>,
    /// 收款地址，建单阶段为空。
    pub(crate) receive_address: Option<String>,
    /// 支付方收银台地址，建单阶段为空。
    pub(crate) payment_url: Option<String>,
    /// 下单来源端标识。
    pub(crate) return_target: Option<String>,
    /// 本单选定的同步回跳地址。
    pub(crate) redirect_url: Option<String>,
    /// 收款地址失效时间戳。
    pub(crate) expiration_time: Option<i64>,
    /// 订单状态，取值为 `created`、`pending`、`paid`、`failed`、`expired`，其中 `paid` 为入账终态。
    pub(crate) status: String,
    /// 链上转账哈希，仅回调携带时才有值。
    pub(crate) block_transaction_id: Option<String>,
    /// 入账完成时刻，非 `paid` 订单为空。
    pub(crate) paid_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

/// 建单时用到的资产标识快照，来源于按收款币种符号查到的启用资产。
/// 两个字段都会被冗余写进订单行，使订单不依赖资产表的后续改名或停用。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeAssetRow {
    /// 资产编号，决定入账写到哪个钱包账户。
    pub(crate) id: u64,
    /// 资产符号，快照进订单用于展示。
    pub(crate) symbol: String,
}

/// 入账事务内加锁读到的钱包余额快照，充值只增加可用余额。
/// 另两项余额在此仅用于写流水时记录变更当时的完整分布，充值路径不会改动它们。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeWalletRow {
    /// 可用余额，到账金额直接累加到这一项。
    pub(crate) available: BigDecimal,
    /// 冻结余额，充值不改动。
    pub(crate) frozen: BigDecimal,
    /// 锁定余额，充值不改动。
    pub(crate) locked: BigDecimal,
}
