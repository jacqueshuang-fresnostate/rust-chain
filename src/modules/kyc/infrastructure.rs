//! kyc bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 本文件负责 `kyc_configs` 与 `user_kyc_submissions` 两张表的读写，
//! 并在读取申请时左连 `users` 补齐邮箱与手机号，同时按审核结论回写用户主表的 KYC 等级。
//! 配置以固定名称 `default` 作为单例存储，读取路径会先幂等补齐默认行，
//! 因此配置类查询即便签名看起来只读也可能产生写入。
//! 命名约定与 user 上下文一致：带 `_in_tx` 后缀的函数使用调用方事务且从不自行提交。
//! 加锁约定：配置更新与申请审核都先 `FOR UPDATE` 锁行再判断状态，
//! 保证「读出当前状态」与「写入新状态」之间不会被并发插入；
//! 但每用户唯一待审申请没有对应的数据库唯一约束，仅靠待审行锁与事务隔离级别兜底。
//! 隐私边界特别提醒：本层返回的申请对象与摘要都携带未脱敏的证件号原文、姓名与联系方式，
//! 完整对象还含证件图片内容，这些都不是可直接对外的脱敏 DTO。
//! 掩码只发生在 service 层构造审计 JSON 时，读取路径不做任何脱敏，
//! 调用方必须自行限制受众为本人或已授权的审核管理员，并避免写入日志。

use crate::{
    error::{AppError, AppResult},
    modules::kyc::domain::KycCountryDocumentTypeRule,
    modules::kyc::presentation::{
        KycConfigResponse, KycSubmissionResponse, KycSubmissionSummary, ListKycSubmissionsFilter,
    },
};
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool, QueryBuilder, Transaction, types::Json as SqlxJson};

const DEFAULT_CONFIG_NAME: &str = "default";

