//! MongoDB 基础设施：承载按交易对分集合存放的 K 线历史，是行情读写与索引约束的共同入口。
//! 集合名一律由已验证交易对推导，避免未经校验的用户输入直接参与集合寻址，也保证读写双方命名口径一致。
//! 唯一索引把周期与开盘时间组合成 K 线的天然幂等键，行情补写与重放依赖它做去重，缺失索引会导致重复数据。
//! 本文件不定义文档结构，也不提供查询封装，具体读写留在 market 上下文自己的基础设施层。

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

/// 使用已验证交易对生成 K 线集合名，统一委托 market 基础设施实现，避免读写双方各自拼出不同名字。
/// 入参类型本身即证明交易对已通过校验，因此这里不再做二次清洗，也不会因非法字符而失败。
pub fn kline_collection_name(symbol: &ValidatedMarketSymbol) -> String {
    crate::modules::market::kline_collection_name(symbol)
}

/// 仅为兼容旧调用方保留的交易对文本清洗入口，直接转发给 market 的清洗函数，不做任何额外判断。
/// 它返回的只是字符串而非已验证类型，因此不能作为合法性凭据；凡涉及集合访问都应先构造已验证交易对，
/// 避免未经校验的名称被拼进集合名进而访问到非预期的 MongoDB 集合。
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

/// 构造 K 线唯一索引定义：以周期加开盘时间为复合键，索引名固定为常量声明的稳定值。
/// 单独抽出来是为了让启动时的建索引流程和测试断言引用同一份定义，避免两边字段顺序或选项悄悄分叉。
/// 本函数只描述索引结构，不连接数据库也不创建索引，真正的持久化副作用由建索引入口触发。
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
