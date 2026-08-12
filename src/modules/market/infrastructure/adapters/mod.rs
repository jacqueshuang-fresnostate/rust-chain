//! 行情 provider 与 ingestion 适配器兼容 façade。
//!
//! provider 模块负责第三方协议归一化，feed 模块负责帧流/REST 兜底编排，ingestion 模块
//! 负责把已验证快照写入权威 Redis/Mongo。公开项继续从 `market::adapters` 暴露。

mod feed;
mod ingestion;
mod provider;

pub use feed::*;
pub use ingestion::*;
pub use provider::*;