#[derive(Debug, sqlx::FromRow)]
struct KycConfigRow {
    id: u64,
    name: String,
    enabled: bool,
    target_kyc_level: i32,
    required_documents_json: SqlxJson<Vec<String>>,
    allowed_countries_json: SqlxJson<Vec<String>>,
    country_document_types_json: SqlxJson<Vec<KycCountryDocumentTypeRule>>,
    max_document_size_bytes: u64,
    updated_by: Option<u64>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct KycSubmissionRow {
    id: u64,
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    real_name: String,
    country: String,
    id_number: String,
    submission_type: String,
    enterprise_name: Option<String>,
    business_registration_number: Option<String>,
    document_type: String,
    document_front_image: String,
    document_back_image: String,
    document_handheld_image: Option<String>,
    status: String,
    target_kyc_level: i32,
    reviewed_by: Option<u64>,
    review_reason: Option<String>,
    submitted_at: DateTime<Utc>,
    reviewed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct KycSubmissionSummaryRow {
    id: u64,
    user_id: u64,
    email: Option<String>,
    phone: Option<String>,
    real_name: String,
    country: String,
    id_number: String,
    submission_type: String,
    enterprise_name: Option<String>,
    business_registration_number: Option<String>,
    document_type: String,
    status: String,
    target_kyc_level: i32,
    reviewed_by: Option<u64>,
    review_reason: Option<String>,
    submitted_at: DateTime<Utc>,
    reviewed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub(crate) struct UserKycStateRecord {
    pub(crate) status: String,
    pub(crate) kyc_level: i32,
}

/// 读取单例 KYC 配置，读之前先幂等补齐默认行，因此首次调用会真实写入一条配置。
/// 这一「先补后读」的设计让上层无需区分「系统刚部署尚未配置」与「已配置」，任何时候都能拿到可用规则。
/// 补写与读取是两条独立语句且不在事务内，理论上存在补写成功后被并发删除导致读不到的窗口，
/// 此时返回 `AppError::NotFound`。
/// 不加锁，适用于展示与提交校验这类只需快照的场景；需要改配置的路径应改用锁定版本。
pub(crate) async fn load_kyc_config(pool: &Pool<MySql>) -> AppResult<KycConfigResponse> {
    ensure_default_config(pool).await?;
    let row = sqlx::query_as::<_, KycConfigRow>(&select_kyc_config_sql(false))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(config_response(row))
}

/// 在调用方事务内读取 KYC 配置，同样先幂等补齐默认行，但补写与读取共用该事务。
/// 相对连接池版本的收益正在于此：补写与读取原子，不存在补完被并发删除而读空的窗口。
/// 主要用途是配置保存流程的最后一步，回读刚写入的值作为审计的「改后快照」；
/// 因为在同一事务内，能读到本事务尚未提交的修改。
/// 不加 `FOR UPDATE`，取快照即可，行锁由更早的锁定步骤持有；不自行提交。
pub(crate) async fn load_kyc_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<KycConfigResponse> {
    ensure_default_config_in_tx(tx).await?;
    let row = sqlx::query_as::<_, KycConfigRow>(&select_kyc_config_sql(false))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(config_response(row))
}

/// 锁定单例 KYC 配置行并返回改前快照，是配置更新流程的第一道串行化关卡。
/// 行锁一直持有到调用方事务结束，因此从读出旧值、校验新值到写入的整个区间内，
/// 其他管理员无法并发修改同一行，返回的前后快照必然首尾相接而不会错配。
/// 与另外两个读取函数的关键差异是这里不补默认行：调用方必须先确保配置存在，
/// 否则会因无行可锁而返回 `AppError::NotFound`。
pub(crate) async fn lock_kyc_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
) -> AppResult<KycConfigResponse> {
    let row = sqlx::query_as::<_, KycConfigRow>(&select_kyc_config_sql(true))
        .bind(DEFAULT_CONFIG_NAME)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(config_response(row))
}

#[allow(clippy::too_many_arguments)] // KYC 配置字段按单行原子写入，保持参数与持久化列可审计对应。
/// 写入 KYC 配置：按固定名称 upsert，不存在则建行，已存在则整行覆盖业务列。
/// 三个清单类字段以 JSON 列存储，写入时保留领域校验后的既定顺序与去重结果，本层不再重排。
/// 冲突分支逐列显式列出而非依赖默认行为，使参数与持久化列一一对应，便于审计核对哪些列会被改写。
/// `updated_by` 每次覆盖为本次操作的管理员 ID，只作追溯，本层不校验其权限。
/// 参数较多是有意为之：逐个传值而非传结构体，可确保新增配置项时编译器强制在此补齐绑定。
/// 前置条件是调用方已锁定配置行并完成领域校验；本函数不校验取值合法性，也不自行提交。
pub(crate) async fn upsert_kyc_config_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    enabled: bool,
    target_kyc_level: i32,
    required_documents: Vec<String>,
    allowed_countries: Vec<String>,
    country_document_types: Vec<KycCountryDocumentTypeRule>,
    max_document_size_bytes: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO kyc_configs
           (name, enabled, target_kyc_level, required_documents_json, allowed_countries_json, country_document_types_json, max_document_size_bytes, updated_by)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE enabled = VALUES(enabled),
                                   target_kyc_level = VALUES(target_kyc_level),
                                   required_documents_json = VALUES(required_documents_json),
                                   allowed_countries_json = VALUES(allowed_countries_json),
                                   country_document_types_json = VALUES(country_document_types_json),
                                   max_document_size_bytes = VALUES(max_document_size_bytes),
                                   updated_by = VALUES(updated_by)"#,
    )
    .bind(DEFAULT_CONFIG_NAME)
    .bind(enabled)
    .bind(target_kyc_level)
    .bind(SqlxJson(required_documents))
    .bind(SqlxJson(allowed_countries))
    .bind(SqlxJson(country_document_types))
    .bind(max_document_size_bytes)
    .bind(admin_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 按用户和递减主键读取最新 KYC 申请且不加行锁。
/// 完整响应含未掩码身份号、联系方式与材料地址，调用方须执行本人/管理员授权并避免日志扩散。
pub(crate) async fn latest_kyc_submission(
    pool: &Pool<MySql>,
    user_id: u64,
) -> AppResult<Option<KycSubmissionResponse>> {
    let row = sqlx::query_as::<_, KycSubmissionRow>(&format!(
        "{} WHERE submissions.user_id = ? ORDER BY submissions.submitted_at DESC, submissions.id DESC LIMIT 1",
        select_kyc_submission_sql()
    ))
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(submission_response))
}

/// 管理端分页检索 KYC 申请，返回当页摘要与匹配总数。
/// 用 `QueryBuilder` 把同一组谓词分别压入行查询与计数查询，二者共用一次构造循环，
/// 从结构上避免筛选条件只改了一边而导致总数与列表口径不符。
/// 三个过滤维度可任意组合，均为可选：用户 ID、精确邮箱、申请状态；`WHERE 1 = 1` 只是拼接起点。
/// 分页参数经硬性夹取：每页条数收敛到 1 到 100，偏移量上限十万，
/// 防止调用方传入极端值造成全表扫描或深分页拖垮数据库。
/// 排序按提交时间倒序、主键倒序，主键作次级键保证同一时刻提交的记录顺序稳定，翻页不会重复或漏项。
/// 两个查询各自独立执行且不在事务内，因此并发写入时列表与总数可能来自不同快照，
/// 总数仅供分页控件参考，不应作为精确统计。
/// 隐私提醒：摘要不含证件图片内容，但证件号是未脱敏原文，姓名、邮箱、手机号也原样返回，
/// 掩码只在写审计时另行施加，本查询结果只能提供给已授权的审核管理员。
pub(crate) async fn list_kyc_submissions(
    pool: &Pool<MySql>,
    filter: ListKycSubmissionsFilter,
) -> AppResult<(Vec<KycSubmissionSummary>, i64)> {
    let mut rows_query = QueryBuilder::<MySql>::new(select_kyc_submission_summary_sql());
    let mut total_query = QueryBuilder::<MySql>::new(count_kyc_submission_sql());
    // 计数与行查询压入同一组谓词，总数不会脱离当前筛选。
    for builder in [&mut rows_query, &mut total_query] {
        builder.push(" WHERE 1 = 1");
        if let Some(user_id) = filter.user_id {
            builder.push(" AND submissions.user_id = ");
            builder.push_bind(user_id);
        }
        if let Some(email) = filter.email.as_ref() {
            builder.push(" AND users.email = ");
            builder.push_bind(email);
        }
        if let Some(status) = filter.status.as_ref() {
            builder.push(" AND submissions.status = ");
            builder.push_bind(status);
        }
    }
    rows_query.push(" ORDER BY submissions.submitted_at DESC, submissions.id DESC LIMIT ");
    rows_query.push_bind(i64::from(filter.limit.clamp(1, 100)));
    rows_query.push(" OFFSET ");
    rows_query.push_bind(i64::from(filter.offset.min(100_000)));

    let rows = rows_query
        .build_query_as::<KycSubmissionSummaryRow>()
        .fetch_all(pool)
        .await?;
    let (total,): (i64,) = total_query.build_query_as().fetch_one(pool).await?;
    Ok((rows.into_iter().map(submission_summary).collect(), total))
}

/// 按主键读取 KYC 申请及用户联络信息，未命中返回未找到。
/// 结果保留原始身份号和材料地址，不是脱敏 DTO，必须由上层限制到本人或受权管理员。
pub(crate) async fn load_kyc_submission(
    pool: &Pool<MySql>,
    submission_id: u64,
) -> AppResult<KycSubmissionResponse> {
    let row = sqlx::query_as::<_, KycSubmissionRow>(&format!(
        "{} WHERE submissions.id = ? LIMIT 1",
        select_kyc_submission_sql()
    ))
    .bind(submission_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(submission_response(row))
}

/// 在调用方事务内按主键回读申请完整快照，主要用于审核写入后取「改后状态」供审计记录。
/// 因为在同一事务内执行，能看到本事务刚写入但尚未提交的状态变更，无需等提交后再查一次。
/// 与锁定版本的区别是这里不加 `FOR UPDATE`：行锁应在审核流程更早的判定阶段获取，
/// 此处只取值，重复加锁没有额外收益。
/// 记录不存在返回 `AppError::NotFound`；结果含证件号与图片原文，不自行提交事务。
pub(crate) async fn load_kyc_submission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    submission_id: u64,
) -> AppResult<KycSubmissionResponse> {
    let row = sqlx::query_as::<_, KycSubmissionRow>(&format!(
        "{} WHERE submissions.id = ? LIMIT 1",
        select_kyc_submission_sql()
    ))
    .bind(submission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(submission_response(row))
}

/// 锁定指定申请行并返回其当前快照，是审核动作的串行化入口。
/// 行锁让「确认这笔申请仍处于待审」与「写入审核结论」成为不可分割的序列，
/// 从而杜绝两个管理员同时审同一笔申请、后者覆盖前者结论的情况。
/// 返回的是完整快照而非仅状态字段，调用方可据此校验状态机迁移是否合法并取得改前值用于审计。
/// 记录不存在返回 `AppError::NotFound`；锁持有至调用方事务结束。
pub(crate) async fn lock_kyc_submission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    submission_id: u64,
) -> AppResult<KycSubmissionResponse> {
    let row = sqlx::query_as::<_, KycSubmissionRow>(&format!(
        "{} WHERE submissions.id = ? LIMIT 1 FOR UPDATE",
        select_kyc_submission_sql()
    ))
    .bind(submission_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(submission_response(row))
}

/// 锁定用户主表行并取出账号状态与当前 KYC 等级，供提交申请前的资格判定。
/// 两个字段各有用途：状态用于确认账号处于启用态，等级用于判断用户是否已达或超过本次目标等级，
/// 已达标者无需重复提交申请。
/// 行锁把这两项判定与后续的申请插入绑成一个原子单元，避免判定通过后等级被并发抬升而产生冗余申请。
/// 用户不存在返回 `AppError::Unauthorized` 而非 `NotFound`，与用户上下文的口径保持一致，
/// 不让错误类型泄露某个 ID 是否注册过。
/// 本函数只取值不判定，具体规则由 application 层施加；判定失败时调用方须回滚整个事务。
pub(crate) async fn lock_user_kyc_state_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<UserKycStateRecord> {
    sqlx::query_as::<_, (String, i32)>(
        r#"SELECT status, kyc_level
           FROM users
           WHERE id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(|(status, kyc_level)| UserKycStateRecord { status, kyc_level })
    .ok_or(AppError::Unauthorized)
}

/// 锁定用户最新待审申请主键，作为并发提交前的重复申请边界。
/// 未命中返回 `None`；命中行保持到事务结束。无命中时是否形成范围锁取决于索引和隔离级别，
/// 本函数不创建申请，也不单独提供“每用户唯一待审”的数据库约束。
pub(crate) async fn lock_pending_kyc_submission_id_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<Option<u64>> {
    sqlx::query_scalar(
        r#"SELECT id
           FROM user_kyc_submissions
           WHERE user_id = ? AND status = 'pending'
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}

#[allow(clippy::too_many_arguments)] // 身份材料字段需显式绑定 SQL 列，避免结构化调试输出泄露敏感数据。
/// 插入一笔新的实名认证申请，状态在 SQL 中硬编码为 `pending`，不接受调用方指定。
/// 这样从数据库层就保证任何新申请都从待审开始，无法伪造一笔直接为通过状态的记录。
/// `target_kyc_level` 在插入时固化为当时配置的目标等级，因此后续运营调高目标等级不会影响在途申请，
/// 审核通过时抬升的仍是提交那一刻承诺的等级。
/// 敏感字段逐个绑定为 SQL 参数而非拼接字符串，既杜绝注入，也避免把结构体整体格式化输出而泄露证件内容。
/// 企业名称、注册号与手持照三项可为空，分别对应个人申请与不要求手持照的国家证件组合。
/// 返回自增主键供调用方回读或写审计；不校验是否已有待审申请，防重由调用方先行锁定待审行完成。
/// 不自行提交，失败时由调用方回滚，不会留下半成品记录。
pub(crate) async fn insert_user_kyc_submission_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    real_name: &str,
    country: &str,
    id_number: &str,
    submission_type: &str,
    enterprise_name: Option<&str>,
    business_registration_number: Option<&str>,
    document_type: &str,
    document_front_image: &str,
    document_back_image: &str,
    document_handheld_image: &Option<String>,
    target_kyc_level: i32,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO user_kyc_submissions
           (user_id, real_name, country, id_number, submission_type, enterprise_name, business_registration_number, document_type, document_front_image, document_back_image, document_handheld_image, status, target_kyc_level)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)"#,
    )
    .bind(user_id)
    .bind(real_name)
    .bind(country)
    .bind(id_number)
    .bind(submission_type)
    .bind(enterprise_name)
    .bind(business_registration_number)
    .bind(document_type)
    .bind(document_front_image)
    .bind(document_back_image)
    .bind(document_handheld_image)
    .bind(target_kyc_level)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_id())
}

