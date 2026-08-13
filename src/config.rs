//! 服务端全局配置装配层：把进程环境变量解析成一份不可变的 `Settings` 快照，供传输层、基础设施连接与后台 worker 共享。
//! 配置只在启动时读取一次，运行期不监听环境变化，修改任何环境变量都必须重启进程才会生效。
//! 数据库、Mongo、Redis、RabbitMQ、JWT 密钥与三家行情源地址属于强制项，缺失时启动即失败，不存在带半套配置继续运行的分支。
//! 其余带 `#[serde(default = ...)]` 的项由本文件的 `default_*` 函数兜底，运维只需覆盖需要偏离默认值的开关。
//! 连接串与密钥统一用 `SecretString` 承载，避免出现在 `Debug` 输出与日志里，只有 `exposed_*` 方法能取到明文。

use config::{Config, ConfigError, Environment};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// 进程启动时解析出的全局配置快照，字段与同名大写环境变量一一对应，克隆代价仅为字符串与标量复制。
/// 后台任务的开关、轮询间隔与批量上限都集中在这里，便于对照单个子系统确认它是否会被拉起以及以什么节奏运行。
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    #[serde(default = "default_app_env")]
    pub app_env: String,
    #[serde(default = "default_host")]
    pub app_host: IpAddr,
    #[serde(default = "default_port")]
    pub app_port: u16,
    pub database_url: SecretString,
    pub mongodb_uri: SecretString,
    pub mongodb_database: String,
    pub redis_url: SecretString,
    pub rabbitmq_url: SecretString,
    pub jwt_secret: SecretString,
    #[serde(default)]
    pub credential_encryption_key: Option<SecretString>,
    #[serde(default = "default_access_ttl")]
    pub jwt_access_ttl_seconds: u64,
    #[serde(default = "default_refresh_ttl")]
    pub jwt_refresh_ttl_seconds: u64,
    pub bitget_rest_base_url: String,
    pub bitget_ws_url: String,
    pub htx_rest_base_url: String,
    pub htx_ws_url: String,
    #[serde(default = "default_coinbase_rest_base_url")]
    pub coinbase_rest_base_url: String,
    #[serde(default = "default_coinbase_ws_url")]
    pub coinbase_ws_url: String,
    #[serde(default, deserialize_with = "deserialize_env_vec")]
    pub market_feed_symbols: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_env_vec")]
    pub market_feed_intervals: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_env_vec")]
    pub market_feed_providers: Vec<String>,
    #[serde(default = "default_market_feed_reconnect_seconds")]
    pub market_feed_reconnect_seconds: u64,
    #[serde(default = "default_market_feed_rest_fallback_timeout_seconds")]
    pub market_feed_rest_fallback_timeout_seconds: u64,
    #[serde(default = "default_event_inbox_retry_scan_seconds")]
    pub event_inbox_retry_scan_seconds: u64,
    #[serde(default = "default_event_outbox_publisher_enabled")]
    pub event_outbox_publisher_enabled: bool,
    #[serde(default = "default_event_outbox_publisher_interval_seconds")]
    pub event_outbox_publisher_interval_seconds: u64,
    #[serde(default = "default_unlock_scanner_enabled")]
    pub unlock_scanner_enabled: bool,
    #[serde(default = "default_unlock_scanner_interval_seconds")]
    pub unlock_scanner_interval_seconds: u64,
    #[serde(default = "default_unlock_scanner_batch_limit")]
    pub unlock_scanner_batch_limit: u32,
    #[serde(default = "default_kline_recovery_enabled")]
    pub kline_recovery_enabled: bool,
    #[serde(default = "default_kline_recovery_interval_seconds")]
    pub kline_recovery_interval_seconds: u64,
    #[serde(default = "default_kline_recovery_batch_limit")]
    pub kline_recovery_batch_limit: u32,
    #[serde(default = "default_seconds_contract_settlement_enabled")]
    pub seconds_contract_settlement_enabled: bool,
    #[serde(default = "default_seconds_contract_settlement_interval_seconds")]
    pub seconds_contract_settlement_interval_seconds: u64,
    #[serde(default = "default_seconds_contract_settlement_batch_limit")]
    pub seconds_contract_settlement_batch_limit: u32,
    #[serde(default = "default_earn_auto_redemption_enabled")]
    pub earn_auto_redemption_enabled: bool,
    #[serde(default = "default_earn_auto_redemption_interval_seconds")]
    pub earn_auto_redemption_interval_seconds: u64,
    #[serde(default = "default_earn_auto_redemption_batch_limit")]
    pub earn_auto_redemption_batch_limit: u32,
    #[serde(default = "default_margin_liquidation_enabled")]
    pub margin_liquidation_enabled: bool,
    #[serde(default = "default_margin_liquidation_interval_seconds")]
    pub margin_liquidation_interval_seconds: u64,
    #[serde(default = "default_margin_liquidation_batch_limit")]
    pub margin_liquidation_batch_limit: u32,
    #[serde(default = "default_margin_interest_enabled")]
    pub margin_interest_enabled: bool,
    #[serde(default = "default_margin_interest_interval_seconds")]
    pub margin_interest_interval_seconds: u64,
    #[serde(default = "default_margin_interest_batch_limit")]
    pub margin_interest_batch_limit: u32,
    #[serde(default = "default_agent_commission_auto_settle_enabled")]
    pub agent_commission_auto_settle_enabled: bool,
    #[serde(default = "default_agent_commission_auto_settle_interval_seconds")]
    pub agent_commission_auto_settle_interval_seconds: u64,
    #[serde(default = "default_agent_commission_auto_settle_min_age_seconds")]
    pub agent_commission_auto_settle_min_age_seconds: u64,
    #[serde(default = "default_agent_commission_auto_settle_batch_limit")]
    pub agent_commission_auto_settle_batch_limit: u32,
}

