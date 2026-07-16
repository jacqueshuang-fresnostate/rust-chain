//! market bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。

use crate::architecture::RepositoryLayer;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use mongodb::bson::{DateTime as BsonDateTime, oid::ObjectId};
use serde::Deserialize;

#[derive(Debug)]
pub struct RepositoryLayerMarker;

impl RepositoryLayer for RepositoryLayerMarker {}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct SpotTradeRecord {
    pub(crate) id: u64,
    pub(crate) symbol: String,
    pub(crate) price: BigDecimal,
    pub(crate) quantity: BigDecimal,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct KlineDocumentRecord {
    #[serde(rename = "_id")]
    pub(crate) _id: Option<ObjectId>,
    pub(crate) interval: String,
    pub(crate) open_time: BsonDateTime,
    pub(crate) open: String,
    pub(crate) high: String,
    pub(crate) low: String,
    pub(crate) close: String,
    pub(crate) volume: String,
}
