use crate::{
    config::Settings,
    error::AppResult,
    modules::market::{ValidatedMarketSymbol, sanitize_symbol},
};
use mongodb::{Client, Database, IndexModel, bson::doc, options::IndexOptions};

pub const KLINE_UNIQUE_INDEX_NAME: &str = "interval_open_time_unique";

/// 连接 MongoDB 并返回配置指定的业务数据库句柄；连接或 URI 错误直接上抛，不创建替代数据库。
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

/// 为指定交易对的 K 线集合建立 interval+open_time 唯一索引，使行情重放和恢复任务只能覆盖同一根蜡烛而不能重复插入。
/// 索引创建失败应阻止依赖该集合的写入启动，避免幂等约束缺失。
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