impl Settings {
    /// 装配全局配置：先尽力加载工作目录下的 `.env`，文件缺失或解析失败都被忽略，随后整份从进程环境变量反序列化。
    /// 环境变量名不带前缀，就是字段名的大写形式，例如 `DATABASE_URL`、`MONGODB_DATABASE`、`MARKET_FEED_SYMBOLS`。
    /// 三项行情列表配置启用逗号分隔解析，其余字段按目标类型转换，端口、秒数、批量上限出现非数字文本会让整份配置失败。
    /// 缺少任一强制项时返回 `ConfigError` 而不是回落到占位值，调用方应据此终止启动，避免服务连着错误的库或队列跑起来。
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        Config::builder()
            .add_source(
                Environment::default()
                    .list_separator(",")
                    .with_list_parse_key("market_feed_symbols")
                    .with_list_parse_key("market_feed_intervals")
                    .with_list_parse_key("market_feed_providers")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()
    }

    /// 组合监听地址与端口，得到 HTTP 服务实际绑定的套接字地址，供启动流程建立 TCP 监听器。
    /// 默认落在 0.0.0.0:8080，即对容器内全部网卡开放；真正的访问边界由部署层网络策略决定，本方法不做任何限制。
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.app_host, self.app_port)
    }

    /// 取出 MySQL 连接串明文，仅供构建 SQLx 连接池与迁移工具建连使用，串内含账号口令，不得写入日志或接口响应。
    /// 该项为强制配置，能调用到这里说明启动阶段已解析成功，因此不存在缺省值或空串分支需要处理。
    pub fn exposed_database_url(&self) -> &str {
        self.database_url.expose_secret()
    }

    /// 取出 MongoDB 连接串明文，供 K 线等行情历史数据建连；具体使用哪个库由 `mongodb_database` 单独指定，不从串里推断。
    /// 与其他凭据一致，只在基础设施初始化时短暂暴露，禁止透传到管理端接口、错误信息或链路追踪字段中。
    pub fn exposed_mongodb_uri(&self) -> &str {
        self.mongodb_uri.expose_secret()
    }

    /// 取出 Redis 连接串明文，该实例同时承载登录会话存储、行情缓存与多个 worker 的协调键，属于共享依赖。
    /// 切换地址等于同时更换会话与缓存后端，会造成已登录用户令牌全部失效，线上变更前需要评估强制下线影响。
    pub fn exposed_redis_url(&self) -> &str {
        self.redis_url.expose_secret()
    }

    /// 取出 RabbitMQ 连接串明文，供事件 outbox 发布与 inbox 消费建立同一条共享 AMQP 连接。
    /// 该 URL 通常带虚拟主机路径且需要转义，配置错误会在启动建连阶段直接失败，进而导致事件相关协程无法拉起。
    pub fn exposed_rabbitmq_url(&self) -> &str {
        self.rabbitmq_url.expose_secret()
    }

    /// 取出凭据加密主密钥明文，未配置时返回 `None`，由调用方决定是拒绝加解密还是关闭依赖该密钥的管理端功能。
    /// 密钥长度必须为 32 字节，但校验发生在实际加解密处而非此处；更换密钥会让既有密文无法解开，属于不可逆操作。
    pub fn exposed_credential_encryption_key(&self) -> Option<&str> {
        self.credential_encryption_key
            .as_ref()
            .map(SecretString::expose_secret)
            .map(String::as_str)
    }
}