/// 把审核结论写入申请记录，一次性落下状态、审核人、审核理由与审核时间四项。
/// 四者必须同时写入：只改状态而缺审核人或理由会让这笔终态记录失去可追溯性。
/// 审核时间取数据库当前时间，避免多个应用节点时钟不一致导致审核先后顺序错乱。
/// WHERE 只按主键匹配而不带状态条件，因此本语句本身不阻止把已审结的申请再改一次；
/// 「只有待审申请可被审核」这条状态机约束由调用方在锁行后判定，本层不重复施加。
/// 传入状态应已通过领域校验，只能是通过或驳回，`pending` 不是合法的审核结论。
/// 不自行提交：审核通过时还需同事务抬升用户 KYC 等级，两者必须一起成功或一起回滚。
pub(crate) async fn update_kyc_submission_review_in_tx(
    tx: &mut Transaction<'_, MySql>,
    submission_id: u64,
    admin_id: u64,
    status: &str,
    reason: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE user_kyc_submissions
           SET status = ?, reviewed_by = ?, review_reason = ?, reviewed_at = CURRENT_TIMESTAMP(6)
           WHERE id = ?"#,
    )
    .bind(status)
    .bind(admin_id)
    .bind(reason)
    .bind(submission_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在审核通过时抬升用户主表的 KYC 等级，用 `GREATEST` 取现值与目标值中的较大者。
/// 这个取大而非直接赋值的写法是一道防降级保险：
/// 若用户已通过更高等级的认证，一笔目标等级较低的旧申请获批也不会把等级拉低，
/// 因此本函数天然幂等，重复执行不会改变结果。
/// 只在通过分支调用，驳回不应触碰用户等级。
/// 不检查受影响行数，用户不存在时同样返回成功且实际未改动任何数据；
/// 用户存在性由审核流程更早的锁定步骤保证。
/// 必须与审核结论写入处于同一事务，否则可能出现等级已升而申请仍显示待审。
pub(crate) async fn update_user_kyc_level_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    level: i32,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE users
           SET kyc_level = GREATEST(kyc_level, ?)
           WHERE id = ?"#,
    )
    .bind(level)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 幂等补齐名为 `default` 的 KYC 配置行，使系统在从未配置过时也有一份可用规则。
