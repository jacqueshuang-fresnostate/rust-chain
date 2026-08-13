//! 杠杆基础设施层的共享查询构建助手。
//!
//! 提供后台分页的统一执行入口、可复用的邮箱筛选片段、稳定排序常量和资金零值构造。
//! 存在的意义是让所有后台列表共用同一套「行查询与 COUNT 复用谓词」的写法，
//! 避免各处自行拼装导致明细与分页总数口径分裂，或漏掉唯一列排序造成翻页重复。
//! 本文件不定义事务边界，全部函数在调用方给定的连接池上执行且都是只读的。

use crate::error::AppResult;
use bigdecimal::BigDecimal;
use sqlx::{MySql, Pool, QueryBuilder};
/// 裁剪可选文本两端空白，并把裁剪后为空的值折叠成 None，消除「传了但等于没传」的中间态。
/// 基础设施层的邮箱筛选和审计原因入库都走它，保证空串不会被当成有效筛选值或落成空文本。
/// 与应用层同名助手职责相同，各自保留一份以免基础设施层反向依赖用例层。
pub(super) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 分页排序必须带唯一列 id，否则同一排序值的行会在页间重复或丢失。
pub(super) const MARGIN_PRODUCT_ORDER_BY: &str = " ORDER BY products.id DESC";

/// 行查询与 COUNT 查询必须由同一组过滤谓词构建，返回总数才能与当前筛选一致。
/// 行查询与 COUNT 查询复用同一筛选构建器；任一失败整体返回，避免列表与总数口径分裂。
///
/// 调用方负责传入已追加完筛选条件的两个 builder，这里只补排序、LIMIT 和 OFFSET 并依次执行。
/// 排序与分页只加在行查询上，COUNT 侧保持无序无分页，因此总数始终是全量匹配数而非当页条数。
/// 两次查询各自独立发往连接池、不在同一事务内，并发写入下总数与明细可能有极短暂的偏差。
/// 泛型行类型只要求可从 MySQL 行反序列化，产品、仓位和利息汇总三种读模型因此能共用本函数。
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

/// 向后台查询追加按用户邮箱过滤的 EXISTS 子查询，邮箱值以参数化方式绑定不做字符串插值。
/// 用 EXISTS 而非 JOIN，是为了不改变主查询的行数语义，避免用户表意外重复导致仓位被多计。
/// `user_id_column` 只接受 `'static` 字面量，由本仓库代码给定列名，不来自请求输入。
/// 邮箱先做空白折叠，传空串等同于不加条件；行查询与 COUNT 各调用一次即可保持筛选一致。
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

/// 生成标度固定为十八位的零，与资金列 `DECIMAL(38,18)` 精度一致，避免入库时被隐式补零。
/// 开仓插入仓位时用它初始化 `interest_amount`，让该列从建仓起就非空，利息 worker 可直接累加。
/// 纯函数不读写账户或流水，也不代表任何余额变更。
pub(super) fn zero_amount() -> BigDecimal {
    BigDecimal::from(0).with_scale(18)
}
