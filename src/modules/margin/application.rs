//! Margin bounded context application compatibility façade.
//!
//! 真实用例按产品配置、账户设置、查询、开仓与生命周期职责拆分；本文件仅保留稳定导入路径。

mod account_settings;
mod lifecycle;
mod open_position;
mod product_config;
mod queries;
mod support;

pub(crate) use account_settings::{
    get_user_margin_setting, transfer_margin_funds, update_user_leverage, update_user_margin_mode,
};
pub(crate) use lifecycle::{
    cancel_all_margin_positions_with_events, cancel_margin_position_with_events,
    close_all_margin_positions_with_events, close_margin_position_with_events,
};
pub(crate) use open_position::open_margin_position_with_events;
pub(crate) use product_config::{
    create_margin_product, get_admin_margin_product, list_active_margin_products,
    list_admin_margin_products, update_margin_product_config, update_margin_product_status,
};
pub(crate) use queries::{
    get_admin_margin_position, get_margin_position_risk_snapshot, get_user_margin_position,
    list_admin_margin_interest_summary, list_admin_margin_position_history,
    list_user_margin_positions, list_user_margin_wallets,
};
pub(crate) use support::{mysql_pool, route_limit};

#[cfg(test)]
pub(crate) use product_config::margin_trading_capabilities;