/// 把行情类列表配置规整成干净的字符串数组：先接受 serde 交来的字符串序列，再对每个元素按逗号做二次切分。
/// 二次切分是为了同时兼容两种输入形态，既支持配置库已按分隔符拆好的多元素，也支持整段未拆分的单元素字符串。
/// 每个片段都会去掉首尾空白并丢弃空片段，因此把环境变量设成空串等价于不订阅，而不是产生一个空交易对占位。
/// 本函数只做文本清洗，不校验交易对、周期或数据源是否合法，非法取值要到构建行情订阅运行配置时才会被拒绝。
fn deserialize_env_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<String>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect())
}

/// 提供 `APP_ENV` 缺省值 local，用于标记本地、测试或生产等部署形态，便于排障时辨认实例来源。
/// 该字段目前只随配置一起加载，代码中没有任何分支依据它切换行为，因此写错不会改变功能但会误导运维判断。
fn default_app_env() -> String {
    "local".to_owned()
}

/// 提供 `APP_HOST` 缺省值 0.0.0.0，表示监听容器内全部网卡，便于反向代理或编排平台直接把流量转发进来。
/// 需要只对本机开放时必须显式配置回环地址，这个默认值本身不提供任何访问控制或来源过滤能力。
fn default_host() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
}

/// 提供 `APP_PORT` 缺省值 8080，与监听地址组合成对外服务端口，端口被占用时会在绑定阶段直接启动失败。
/// 取值按无符号十六位整数解析，超出范围或含非数字字符会让整份配置反序列化报错，而不是回落到这个默认端口。
fn default_port() -> u16 {
    8080
}

/// 提供 `JWT_ACCESS_TTL_SECONDS` 缺省值 900 秒，即访问令牌十五分钟过期，同时被用作认证会话存储的超时时间。
/// 调大会延长令牌泄露后的可用窗口，调小则提高刷新频率，两侧都要与前端静默续期节奏一起评估。
fn default_access_ttl() -> u64 {
    900
}

/// 提供 `JWT_REFRESH_TTL_SECONDS` 缺省值 2592000 秒，即刷新令牌三十天有效，决定用户免重新登录的最长跨度。
/// 该值远大于访问令牌有效期，缩短它会让活跃用户被迫重新输入口令，属于影响面较大的安全策略调整。
fn default_refresh_ttl() -> u64 {
    2_592_000
}

/// 提供 `MARKET_FEED_RECONNECT_SECONDS` 缺省值 5 秒，作为行情长连接断开后重新建连前的等待间隔。
/// 该间隔只影响重连节奏，不改变订阅内容；设得过小会在交易所限流或封禁期间放大失败连接次数。
fn default_market_feed_reconnect_seconds() -> u64 {
    5
}

/// 提供 `MARKET_FEED_REST_FALLBACK_TIMEOUT_SECONDS` 缺省值 3 秒，限定实时推送不可用时改走 REST 取数的等待上限。
/// 超时只让当轮兜底放弃，不会终止行情订阅循环；调大能提高弱网下的成功率，但会同步拖慢单轮行情刷新。
fn default_market_feed_rest_fallback_timeout_seconds() -> u64 {
    3
}

/// 提供 `COINBASE_REST_BASE_URL` 缺省值，指向 Coinbase 官方 REST 域名，用于行情快照与历史数据的请求前缀。
/// 与必填的 Bitget、火币地址不同，Coinbase 两项允许留空，因此只接入前两家数据源时无需额外配置。
fn default_coinbase_rest_base_url() -> String {
    "https://api.coinbase.com".to_owned()
}

/// 提供 `COINBASE_WS_URL` 缺省值，指向 Coinbase 高级交易的实时推送域名，供行情订阅建立长连接。
/// 改指自建代理或沙箱时必须同时覆盖对应的 REST 地址，否则实时流与快照来自不同环境，会导致价格口径不一致。
fn default_coinbase_ws_url() -> String {
    "wss://advanced-trade-ws.coinbase.com".to_owned()
}

/// 提供 `EVENT_INBOX_RETRY_SCAN_SECONDS` 缺省值 10 秒，即事件 inbox 重试扫描两轮之间的休眠时长。
/// 它只在消费者启动配置没有单独指定扫描间隔时作为兜底，决定处理失败的事件被重新捡起的最短延迟。
fn default_event_inbox_retry_scan_seconds() -> u64 {
    10
}

/// 提供 `EVENT_OUTBOX_PUBLISHER_ENABLED` 缺省值 true，默认拉起 outbox 发布协程把已落库事件投递到消息队列。
/// 开关为真也要 MySQL 与 RabbitMQ 都连接成功才会真正启动；关闭后事件仍会写库，但只会堆积不再对外发布。
fn default_event_outbox_publisher_enabled() -> bool {
    true
}