/// 幂等性依靠名称唯一键加上冲突分支的空更新实现：行已存在时只把名称赋值给自己，
/// 任何业务列都不会被这次调用改写，因此对已有运营配置绝对安全。
/// 以连接池自治执行单条语句，不参与任何事务；需要与后续读写原子的场景应改用事务版本。
pub(crate) async fn ensure_default_config(pool: &Pool<MySql>) -> AppResult<()> {
    sqlx::query(default_config_insert_sql())
        .execute(pool)
        .await?;
    Ok(())
}

/// 在调用方事务内幂等补齐默认 KYC 配置，与连接池版本共用同一条 SQL，语义完全一致。
/// 使用事务版本的收益是补写与随后的锁行或读取原子完成，
/// 配置保存流程正是靠它保证首次保存时也一定有行可供 `FOR UPDATE` 锁定。
/// 同样不覆盖任何已有业务列，不自行提交事务。
pub(crate) async fn ensure_default_config_in_tx(tx: &mut Transaction<'_, MySql>) -> AppResult<()> {
    sqlx::query(default_config_insert_sql())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 返回补齐默认 KYC 配置的 SQL，两个 `ensure_default_config` 变体共用同一份语句常量。
/// 默认口径写死在此：认证功能开启、目标等级为 1、必填材料为证件正反面、
/// 允许国家与按国家证件规则均为空数组（即不限制），单份材料上限五兆。
/// 冲突分支 `name = name` 是刻意的空更新，MySQL 借此在行已存在时不改动任何业务列，
/// 从而让插入语句退化为无副作用操作。
fn default_config_insert_sql() -> &'static str {
    r#"INSERT INTO kyc_configs
       (name, enabled, target_kyc_level, required_documents_json, allowed_countries_json, country_document_types_json, max_document_size_bytes)
       VALUES ('default', TRUE, 1, JSON_ARRAY('identity_front', 'identity_back'), JSON_ARRAY(), JSON_ARRAY(), 5242880)
       ON DUPLICATE KEY UPDATE name = name"#
}

