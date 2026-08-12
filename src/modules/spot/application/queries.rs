//! 现货读模型应用用例：统一查询作用域、分页边界和状态池获取。

use crate::{
    error::{AppError, AppResult},
    modules::spot::{
        infrastructure::{
            SpotOrderListFilter, SpotTradeListFilter, list_admin_spot_orders_page,
            list_admin_spot_trades_page, list_spot_orders, list_spot_trades, load_spot_order_by_id,
        },
        presentation::{
            AdminSpotOrdersQuery, AdminSpotOrdersResponse, AdminSpotTradesQuery,
            AdminSpotTradesResponse, SpotOrderResponse, SpotOrdersQuery, SpotOrdersResponse,
            SpotTradesQuery, SpotTradesResponse,
        },
    },
    state::AppState,
};
use sqlx::{MySql, Pool};

/// 获取现货应用用例使用的 MySQL 连接池；调用前应用状态必须完成数据库初始化。
/// 本函数不创建连接、不启动事务也不触碰订单或钱包；配置缺失时返回既有内部错误，且没有持久化或事件副作用。
pub(crate) fn mysql_pool(state: &AppState) -> AppResult<Pool<MySql>> {
    state.mysql.clone().ok_or_else(|| {
        AppError::Internal("mysql pool is not configured for spot routes".to_owned())
    })
}

/// 查询认证用户自己的现货订单读模型；`user_id` 必须来自可信鉴权主体，空白筛选值按未过滤处理。
/// 用例固定注入用户作用域并把条数限制在 1..=100；不启动事务、不加资金锁且不改变订单状态。
/// 数据库失败原样返回，重复查询不冻结/解冻钱包、不写流水也不发布事件。
pub(crate) async fn list_user_spot_orders(
    pool: &Pool<MySql>,
    user_id: u64,
    query: SpotOrdersQuery,
) -> AppResult<SpotOrdersResponse> {
    let orders = list_spot_orders(
        pool,
        SpotOrderListFilter {
            user_id: Some(user_id),
            pair_id: optional_query_string(query.pair_id),
            status: optional_query_string(query.status),
            email: None,
            include_internal: true,
            limit: route_limit(query.limit),
            offset: 0,
        },
    )
    .await?;
    Ok(SpotOrdersResponse { orders })
}

/// 按管理员筛选条件查询现货订单及总数；调用方必须先完成管理员鉴权，内部订单仅在显式请求时返回。
/// 条数限制为 1..=100、偏移限制为 100000，筛选值会去除首尾空白；该读模型不拥有事务或订单/钱包锁。
/// 无匹配项返回空页，数据库错误中止查询；不会修改订单、余额、流水、幂等记录或发布私有事件。
pub(crate) async fn list_admin_spot_orders(
    pool: &Pool<MySql>,
    query: AdminSpotOrdersQuery,
) -> AppResult<AdminSpotOrdersResponse> {
    let (orders, total) = list_admin_spot_orders_page(
        pool,
        SpotOrderListFilter {
            user_id: query.user_id,
            pair_id: optional_query_string(query.pair_id),
            status: optional_query_string(query.status),
            email: optional_query_string(query.email),
            include_internal: query.include_internal == Some(true),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminSpotOrdersResponse { orders, total })
}

/// 查询认证用户作为买方或卖方参与的指定交易对成交；`user_id` 必须来自可信鉴权主体。
/// 用例固定用户隔离和 1..=100 的返回上限，不开启事务、不锁订单或钱包，因而不参与成交结算锁序。
/// 查询失败不产生局部写入；重复调用不追加流水、不改变冻结额也不广播事件。
pub(crate) async fn list_user_spot_trades(
    pool: &Pool<MySql>,
    user_id: u64,
    query: SpotTradesQuery,
) -> AppResult<SpotTradesResponse> {
    let trades = list_spot_trades(
        pool,
        SpotTradeListFilter {
            pair_id: optional_query_string(Some(query.pair_id)),
            user_id: Some(user_id),
            email: None,
            include_internal: true,
            limit: route_limit(query.limit),
            offset: 0,
        },
    )
    .await?;
    Ok(SpotTradesResponse { trades })
}

/// 按数据库订单编号读取管理员现货订单详情；调用方必须已在传输层完成管理员鉴权。
/// 本函数不启动事务、不加订单或钱包锁；订单不存在或数据库失败不产生业务副作用。
/// 该读取不改变幂等状态、预留额、冻结余额、流水或事件发布时间。
pub(crate) async fn get_admin_spot_order(
    pool: &Pool<MySql>,
    order_id: u64,
) -> AppResult<SpotOrderResponse> {
    load_spot_order_by_id(pool, order_id).await
}

/// 按交易对、用户或邮箱查询管理员现货成交页；调用方必须先完成管理员鉴权，内部成交仅在显式请求时返回。
/// 查询值会标准化，条数与偏移分别限制为 100 和 100000；读用例不拥有事务，也不取得订单/钱包锁。
/// 数据库错误整体返回且无写入；重复查询不会重放成交、佣金、资金流水或私有事件。
pub(crate) async fn list_admin_spot_trades(
    pool: &Pool<MySql>,
    query: AdminSpotTradesQuery,
) -> AppResult<AdminSpotTradesResponse> {
    let (trades, total) = list_admin_spot_trades_page(
        pool,
        SpotTradeListFilter {
            pair_id: optional_query_string(query.pair_id),
            user_id: query.user_id,
            email: optional_query_string(query.email),
            include_internal: query.include_internal == Some(true),
            limit: route_limit(query.limit),
            offset: route_offset(query.offset),
        },
    )
    .await?;
    Ok(AdminSpotTradesResponse { trades, total })
}
/// 归一化现货列表条数：缺省为 50，并限制在 1..=100，防止调用方绕过读模型分页边界。
/// 该纯函数不访问数据库/Redis，不涉及事务、锁、幂等、资金预留或事件副作用。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 100)
}

/// 偏移同样设上限：超大 offset 会让日志类大表退化为全表扫描加文件排序。
fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 规范化可选查询字符串：去除首尾空白并把空值折叠为 `None`，供读模型与批量撤单共享筛选语义。
/// 该纯函数不访问持久化、不参与事务/锁序/幂等，也不产生订单、钱包、流水或事件副作用。
pub(super) fn optional_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