/// 提供 `EVENT_OUTBOX_PUBLISHER_INTERVAL_SECONDS` 缺省值 5 秒，控制 outbox 每轮扫描待发事件的间隔。
/// 它直接构成事件从落库到进入队列的平均延迟，缩短能提升下游实时性，同时也会加重数据库轮询压力。
fn default_event_outbox_publisher_interval_seconds() -> u64 {
    5
}

/// 提供 `UNLOCK_SCANNER_ENABLED` 缺省值 true，默认启动锁仓解禁扫描协程，让到期资产自动转为可用余额。
/// 启动还要求 MySQL 已连接；关闭后到期解禁不再自动发生，只能依靠人工或其他运维手段补偿，用户会感知资产被卡住。
fn default_unlock_scanner_enabled() -> bool {
    true
}

/// 提供 `UNLOCK_SCANNER_INTERVAL_SECONDS` 缺省值 10 秒，是解禁扫描两轮之间的休眠时长。
/// 该间隔决定资产实际解禁相对到期时刻的最大滞后，与批量上限共同决定单位时间内能消化多少条待解禁记录。
fn default_unlock_scanner_interval_seconds() -> u64 {
    10
}

/// 提供 `UNLOCK_SCANNER_BATCH_LIMIT` 缺省值 100，限制单轮解禁扫描最多取出并处理的记录条数。
/// 上限用于控制单次数据库压力与事务规模；积压超过上限时会顺延到后续轮次，不会在同一轮里全部消化完。
fn default_unlock_scanner_batch_limit() -> u32 {
    100
}

/// 提供 `KLINE_RECOVERY_ENABLED` 缺省值 true，该开关现已被复用为模拟行情实时循环的总开关，不再表示历史补数。
/// 真正启动还要求 MySQL、Mongo 与 Redis 同时可用，缺任一项只打印告警；关闭后当前分钟的模拟 K 线不再生成。
fn default_kline_recovery_enabled() -> bool {
    true
}

/// 提供 `KLINE_RECOVERY_INTERVAL_SECONDS` 缺省值 30 秒，属于仅为兼容旧部署而保留解析的历史配置项。
/// 模拟行情循环固定按一秒节奏运行并不读取该值，它只会随启动日志打印出来，用于确认旧配置是否还留在环境里。
fn default_kline_recovery_interval_seconds() -> u64 {
    30
}

/// 提供 `KLINE_RECOVERY_BATCH_LIMIT` 缺省值 100，当前语义是模拟行情单轮最多处理的策略数量上限。
/// 策略数超过上限时本轮只处理其中一部分，其余等待下一轮；调大能覆盖更多交易对，但会拉长单轮耗时。
fn default_kline_recovery_batch_limit() -> u32 {
    100
}

/// 提供 `SECONDS_CONTRACT_SETTLEMENT_ENABLED` 缺省值 true，默认拉起秒合约到期结算协程判定用户输赢。
/// 启动同时依赖 MySQL 与 Redis；关闭后到期订单会停在未结算状态，用户资金既不入账也不释放，属于严重可感故障。
fn default_seconds_contract_settlement_enabled() -> bool {
    true
}

/// 提供 `SECONDS_CONTRACT_SETTLEMENT_INTERVAL_SECONDS` 缺省值 5 秒，控制秒合约到期结算的轮询频率。
/// 秒级产品对结算及时性最敏感，该间隔会直接叠加成到期后出结果的额外延迟，一般不建议再往上调。
fn default_seconds_contract_settlement_interval_seconds() -> u64 {
    5
}

/// 提供 `SECONDS_CONTRACT_SETTLEMENT_BATCH_LIMIT` 缺省值 100，限制单轮结算最多领取的到期订单条数。
/// 高峰期到期量超过上限时会分多轮逐批消化，因此这个值要和轮询间隔一起评估，确认峰值吞吐是否足够。
fn default_seconds_contract_settlement_batch_limit() -> u32 {
    100
}

/// 提供 `EARN_AUTO_REDEMPTION_ENABLED` 缺省值 true，默认启动理财到期自动赎回协程归还本金与收益。
/// 该协程只要求 MySQL 可用；关闭后到期理财需要人工赎回，用户资金会继续滞留在产品持仓中无法动用。
fn default_earn_auto_redemption_enabled() -> bool {
    true
}

