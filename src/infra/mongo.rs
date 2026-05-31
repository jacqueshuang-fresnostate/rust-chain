use crate::{
    config::Settings,
    error::AppResult,
    modules::market::{ValidatedMarketSymbol, sanitize_symbol},
};
use mongodb::{Client, Database, IndexModel, bson::doc, options::IndexOptions};

pub const KLINE_UNIQUE_INDEX_NAME: &str = "interval_open_time_unique";

pub async fn connect(settings: &Settings) -> AppResult<Database> {
    let client = Client::with_uri_str(settings.exposed_mongodb_uri()).await?;
    Ok(client.database(&settings.mongodb_database))
}

pub fn kline_collection_name(symbol: &ValidatedMarketSymbol) -> String {
    crate::modules::market::kline_collection_name(symbol)
}

pub fn normalize_symbol(symbol: &str) -> String {
    sanitize_symbol(symbol)
}

pub async fn ensure_kline_indexes(db: &Database, symbol: &ValidatedMarketSymbol) -> AppResult<()> {
    let collection = db.collection::<mongodb::bson::Document>(&kline_collection_name(symbol));
    collection.create_index(kline_unique_index_model()).await?;
    Ok(())
}

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
