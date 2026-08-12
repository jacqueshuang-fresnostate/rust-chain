use crate::{
    config::Settings,
    error::AppResult,
    modules::market::{ValidatedMarketSymbol, sanitize_symbol},
};
use mongodb::{Client, Database, IndexModel, bson::doc, options::IndexOptions};

pub const KLINE_UNIQUE_INDEX_NAME: &str = "interval_open_time_unique";

/// 使用暴露后的 MongoDB URI 创建客户端并返回配置命名的数据库句柄；本入口不执行 ping、鉴权降级、建集合或建索引。
/// URI/客户端初始化错误直接上抛；返回成功只表示句柄已构造，首次网络 I/O 的错误仍由具体读写或索引操作返回。
pub async fn connect(settings: &Settings) -> AppResult<Database> {
    let client = Client::with_uri_str(settings.exposed_mongodb_uri()).await?;
    Ok(client.database(&settings.mongodb_database))
}

/// 使用已验证交易对生成 K 线集合名，统一委托 market 基础设施，避免读写双方采用不同命名规则。
pub fn kline_collection_name(symbol: &ValidatedMarketSymbol) -> String {
    crate::modules::market::kline_collection_name(symbol)
}

/// 仅为兼容调用方规范化交易对文本；涉及集合访问时应先构造 `ValidatedMarketSymbol`，避免未校验名称进入 MongoDB。
pub fn normalize_symbol(symbol: &str) -> String {
    sanitize_symbol(symbol)
}

/// 在已验证交易对对应集合上创建唯一索引 `interval_open_time_unique`，把周期与开盘时间固定为 K 线重放/upsert 幂等键。
/// 创建请求是外部持久化副作用；Mongo 错误原样返回，调用方应阻止依赖该约束的写入启动，不使用无索引降级。
pub async fn ensure_kline_indexes(db: &Database, symbol: &ValidatedMarketSymbol) -> AppResult<()> {
    let collection = db.collection::<mongodb::bson::Document>(&kline_collection_name(symbol));
    collection.create_index(kline_unique_index_model()).await?;
    Ok(())
}

/// 构造稳定命名的 K 线唯一索引定义，供启动初始化与测试共享同一持久化约束。
pub fn kline_unique_index_model() -> IndexModel {
    IndexModel::builder()
        .keys(doc! { "interval": 1, "open_time": 1 })
        .options(
            IndexOptions::builder()
                .name(KLINE_UNIQUE_INDEX_NAME.to_owned())
                .unique(true)
                .build(),
        )
        .build()
}