/// 拼装读取 KYC 配置的 SQL，`for_update` 决定是否追加 `FOR UPDATE` 加锁子句。
/// 三个读取入口共用这一处列清单，避免加锁版与非加锁版在字段上出现偏差。
/// 配置名以占位符绑定而非拼入字符串，拼接部分只有固定的锁子句，不存在注入面。
/// 返回 `String` 而非静态串正是因为锁子句需要运行时决定。
fn select_kyc_config_sql(for_update: bool) -> String {
    let mut sql = String::from(
        r#"SELECT id, name, enabled, target_kyc_level, required_documents_json,
                  allowed_countries_json, country_document_types_json, max_document_size_bytes, updated_by, created_at, updated_at
           FROM kyc_configs
           WHERE name = ?"#,
    );
    if for_update {
        sql.push_str(" FOR UPDATE");
    }
    sql
}

/// 返回申请总数查询的 SQL 前缀，供分页检索统计匹配条数。
/// 表别名与连接方式必须与行查询完全一致：同样左连 `users` 且按主键关联。
/// 保持一致有两个原因——过滤条件里可能引用 `users.email`，缺了连接会直接报错；
/// 而左连按主键不会放大基数，因此计数结果与行查询口径严格对应。
/// 只返回不含 WHERE 的前缀，谓词由调用方与行查询共用同一段逻辑压入。
fn count_kyc_submission_sql() -> &'static str {
    // JOIN 与行查询保持一致：users 按主键连接，不改变基数。
    r#"SELECT COUNT(*)
       FROM user_kyc_submissions submissions
       LEFT JOIN users ON users.id = submissions.user_id"#
}

