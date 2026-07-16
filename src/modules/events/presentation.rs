//! events bounded context presentation layer.
//!
//! 表现层：负责请求/响应 DTO 与传输层格式转换。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的业务逻辑逐步迁入。

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct PrivateWsQuery {
    pub(crate) token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PublicWsCommand {
    pub(crate) op: String,
    pub(crate) channel: String,
    pub(crate) symbol: Option<String>,
    pub(crate) interval: Option<String>,
}
