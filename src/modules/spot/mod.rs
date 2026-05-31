use crate::modules::wallet::{
    FreezeBalanceCommand, LedgerMetadata, SettleBalanceCommand, UnfreezeBalanceCommand,
    WalletRepository, WalletService, WalletServiceError,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{MySql, Pool};

pub mod routes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct TradingPairRule {
    pub pair_id: String,
    pub price_precision: u32,
    pub quantity_precision: u32,
    pub min_order_value: BigDecimal,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrder {
    pub user_id: String,
    pub pair_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<BigDecimal>,
    pub quantity: BigDecimal,
    pub filled_quantity: BigDecimal,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotOrder {
    pub id: String,
    pub user_id: String,
    pub pair_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<BigDecimal>,
    pub quantity: BigDecimal,
    pub filled_quantity: BigDecimal,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSpotTrade {
    pub pair_id: String,
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    pub fee: BigDecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotTrade {
    pub id: String,
    pub pair_id: String,
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    pub fee: BigDecimal,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotDomainError {
    TradingPairDisabled,
    LimitOrderRequiresPrice,
    MarketOrderRejectsPrice,
    NonPositivePrice,
    NonPositiveQuantity,
    PricePrecisionExceeded {
        allowed: u32,
    },
    QuantityPrecisionExceeded {
        allowed: u32,
    },
    MinOrderValueNotMet {
        actual: BigDecimal,
        minimum: BigDecimal,
    },
    InvalidStatusTransition {
        from: OrderStatus,
        to: OrderStatus,
    },
    FillQuantityExceedsRemaining {
        remaining: BigDecimal,
        fill: BigDecimal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotServiceError {
    Domain(SpotDomainError),
    Wallet(WalletServiceError),
    MissingPriceForWalletReservation,
    MissingReferencePriceForMarketOrder,
    Repository(String),
}

impl From<SpotDomainError> for SpotServiceError {
    fn from(error: SpotDomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<WalletServiceError> for SpotServiceError {
    fn from(error: WalletServiceError) -> Self {
        Self::Wallet(error)
    }
}

pub trait SpotRepository {
    fn load_pair_rule(&mut self, pair_id: &str) -> Result<TradingPairRule, SpotServiceError>;

    fn insert_order(
        &mut self,
        new_order: NewOrder,
        idempotency_key: Option<&str>,
    ) -> Result<SpotOrder, SpotServiceError>;

    fn load_order(&mut self, order_id: &str) -> Result<SpotOrder, SpotServiceError>;

    fn save_order(&mut self, order: SpotOrder) -> Result<(), SpotServiceError>;
}

#[derive(Debug, Clone)]
pub struct MySqlSpotRepository {
    pool: Pool<MySql>,
}

impl SpotRepository for MySqlSpotRepository {
    fn load_pair_rule(&mut self, _pair_id: &str) -> Result<TradingPairRule, SpotServiceError> {
        Err(SpotServiceError::Repository(
            "MySqlSpotRepository requires async SQLx methods".to_owned(),
        ))
    }

    fn insert_order(
        &mut self,
        _new_order: NewOrder,
        _idempotency_key: Option<&str>,
    ) -> Result<SpotOrder, SpotServiceError> {
        Err(SpotServiceError::Repository(
            "MySqlSpotRepository requires async SQLx methods".to_owned(),
        ))
    }

    fn load_order(&mut self, _order_id: &str) -> Result<SpotOrder, SpotServiceError> {
        Err(SpotServiceError::Repository(
            "MySqlSpotRepository requires async SQLx methods".to_owned(),
        ))
    }

    fn save_order(&mut self, _order: SpotOrder) -> Result<(), SpotServiceError> {
        Err(SpotServiceError::Repository(
            "MySqlSpotRepository requires async SQLx methods".to_owned(),
        ))
    }
}

impl MySqlSpotRepository {
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    pub async fn load_pair_rule_async(
        &self,
        pair_id: &str,
    ) -> Result<TradingPairRule, SpotServiceError> {
        let row = sqlx::query_as::<_, (u64, String, i32, i32, BigDecimal, String)>(
            r#"SELECT id, symbol, price_precision, qty_precision, min_order_value, status
               FROM trading_pairs
               WHERE symbol = ? OR id = ?
               LIMIT 1"#,
        )
        .bind(pair_id)
        .bind(pair_id.parse::<u64>().ok())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?
        .ok_or_else(|| SpotServiceError::Repository(format!("missing trading pair: {pair_id}")))?;

        let (_id, symbol, price_precision, quantity_precision, min_order_value, status) = row;
        Ok(TradingPairRule {
            pair_id: symbol,
            price_precision: price_precision as u32,
            quantity_precision: quantity_precision as u32,
            min_order_value,
            enabled: status == "active",
        })
    }

    pub async fn insert_order_async(
        &self,
        new_order: NewOrder,
        idempotency_key: Option<&str>,
    ) -> Result<SpotOrder, SpotServiceError> {
        let user_id = parse_spot_u64_identifier("user_id", &new_order.user_id)?;
        let pair_db_id = resolve_pair_id(&self.pool, &new_order.pair_id).await?;
        let result = sqlx::query(
            r#"INSERT INTO spot_orders
               (user_id, pair_id, side, order_type, price, quantity, filled_quantity, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE id = LAST_INSERT_ID(id)"#,
        )
        .bind(user_id)
        .bind(pair_db_id)
        .bind(order_side_as_str(new_order.side))
        .bind(order_type_as_str(new_order.order_type))
        .bind(&new_order.price)
        .bind(&new_order.quantity)
        .bind(&new_order.filled_quantity)
        .bind(order_status_as_str(new_order.status))
        .bind(idempotency_key)
        .execute(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;

        self.load_order_async(&result.last_insert_id().to_string())
            .await
    }

    pub async fn load_order_async(&self, order_id: &str) -> Result<SpotOrder, SpotServiceError> {
        let order_db_id = parse_spot_u64_identifier("order_id", order_id)?;
        let row = sqlx::query_as::<
            _,
            (
                u64,
                u64,
                String,
                String,
                String,
                Option<BigDecimal>,
                BigDecimal,
                BigDecimal,
                String,
            ),
        >(
            r#"SELECT orders.id, orders.user_id, pairs.symbol, orders.side, orders.order_type,
                      orders.price, orders.quantity, orders.filled_quantity, orders.status
               FROM spot_orders orders
               INNER JOIN trading_pairs pairs ON pairs.id = orders.pair_id
               WHERE orders.id = ?
               LIMIT 1"#,
        )
        .bind(order_db_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?
        .ok_or_else(|| SpotServiceError::Repository(format!("missing spot order: {order_id}")))?;

        spot_order_from_row(row)
    }

    pub async fn save_order_async(&self, order: SpotOrder) -> Result<(), SpotServiceError> {
        let order_db_id = parse_spot_u64_identifier("order_id", &order.id)?;
        let pair_db_id = resolve_pair_id(&self.pool, &order.pair_id).await?;
        sqlx::query(
            r#"UPDATE spot_orders
               SET pair_id = ?, side = ?, order_type = ?, price = ?, quantity = ?,
                   filled_quantity = ?, status = ?
               WHERE id = ?"#,
        )
        .bind(pair_db_id)
        .bind(order_side_as_str(order.side))
        .bind(order_type_as_str(order.order_type))
        .bind(order.price)
        .bind(order.quantity)
        .bind(order.filled_quantity)
        .bind(order_status_as_str(order.status))
        .bind(order_db_id)
        .execute(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;

        Ok(())
    }

    pub async fn insert_trade_async(
        &self,
        trade: NewSpotTrade,
    ) -> Result<SpotTrade, SpotServiceError> {
        let pair_db_id = resolve_pair_id(&self.pool, &trade.pair_id).await?;
        let buy_order_id = parse_spot_u64_identifier("buy_order_id", &trade.buy_order_id)?;
        let sell_order_id = parse_spot_u64_identifier("sell_order_id", &trade.sell_order_id)?;
        let result = sqlx::query(
            r#"INSERT INTO spot_trades
               (pair_id, buy_order_id, sell_order_id, price, quantity, fee)
               VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(pair_db_id)
        .bind(buy_order_id)
        .bind(sell_order_id)
        .bind(&trade.price)
        .bind(&trade.quantity)
        .bind(&trade.fee)
        .execute(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;

        self.load_trade_by_id_async(result.last_insert_id()).await
    }

    pub async fn list_trades_by_pair_async(
        &self,
        pair_id: &str,
        limit: u32,
    ) -> Result<Vec<SpotTrade>, SpotServiceError> {
        let pair_db_id = resolve_pair_id(&self.pool, pair_id).await?;
        let rows = sqlx::query_as::<
            _,
            (
                u64,
                String,
                u64,
                u64,
                BigDecimal,
                BigDecimal,
                BigDecimal,
                DateTime<Utc>,
            ),
        >(
            r#"SELECT trades.id, pairs.symbol, trades.buy_order_id, trades.sell_order_id,
                      trades.price, trades.quantity, trades.fee, trades.created_at
               FROM spot_trades trades
               INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
               WHERE trades.pair_id = ?
               ORDER BY trades.id DESC
               LIMIT ?"#,
        )
        .bind(pair_db_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;

        rows.into_iter().map(spot_trade_from_row).collect()
    }

    async fn load_trade_by_id_async(&self, trade_id: u64) -> Result<SpotTrade, SpotServiceError> {
        let row = sqlx::query_as::<
            _,
            (
                u64,
                String,
                u64,
                u64,
                BigDecimal,
                BigDecimal,
                BigDecimal,
                DateTime<Utc>,
            ),
        >(
            r#"SELECT trades.id, pairs.symbol, trades.buy_order_id, trades.sell_order_id,
                      trades.price, trades.quantity, trades.fee, trades.created_at
               FROM spot_trades trades
               INNER JOIN trading_pairs pairs ON pairs.id = trades.pair_id
               WHERE trades.id = ?
               LIMIT 1"#,
        )
        .bind(trade_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_spot_sqlx_error)?;

        spot_trade_from_row(row)
    }
}

fn spot_order_from_row(
    row: (
        u64,
        u64,
        String,
        String,
        String,
        Option<BigDecimal>,
        BigDecimal,
        BigDecimal,
        String,
    ),
) -> Result<SpotOrder, SpotServiceError> {
    let (id, user_id, pair_id, side, order_type, price, quantity, filled_quantity, status) = row;

    Ok(SpotOrder {
        id: id.to_string(),
        user_id: user_id.to_string(),
        pair_id,
        side: order_side_from_str(&side)?,
        order_type: order_type_from_str(&order_type)?,
        price,
        quantity,
        filled_quantity,
        status: order_status_from_str(&status)?,
    })
}

fn spot_trade_from_row(
    row: (
        u64,
        String,
        u64,
        u64,
        BigDecimal,
        BigDecimal,
        BigDecimal,
        DateTime<Utc>,
    ),
) -> Result<SpotTrade, SpotServiceError> {
    let (id, pair_id, buy_order_id, sell_order_id, price, quantity, fee, created_at) = row;
    Ok(SpotTrade {
        id: id.to_string(),
        pair_id,
        buy_order_id: buy_order_id.to_string(),
        sell_order_id: sell_order_id.to_string(),
        price,
        quantity,
        fee,
        created_at,
    })
}

async fn resolve_pair_id(pool: &Pool<MySql>, pair_id: &str) -> Result<u64, SpotServiceError> {
    if let Ok(pair_db_id) = pair_id.parse::<u64>() {
        return Ok(pair_db_id);
    }

    sqlx::query_as::<_, (u64,)>("SELECT id FROM trading_pairs WHERE symbol = ? LIMIT 1")
        .bind(pair_id)
        .fetch_optional(pool)
        .await
        .map_err(map_spot_sqlx_error)?
        .map(|(id,)| id)
        .ok_or_else(|| SpotServiceError::Repository(format!("missing trading pair: {pair_id}")))
}

fn order_side_as_str(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

fn order_side_from_str(value: &str) -> Result<OrderSide, SpotServiceError> {
    match value {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        _ => Err(SpotServiceError::Repository(format!(
            "unknown order side: {value}"
        ))),
    }
}

fn order_type_as_str(order_type: OrderType) -> &'static str {
    match order_type {
        OrderType::Limit => "limit",
        OrderType::Market => "market",
    }
}

fn order_type_from_str(value: &str) -> Result<OrderType, SpotServiceError> {
    match value {
        "limit" => Ok(OrderType::Limit),
        "market" => Ok(OrderType::Market),
        _ => Err(SpotServiceError::Repository(format!(
            "unknown order type: {value}"
        ))),
    }
}

fn order_status_as_str(status: OrderStatus) -> &'static str {
    match status {
        OrderStatus::Pending => "pending",
        OrderStatus::Open => "open",
        OrderStatus::PartiallyFilled => "partially_filled",
        OrderStatus::Filled => "filled",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Rejected => "rejected",
    }
}

fn order_status_from_str(value: &str) -> Result<OrderStatus, SpotServiceError> {
    match value {
        "pending" => Ok(OrderStatus::Pending),
        "open" => Ok(OrderStatus::Open),
        "partially_filled" => Ok(OrderStatus::PartiallyFilled),
        "filled" => Ok(OrderStatus::Filled),
        "cancelled" => Ok(OrderStatus::Cancelled),
        "rejected" => Ok(OrderStatus::Rejected),
        _ => Err(SpotServiceError::Repository(format!(
            "unknown order status: {value}"
        ))),
    }
}

fn parse_spot_u64_identifier(field: &str, value: &str) -> Result<u64, SpotServiceError> {
    value.parse::<u64>().map_err(|error| {
        SpotServiceError::Repository(format!("invalid numeric {field} `{value}`: {error}"))
    })
}

fn map_spot_sqlx_error(error: sqlx::Error) -> SpotServiceError {
    SpotServiceError::Repository(error.to_string())
}

#[derive(Debug, Clone)]
pub struct CreateSpotOrderCommand {
    pub user_id: String,
    pub pair_id: String,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<BigDecimal>,
    pub quantity: BigDecimal,
    pub reference_price: Option<BigDecimal>,
    pub idempotency_key: Option<String>,
    pub wallet_ledger: LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct CancelSpotOrderCommand {
    pub order_id: String,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub wallet_ledger: LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct FillSpotOrderCommand {
    pub order_id: String,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub fill_price: BigDecimal,
    pub fill_quantity: BigDecimal,
    pub wallet_ledger: LedgerMetadata,
}

#[derive(Debug, Clone)]
pub struct SpotService<S, W> {
    spot_repository: S,
    wallet_service: WalletService<W>,
}

impl<S, W> SpotService<S, W> {
    pub fn new(spot_repository: S, wallet_repository: W) -> Self {
        Self {
            spot_repository,
            wallet_service: WalletService::new(wallet_repository),
        }
    }

    pub fn into_repositories(self) -> (S, W) {
        (self.spot_repository, self.wallet_service.into_repository())
    }
}

impl<S: SpotRepository, W: WalletRepository> SpotService<S, W> {
    pub fn create_order(
        &mut self,
        command: CreateSpotOrderCommand,
    ) -> Result<SpotOrder, SpotServiceError> {
        let pair = self.spot_repository.load_pair_rule(&command.pair_id)?;
        let new_order = match command.order_type {
            OrderType::Limit => create_limit_order(
                command.user_id.clone(),
                command.side,
                command
                    .price
                    .clone()
                    .ok_or(SpotServiceError::MissingPriceForWalletReservation)?,
                command.quantity.clone(),
                &pair,
            )?,
            OrderType::Market => create_market_order(
                command.user_id.clone(),
                command.side,
                command.quantity.clone(),
                command
                    .reference_price
                    .clone()
                    .ok_or(SpotServiceError::MissingReferencePriceForMarketOrder)?,
                &pair,
            )?,
        };

        let reservation_price =
            command
                .price
                .or(command.reference_price)
                .ok_or(match command.order_type {
                    OrderType::Limit => SpotServiceError::MissingPriceForWalletReservation,
                    OrderType::Market => SpotServiceError::MissingReferencePriceForMarketOrder,
                })?;
        let reserve_amount =
            reservation_amount(command.side, &reservation_price, &command.quantity);
        let reserve_asset_id = reserve_asset_id(
            command.side,
            &command.base_asset_id,
            &command.quote_asset_id,
        );
        self.wallet_service.freeze(FreezeBalanceCommand {
            user_id: command.user_id,
            asset_id: reserve_asset_id.to_owned(),
            amount: reserve_amount,
            ledger: command.wallet_ledger,
        })?;

        self.spot_repository
            .insert_order(new_order, command.idempotency_key.as_deref())
    }

    pub fn cancel_order(
        &mut self,
        command: CancelSpotOrderCommand,
    ) -> Result<bool, SpotServiceError> {
        let mut order = self.spot_repository.load_order(&command.order_id)?;
        let was_cancelled = cancel_order(&mut order)?;
        if !was_cancelled {
            return Ok(false);
        }

        let remaining_reservation =
            remaining_reserved_amount(&order, &command.base_asset_id, &command.quote_asset_id)?;
        if remaining_reservation.amount > 0 {
            self.wallet_service.unfreeze(UnfreezeBalanceCommand {
                user_id: order.user_id.clone(),
                asset_id: remaining_reservation.asset_id,
                amount: remaining_reservation.amount,
                ledger: command.wallet_ledger,
            })?;
        }
        self.spot_repository.save_order(order)?;
        Ok(true)
    }

    pub fn fill_order(
        &mut self,
        command: FillSpotOrderCommand,
    ) -> Result<SpotOrder, SpotServiceError> {
        let mut order = self.spot_repository.load_order(&command.order_id)?;
        apply_fill(&mut order, command.fill_quantity.clone())?;

        match order.side {
            OrderSide::Buy => {
                self.wallet_service.settle(SettleBalanceCommand {
                    user_id: order.user_id.clone(),
                    debit_frozen_asset_id: command.quote_asset_id,
                    debit_frozen_amount: command.fill_price * command.fill_quantity.clone(),
                    credit_available_asset_id: command.base_asset_id,
                    credit_available_amount: command.fill_quantity,
                    ledger: command.wallet_ledger,
                })?;
            }
            OrderSide::Sell => {
                self.wallet_service.settle(SettleBalanceCommand {
                    user_id: order.user_id.clone(),
                    debit_frozen_asset_id: command.base_asset_id,
                    debit_frozen_amount: command.fill_quantity.clone(),
                    credit_available_asset_id: command.quote_asset_id,
                    credit_available_amount: command.fill_price * command.fill_quantity,
                    ledger: command.wallet_ledger,
                })?;
            }
        }

        self.spot_repository.save_order(order.clone())?;
        Ok(order)
    }
}

pub fn create_limit_order(
    user_id: impl Into<String>,
    side: OrderSide,
    price: BigDecimal,
    quantity: BigDecimal,
    pair: &TradingPairRule,
) -> Result<NewOrder, SpotDomainError> {
    validate_common(&quantity, pair)?;
    validate_price(&price, pair.price_precision)?;
    validate_min_order_value(price.clone() * quantity.clone(), pair)?;

    Ok(NewOrder {
        user_id: user_id.into(),
        pair_id: pair.pair_id.clone(),
        side,
        order_type: OrderType::Limit,
        price: Some(price),
        quantity,
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    })
}

pub fn create_market_order(
    user_id: impl Into<String>,
    side: OrderSide,
    quantity: BigDecimal,
    reference_price: BigDecimal,
    pair: &TradingPairRule,
) -> Result<NewOrder, SpotDomainError> {
    validate_common(&quantity, pair)?;
    validate_price(&reference_price, pair.price_precision)?;
    validate_min_order_value(reference_price * quantity.clone(), pair)?;

    Ok(NewOrder {
        user_id: user_id.into(),
        pair_id: pair.pair_id.clone(),
        side,
        order_type: OrderType::Market,
        price: None,
        quantity,
        filled_quantity: BigDecimal::from(0),
        status: OrderStatus::Pending,
    })
}

pub fn spot_reservation_amount(
    side: OrderSide,
    price: &BigDecimal,
    quantity: &BigDecimal,
) -> BigDecimal {
    reservation_amount(side, price, quantity)
}

pub fn spot_reserve_asset_id<'a>(
    side: OrderSide,
    base_asset_id: &'a str,
    quote_asset_id: &'a str,
) -> &'a str {
    reserve_asset_id(side, base_asset_id, quote_asset_id)
}

pub fn spot_remaining_reserved_amount(
    order: &SpotOrder,
    base_asset_id: &str,
    quote_asset_id: &str,
) -> Result<(String, BigDecimal), SpotServiceError> {
    remaining_reserved_amount(order, base_asset_id, quote_asset_id)
        .map(|reserved| (reserved.asset_id, reserved.amount))
}

pub fn validate_order_request(
    order_type: OrderType,
    price: Option<BigDecimal>,
    quantity: BigDecimal,
    pair: &TradingPairRule,
) -> Result<(), SpotDomainError> {
    match (order_type, price) {
        (OrderType::Limit, Some(price)) => {
            create_limit_order("validation", OrderSide::Buy, price, quantity, pair).map(|_| ())
        }
        (OrderType::Limit, None) => Err(SpotDomainError::LimitOrderRequiresPrice),
        (OrderType::Market, Some(_)) => Err(SpotDomainError::MarketOrderRejectsPrice),
        (OrderType::Market, None) => validate_common(&quantity, pair),
    }
}

pub fn transition_status(
    current: OrderStatus,
    next: OrderStatus,
) -> Result<OrderStatus, SpotDomainError> {
    if can_transition(current, next) {
        Ok(next)
    } else {
        Err(SpotDomainError::InvalidStatusTransition {
            from: current,
            to: next,
        })
    }
}

pub fn cancel_order(order: &mut SpotOrder) -> Result<bool, SpotDomainError> {
    match order.status {
        OrderStatus::Cancelled => Ok(false),
        OrderStatus::Pending | OrderStatus::Open | OrderStatus::PartiallyFilled => {
            order.status = OrderStatus::Cancelled;
            Ok(true)
        }
        status => Err(SpotDomainError::InvalidStatusTransition {
            from: status,
            to: OrderStatus::Cancelled,
        }),
    }
}

pub fn apply_fill(order: &mut SpotOrder, fill_quantity: BigDecimal) -> Result<(), SpotDomainError> {
    if fill_quantity <= 0 {
        return Err(SpotDomainError::NonPositiveQuantity);
    }

    let remaining = order.quantity.clone() - order.filled_quantity.clone();
    if fill_quantity > remaining {
        return Err(SpotDomainError::FillQuantityExceedsRemaining {
            remaining,
            fill: fill_quantity,
        });
    }

    let next_filled = order.filled_quantity.clone() + fill_quantity;
    let next_status = if next_filled == order.quantity {
        OrderStatus::Filled
    } else {
        OrderStatus::PartiallyFilled
    };

    transition_status(order.status, next_status)?;
    order.filled_quantity = next_filled;
    order.status = next_status;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReservedAmount {
    asset_id: String,
    amount: BigDecimal,
}

fn remaining_reserved_amount(
    order: &SpotOrder,
    base_asset_id: &str,
    quote_asset_id: &str,
) -> Result<ReservedAmount, SpotServiceError> {
    let remaining_quantity = order.quantity.clone() - order.filled_quantity.clone();
    match order.side {
        OrderSide::Buy => {
            let price = order
                .price
                .clone()
                .ok_or(SpotServiceError::MissingPriceForWalletReservation)?;
            Ok(ReservedAmount {
                asset_id: quote_asset_id.to_owned(),
                amount: price * remaining_quantity,
            })
        }
        OrderSide::Sell => Ok(ReservedAmount {
            asset_id: base_asset_id.to_owned(),
            amount: remaining_quantity,
        }),
    }
}

fn reserve_asset_id<'a>(
    side: OrderSide,
    base_asset_id: &'a str,
    quote_asset_id: &'a str,
) -> &'a str {
    match side {
        OrderSide::Buy => quote_asset_id,
        OrderSide::Sell => base_asset_id,
    }
}

fn reservation_amount(side: OrderSide, price: &BigDecimal, quantity: &BigDecimal) -> BigDecimal {
    match side {
        OrderSide::Buy => price.clone() * quantity.clone(),
        OrderSide::Sell => quantity.clone(),
    }
}

fn validate_common(quantity: &BigDecimal, pair: &TradingPairRule) -> Result<(), SpotDomainError> {
    if !pair.enabled {
        return Err(SpotDomainError::TradingPairDisabled);
    }
    if quantity <= &BigDecimal::from(0) {
        return Err(SpotDomainError::NonPositiveQuantity);
    }
    validate_precision(quantity, pair.quantity_precision).map_err(|()| {
        SpotDomainError::QuantityPrecisionExceeded {
            allowed: pair.quantity_precision,
        }
    })
}

fn validate_price(price: &BigDecimal, precision: u32) -> Result<(), SpotDomainError> {
    if price <= &BigDecimal::from(0) {
        return Err(SpotDomainError::NonPositivePrice);
    }
    validate_precision(price, precision)
        .map_err(|()| SpotDomainError::PricePrecisionExceeded { allowed: precision })
}

fn validate_min_order_value(
    actual: BigDecimal,
    pair: &TradingPairRule,
) -> Result<(), SpotDomainError> {
    if actual < pair.min_order_value {
        Err(SpotDomainError::MinOrderValueNotMet {
            actual,
            minimum: pair.min_order_value.clone(),
        })
    } else {
        Ok(())
    }
}

fn validate_precision(amount: &BigDecimal, precision: u32) -> Result<(), ()> {
    let (_, scale) = amount.normalized().as_bigint_and_exponent();
    if scale.max(0) as u32 <= precision {
        Ok(())
    } else {
        Err(())
    }
}

fn can_transition(current: OrderStatus, next: OrderStatus) -> bool {
    use OrderStatus::*;
    matches!(
        (current, next),
        (Pending, Open)
            | (Pending, Cancelled)
            | (Pending, Rejected)
            | (Open, PartiallyFilled)
            | (Open, Filled)
            | (Open, Cancelled)
            | (PartiallyFilled, PartiallyFilled)
            | (PartiallyFilled, Filled)
            | (PartiallyFilled, Cancelled)
            | (Cancelled, Cancelled)
    )
}