/// 提供 `EARN_AUTO_REDEMPTION_INTERVAL_SECONDS` 缺省值 60 秒，是理财自动赎回的轮询周期。
/// 理财到期按日粒度推进，分钟级轮询足够覆盖；该值只影响到账时刻的分钟级抖动，不改变收益本身的计算口径。
fn default_earn_auto_redemption_interval_seconds() -> u64 {
    60
}

/// 提供 `EARN_AUTO_REDEMPTION_BATCH_LIMIT` 缺省值 100，限制单轮自动赎回处理的到期订单条数。
/// 集中到期日容易一次性堆出大量订单，保留上限是为了避免单轮长事务长时间占用钱包相关写入路径。
fn default_earn_auto_redemption_batch_limit() -> u32 {
    100
}

/// 提供 `MARGIN_LIQUIDATION_ENABLED` 缺省值 true，默认启动杠杆强平巡检协程处置风险率越线的仓位。
/// 该协程同时依赖 MySQL 与 Redis 行情；关闭等于放弃自动风控，穿仓损失将由平台兜底，只适合短暂排障使用。
fn default_margin_liquidation_enabled() -> bool {
    true
}

/// 提供 `MARGIN_LIQUIDATION_INTERVAL_SECONDS` 缺省值 5 秒，决定杠杆强平巡检的扫描频率。
/// 间隔越大，价格剧烈波动时从触及强平线到真正平仓的滞后越长，账户被击穿到负权益的概率也随之上升。
fn default_margin_liquidation_interval_seconds() -> u64 {
    5
}

/// 提供 `MARGIN_LIQUIDATION_BATCH_LIMIT` 缺省值 100，限制单轮巡检最多处理的风险仓位数量。
/// 极端行情下待处置仓位可能远超上限，此时只能按轮次分批清算，需要结合扫描间隔判断整体清算能力是否够用。
fn default_margin_liquidation_batch_limit() -> u32 {
    100
}

/// 提供 `MARGIN_INTEREST_ENABLED` 缺省值 true，默认启动杠杆计息协程为借币仓位累计利息。
/// 与强平不同，它只需要 MySQL 连接池即可运行，不依赖共享状态里的其他资源；关闭后借币期间不再产生新利息。
fn default_margin_interest_enabled() -> bool {
    true
}

/// 提供 `MARGIN_INTEREST_INTERVAL_SECONDS` 缺省值 60 秒，是杠杆计息任务两轮之间的等待时长。
/// 该值只控制检查频率，实际计息口径与费率由计息任务内部规则决定；worker 会把小于一秒的取值抬到一秒。
fn default_margin_interest_interval_seconds() -> u64 {
    60
}

/// 提供 `MARGIN_INTEREST_BATCH_LIMIT` 缺省值 100，限制单轮计息扫描取出的借币仓位条数。
/// 借币账户较多时按批推进，上限过小会导致一轮覆盖不完全量账户，需要结合轮询周期确认计息不会持续落后。
fn default_margin_interest_batch_limit() -> u32 {
    100
}

/// 提供 `AGENT_COMMISSION_AUTO_SETTLE_ENABLED` 缺省值 false，是本文件里唯一默认关闭的后台任务开关。
/// 佣金结算会直接向代理账户打款，因此默认要求后台人工审核发放，只有显式置为真才会拉起自动结算协程。
fn default_agent_commission_auto_settle_enabled() -> bool {
    false
}

/// 提供 `AGENT_COMMISSION_AUTO_SETTLE_INTERVAL_SECONDS` 缺省值 60 秒，是佣金自动结算的轮询周期。
/// 仅在自动结算开关打开且 MySQL 可用时才有意义；佣金入账本身允许分钟级延迟，不需要压到秒级。
fn default_agent_commission_auto_settle_interval_seconds() -> u64 {
    60
}

/// 提供 `AGENT_COMMISSION_AUTO_SETTLE_MIN_AGE_SECONDS` 缺省值 3600 秒，要求佣金记录至少存在一小时才允许自动结算。
/// 这段静默期留给撤单、回滚与风控复核，避免刚产生的佣金在其对应的原始交易被推翻之前就已经发放出去。
fn default_agent_commission_auto_settle_min_age_seconds() -> u64 {
    3600
}

/// 提供 `AGENT_COMMISSION_AUTO_SETTLE_BATCH_LIMIT` 缺省值 100，限制单轮自动结算处理的佣金记录条数。
/// 该上限同时约束一轮内产生的账户变动笔数，超出部分留到后续轮次，防止批量打款形成过长事务阻塞其他写入。
fn default_agent_commission_auto_settle_batch_limit() -> u32 {
    100
}

#[cfg(test)]
#[path = "../tests/unit_src/src_config_tests.rs"]
mod tests;