/// 返回申请摘要列表查询的 SQL 前缀，用于管理端分页浏览。
/// 与完整查询的关键差异是列清单刻意排除三个证件图片字段与 `created_at`：
/// 图片是 Base64 长文本，逐行带出会让列表响应急剧膨胀，而列表场景并不需要看图。
/// 其余字段保持一致，包括未脱敏的证件号，因此摘要同样属于敏感数据。
/// 左连 `users` 补齐邮箱与手机号，用左连而非内连以确保用户记录异常缺失时申请仍可被检索到。
fn select_kyc_submission_summary_sql() -> &'static str {
    r#"SELECT submissions.id, submissions.user_id, users.email, users.phone,
              submissions.real_name, submissions.country, submissions.id_number, submissions.submission_type,
              submissions.enterprise_name, submissions.business_registration_number,
              submissions.document_type, submissions.status, submissions.target_kyc_level,
              submissions.reviewed_by, submissions.review_reason, submissions.submitted_at,
              submissions.reviewed_at, submissions.updated_at
       FROM user_kyc_submissions submissions
       LEFT JOIN users ON users.id = submissions.user_id"#
}

/// 返回单笔申请完整查询的 SQL 前缀，被最新申请、按主键读取、事务内回读与锁定四处复用。
/// 相对摘要版本额外带出三张证件图片与 `created_at`，因此结果是全量而非列表投影。
/// 四个调用点各自追加不同的 WHERE、排序与锁子句，共用前缀确保它们的字段口径不会漂移。
/// 同样左连 `users` 补齐联系方式。
/// 输出含证件图片原文与未脱敏证件号，是本模块敏感度最高的查询，受众必须严格限定。
fn select_kyc_submission_sql() -> &'static str {
    r#"SELECT submissions.id, submissions.user_id, users.email, users.phone,
              submissions.real_name, submissions.country, submissions.id_number, submissions.submission_type,
              submissions.enterprise_name, submissions.business_registration_number,
              submissions.document_type, submissions.document_front_image, submissions.document_back_image,
              submissions.document_handheld_image,
              submissions.status, submissions.target_kyc_level, submissions.reviewed_by,
              submissions.review_reason, submissions.submitted_at, submissions.reviewed_at,
              submissions.created_at, submissions.updated_at
       FROM user_kyc_submissions submissions
       LEFT JOIN users ON users.id = submissions.user_id"#
}

