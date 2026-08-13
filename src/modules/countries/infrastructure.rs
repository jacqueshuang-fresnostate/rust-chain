//! countries bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 国家配置对外只有一条读路径，即取出开放注册的国家清单，过滤与排序都写死在 SQL 中。
//! 支持语言在库中以 JSON 数组列存放，读取时由 sqlx 直接解码为字符串数组。

use crate::{
    architecture::InfrastructureLayer, error::AppResult, modules::countries::domain::PublicCountry,
};
use sqlx::{MySql, Pool, types::Json as SqlxJson};

#[derive(Debug)]
pub struct CountryConfigRepository;

impl InfrastructureLayer for CountryConfigRepository {}

/// 国家配置表的查询行，仅取公开接口需要的四列，后台字段如排序值与开关不进入领域对象。
#[derive(Debug, sqlx::FromRow)]
struct PublicCountryRow {
    country_code: String,
    country_name: String,
    default_locale: String,
    supported_locales: SqlxJson<Vec<String>>,
}

impl From<PublicCountryRow> for PublicCountry {
    /// 把数据库行转成领域对象，其中支持语言列从 JSON 包装中取出内层字符串数组。
    /// 纯字段搬运，不再校验语言白名单也不补默认值，取值以写入时的校验结果为准。
    fn from(row: PublicCountryRow) -> Self {
        Self {
            country_code: row.country_code,
            country_name: row.country_name,
            default_locale: row.default_locale,
            supported_locales: row.supported_locales.0,
        }
    }
}

/// 按后台配置顺序读取启用且允许注册的国家记录。
/// 查询为只读操作，不开启事务；JSON 语言字段解码失败按数据库错误返回。
/// 两个过滤条件缺一不可：状态启用表示该国配置有效，注册开关表示当前接受新用户，
/// 只有同时满足才会出现在注册页的国家选择列表中。
/// 排序先按后台维护的排序值升序，再以国家代码兜底，保证同序号国家的相对次序稳定不随查询抖动。
pub async fn fetch_public_countries(pool: &Pool<MySql>) -> AppResult<Vec<PublicCountry>> {
    let rows = sqlx::query_as::<_, PublicCountryRow>(
        r#"SELECT country_code, country_name, default_locale, supported_locales
           FROM country_configs
           WHERE registration_enabled = TRUE AND status = 'active'
           ORDER BY sort_order ASC, country_code ASC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}
