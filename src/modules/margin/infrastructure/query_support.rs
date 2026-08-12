use crate::error::AppResult;
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder};
/// 裁剪可选文本并把空白值归一为空，供保证金筛选和配置校验共享。
pub(super) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 分页排序必须带唯一列 id，否则同一排序值的行会在页间重复或丢失。
pub(super) const MARGIN_PRODUCT_ORDER_BY: &str = " ORDER BY products.id DESC";

/// 行查询与 COUNT 查询必须由同一组过滤谓词构建，返回总数才能与当前筛选一致。
/// 行查询与 COUNT 查询复用同一筛选构建器；任一失败整体返回，避免列表与总数口径分裂。
pub(super) async fn fetch_admin_page<T>(
    pool: &Pool<MySql>,
    mut rows: QueryBuilder<'_, MySql>,
    mut total: QueryBuilder<'_, MySql>,
    order_by: &str,
    limit: u32,
    offset: u32,
) -> AppResult<(Vec<T>, i64)>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
{
    rows.push(order_by);
    rows.push(" LIMIT ");
    rows.push_bind(limit as i64);
    rows.push(" OFFSET ");
    rows.push_bind(offset as i64);

    let items = rows.build_query_as::<T>().fetch_all(pool).await?;
    let total = total.build_query_scalar::<i64>().fetch_one(pool).await?;

    Ok((items, total))
}

/// 向仓位后台查询追加参数化邮箱条件，避免字符串拼接并保持行查询与计数一致。
pub(super) fn push_user_email_filter(
    builder: &mut QueryBuilder<'_, MySql>,
    user_id_column: &'static str,
    email: Option<String>,
) {
    if let Some(email) = optional_string(email) {
        builder.push(" AND EXISTS (SELECT 1 FROM users WHERE users.id = ");
        builder.push(user_id_column);
        builder.push(" AND users.email = ");
        builder.push_bind(email);
        builder.push(")");
    }
}

/// 生成与 `DECIMAL(38,18)` 持久化一致的零金额；纯函数不读写账户或流水。
pub(super) fn zero_amount() -> BigDecimal {
    BigDecimal::from(0).with_scale(18)
}