/// 把配置数据库行转换为对外响应对象，逐字段平移不做业务判断。
/// 唯一的形态调整是把三个清单字段从 SQLx 的 JSON 包装解开为裸向量，
/// 使响应结构不必依赖 SQLx 类型，也让上层无需感知这些字段在库中以 JSON 列存储。
/// 配置不含个人数据，因此无需脱敏。
fn config_response(row: KycConfigRow) -> KycConfigResponse {
    KycConfigResponse {
        id: row.id,
        name: row.name,
        enabled: row.enabled,
        target_kyc_level: row.target_kyc_level,
        required_documents: row.required_documents_json.0,
        allowed_countries: row.allowed_countries_json.0,
        country_document_types: row.country_document_types_json.0,
        max_document_size_bytes: row.max_document_size_bytes,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

/// 把申请完整行转换为响应对象，全部字段原样搬运。
/// 明确不做的事：不掩码证件号、不裁剪证件图片、不按调用者身份筛选字段。
/// 这是一次纯粹的类型转换而非脱敏边界，输出与数据库中的原文完全一致，
/// 受众限制必须由更上层根据请求者是本人还是审核管理员来施加。
fn submission_response(row: KycSubmissionRow) -> KycSubmissionResponse {
    KycSubmissionResponse {
        id: row.id,
        user_id: row.user_id,
        email: row.email,
        phone: row.phone,
        real_name: row.real_name,
        country: row.country,
        id_number: row.id_number,
        submission_type: row.submission_type,
        enterprise_name: row.enterprise_name,
        business_registration_number: row.business_registration_number,
        document_type: row.document_type,
        document_front_image: row.document_front_image,
        document_back_image: row.document_back_image,
        document_handheld_image: row.document_handheld_image,
        status: row.status,
        target_kyc_level: row.target_kyc_level,
        reviewed_by: row.reviewed_by,
        review_reason: row.review_reason,
        submitted_at: row.submitted_at,
        reviewed_at: row.reviewed_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

/// 把申请摘要行转换为列表项对象，与完整版本同为原样搬运。
/// 二者的字段差异完全来自 SQL 的列清单而非这里的取舍：摘要行本就不含证件图片与创建时间，
/// 所以本函数没有任何丢弃字段的逻辑。
/// 同样不掩码证件号，摘要列表仍属敏感数据，只可提供给已授权的审核管理员。
fn submission_summary(row: KycSubmissionSummaryRow) -> KycSubmissionSummary {
    KycSubmissionSummary {
        id: row.id,
        user_id: row.user_id,
        email: row.email,
        phone: row.phone,
        real_name: row.real_name,
        country: row.country,
        id_number: row.id_number,
        submission_type: row.submission_type,
        enterprise_name: row.enterprise_name,
        business_registration_number: row.business_registration_number,
        document_type: row.document_type,
        status: row.status,
        target_kyc_level: row.target_kyc_level,
        reviewed_by: row.reviewed_by,
        review_reason: row.review_reason,
        submitted_at: row.submitted_at,
        reviewed_at: row.reviewed_at,
        updated_at: row.updated_at,
    }
}
