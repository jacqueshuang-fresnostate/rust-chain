//! new_coin bounded context infrastructure layer.
//!
//! 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
//! 本文件提供新币发行上下文全部仓储 trait 的 MySQL 适配器，覆盖项目与订单只读查询、
//! 申购与上市后购买的下单事务、解禁手续费状态置位，以及到期锁仓的释放入账。
//! 所有资金动作都收敛在单个 MySQL 事务内，统一按「先锁项目与交易对配置、再锁钱包账户行、
//! 最后写锁仓与解禁记录」的方向取行锁，保证并发下单与后台改配置之间不会互相插队。
//! 金额一律由 `BigDecimal` 承载并按数据库列定义的 18 位小数存取，本层不额外舍入或截断。
//! 本层不发布任何领域事件，事件广播由 application 层在事务提交成功后自行触发。

use crate::{
    error::{AppError, AppResult},
    modules::new_coin::{
        LifecycleStatus,
        repository::{
            NewCoinDistributionRead, NewCoinLedgerMetadata, NewCoinLockPositionWrite,
            NewCoinOrderRepository, NewCoinPairRead, NewCoinProjectRead, NewCoinProjectRuleRead,
            NewCoinPurchaseOrderInsert, NewCoinPurchaseOrderInsertResult,
            NewCoinPurchaseOrderWrite, NewCoinPurchaseRead, NewCoinReadRepository,
            NewCoinRepositoryError, NewCoinSubscriptionOrderWrite, NewCoinSubscriptionRead,
            NewCoinUnlockFeeRepository, NewCoinUnlockRead, NewCoinUnlockReleaseRepository,
            NewCoinWalletRead, ReleaseUnlockOutcome, UnlockFeeExpectation, UnlockFeePaidStatus,
            UnlockFeePaymentUpdate, UnlockFeePaymentWrite,
        },
        service::{
            ensure_post_listing_purchase_enabled, lifecycle_status, lock_positions_for_project,
            unlock_fee_fields,
        },
    },
};
use axum::async_trait;
use bigdecimal::BigDecimal;
use chrono::Utc;
use sqlx::{MySql, Pool, QueryBuilder, Transaction};

impl From<sqlx::Error> for NewCoinRepositoryError {
    /// 把 SQLx 底层错误折叠为仓储层的 `Storage` 变体，只保留其字符串描述。
    /// 折叠后无法再区分连接断开、唯一键冲突和语法错误，需要按类别分支处理的调用点
    /// 必须在此转换之前自行匹配原始 `sqlx::Error`，不能依赖转换结果做判定。
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

/// 兼容既有公共导入的新币 MySQL 适配器，负责购买单幂等写入与解锁费状态持久化。
#[derive(Debug, Clone)]
pub struct MySqlNewCoinRepository {
    pool: Pool<MySql>,
}

impl MySqlNewCoinRepository {
    /// 绑定调用方已建好的 MySQL 连接池，构造时不获取连接、不发送查询、不校验表结构。
    /// 连接池本身是引用计数句柄，克隆本适配器不会新增物理连接，超时与最大连接数沿用外部配置。
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }

    /// 借出内部连接池引用，供旧集成代码在本适配器之外自行拼装查询或开启事务。
    /// 直接使用连接池会绕过本适配器的幂等判定与状态守卫，调用方须自行保证
    /// 涉及资金的写入仍走既有方法，避免出现无账本的余额变动。
    pub fn pool(&self) -> &Pool<MySql> {
        &self.pool
    }

    /// 按幂等键向 `new_coin_purchase_orders` 单表写入一条购买单，价格、数量、
    /// 计价金额与锁仓编号全部取自入参快照，不做任何重算或补全。
    /// 依靠 `ON DUPLICATE KEY UPDATE idempotency_key = idempotency_key` 实现幂等：
    /// 键已存在时不改写任何列，`last_insert_id` 返回 0，随后回查既有订单编号并把
    /// `inserted` 置为 false，因此重复调用不会产生第二条记录。
    /// 此兼容入口不开启事务、不扣减钱包、不创建锁仓也不写资金流水，仅登记订单行。
    /// 冲突以外的 SQL 错误统一折叠为 `Storage`，失败时这条 INSERT 自身不留部分写入。
    pub async fn insert_purchase_order(
        &self,
        order: NewCoinPurchaseOrderInsert,
    ) -> Result<NewCoinPurchaseOrderInsertResult, NewCoinRepositoryError> {
        let insert_result = sqlx::query(
            r#"INSERT INTO new_coin_purchase_orders
               (project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                quote_amount, lock_position_id, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE idempotency_key = idempotency_key"#,
        )
        .bind(order.project_id)
        .bind(order.user_id)
        .bind(order.pair_id)
        .bind(order.base_asset_id)
        .bind(order.quote_asset_id)
        .bind(order.price)
        .bind(order.quantity)
        .bind(order.quote_amount)
        .bind(order.lock_position_id)
        .bind(order.status)
        .bind(&order.idempotency_key)
        .execute(&self.pool)
        .await?;

        let order_id = insert_result.last_insert_id();
        Ok(NewCoinPurchaseOrderInsertResult {
            order_id: if order_id == 0 {
                self.purchase_order_id(&order.idempotency_key).await?
            } else {
                order_id
            },
            inserted: order_id != 0,
        })
    }

    /// 以解禁幂等键加 `user_id` 双条件回读 `asset_unlock_records.fee_paid_status`，
    /// 用户维度写进 `WHERE` 而非事后过滤，避免凭键越权读到他人的解禁记录。
    /// 记录不存在返回 `None`；存储值只接受 not_required、pending、paid 三种，
    /// 其余一律判为脏数据并返回 `InvalidStatus` 且带上原始字符串。
    /// 该查询走连接池自动提交且不加行锁，返回后可能立刻被并发缴费改写，
    /// 因此不能作为放行解禁释放的唯一依据。
    pub async fn unlock_fee_paid_status(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> Result<Option<UnlockFeePaidStatus>, NewCoinRepositoryError> {
        let row = sqlx::query_as::<_, (String,)>(
            r#"SELECT fee_paid_status
               FROM asset_unlock_records
               WHERE idempotency_key = ? AND user_id = ?
               LIMIT 1"#,
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|(status,)| unlock_fee_paid_status_from_storage(&status))
            .transpose()
    }

    /// 按解禁幂等键与 `user_id` 把 `fee_paid_status` 由任意非 paid 值改写为 paid，
    /// 同时用入参覆盖记录上的费用资产与费用金额两列。
    /// `WHERE` 自带 `fee_paid_status <> 'paid'` 守卫，把状态判定与更新压在一条语句里，
    /// 因此重复调用只有首次影响一行返回 `true`，之后恒为 `false`，可安全重放。
    /// 此兼容入口不校验金额是否与项目费率一致、不扣减钱包、不写 `wallet_ledger`，
    /// 是纯粹的状态置位；调用方须先完成金额与支付资产比对，
    /// 否则会把错误的收费口径永久写入解禁记录。
    pub async fn mark_unlock_fee_paid(
        &self,
        payment: UnlockFeePaymentUpdate,
    ) -> Result<bool, NewCoinRepositoryError> {
        let result = sqlx::query(
            r#"UPDATE asset_unlock_records
               SET fee_paid_status = 'paid',
                   unlock_fee_asset = ?,
                   unlock_fee_amount = ?
               WHERE idempotency_key = ?
                 AND user_id = ?
                 AND fee_paid_status <> 'paid'"#,
        )
        .bind(payment.payment_asset_id)
        .bind(payment.amount)
        .bind(payment.unlock_idempotency_key)
        .bind(payment.user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    /// 在幂等插入命中重复键、`last_insert_id` 退化为 0 时，按幂等键回读既有购买单主键。
    /// 仅供 `insert_purchase_order` 内部收敛返回值，不对外暴露也不做用户维度过滤。
    /// 若此刻查不到行，说明该键对应的订单已被并发删除，`RowNotFound` 会折叠为
    /// `Storage` 错误向上抛出，而不是伪造一个零编号。
    async fn purchase_order_id(
        &self,
        idempotency_key: &str,
    ) -> Result<u64, NewCoinRepositoryError> {
        let row = sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM new_coin_purchase_orders WHERE idempotency_key = ? LIMIT 1",
        )
        .bind(idempotency_key)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }
}

/// 把数据库中的费用状态字符串映射为枚举，是存储表示与领域表示之间唯一的解析入口。
/// not_required 表示项目未开启解禁收费，pending 表示应收未付，paid 表示已完成缴费。
/// 枚举外的取值不做兜底降级，直接返回 `InvalidStatus` 并回带原始字符串，
/// 让脏数据在读取阶段就暴露，而不是被静默当成未收费放行。
fn unlock_fee_paid_status_from_storage(
    value: &str,
) -> Result<UnlockFeePaidStatus, NewCoinRepositoryError> {
    match value {
        "not_required" => Ok(UnlockFeePaidStatus::NotRequired),
        "pending" => Ok(UnlockFeePaidStatus::Pending),
        "paid" => Ok(UnlockFeePaidStatus::Paid),
        _ => Err(NewCoinRepositoryError::InvalidStatus(value.to_owned())),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MySqlNewCoinReadRepository {
    pool: Pool<MySql>,
}

impl MySqlNewCoinReadRepository {
    /// 保存 MySQL 连接池，构造出同时实现只读查询、下单、解禁费与释放四组仓储 trait 的统一适配器。
    /// 构造过程不获取连接、不发送查询、不校验 schema；池句柄可廉价克隆，
    /// 四组 trait 方法共用同一数据库边界，因此跨 trait 的调用可以落在同一事务里。
    pub(crate) fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NewCoinReadRepository for MySqlNewCoinReadRepository {
    /// 读取 `status = 'active'` 的公开新币项目，按主键倒序返回最新上架的若干条并受 `limit` 截断。
    /// 单行同时带出生命周期状态、发行价、上市时间、解禁类型与解禁费配置，
    /// 以及上市后购买开关和后台指定的唯一交易对，供项目列表页一次渲染完成。
    /// 查询不带用户维度条件，返回的是面向所有人的公告数据；
    /// 被后台停用的项目在此不可见，也不会回退去读草稿态记录。
    async fn list_active_projects(&self, limit: u32) -> AppResult<Vec<NewCoinProjectRead>> {
        let rows = sqlx::query_as::<_, NewCoinProjectReadRow>(
            r#"SELECT id, asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
                      unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
                      unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                      post_listing_purchase_enabled, post_listing_pair_id, status
               FROM new_coin_projects
               WHERE status = 'active'
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// 按项目符号精确匹配单个启用项目，取的列与列表查询完全一致，让详情页与列表页共用同一套映射。
    /// 符号未命中、项目已被后台停用或尚未创建，三种情况统一返回 `None`，
    /// 由上层决定渲染空态还是抛出 404，本层不区分也不额外报错。
    /// 符号在启用项目中视为唯一，SQL 仍加 `LIMIT 1` 兜底，
    /// 万一存在历史脏数据也只取主键顺序上的首行而不是报错。
    async fn find_active_project_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRead>> {
        let row = sqlx::query_as::<_, NewCoinProjectReadRow>(
            r#"SELECT id, asset_id, symbol, lifecycle_status, total_supply, issue_price, listed_at,
                      unlock_type, fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
                      unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                      post_listing_purchase_enabled, post_listing_pair_id, status
               FROM new_coin_projects
               WHERE symbol = ? AND status = 'active'
               LIMIT 1"#,
        )
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// 读取指定用户的新币申购单，`user_id` 直接进入 `WHERE` 实现租户隔离，调用方无需再次过滤。
    /// 每行带出申购时冻结的计价资产、已支付金额、申请数量与最终配额数量，
    /// 可据此直接展示「申请多少、实际中签多少」而不必回表补算。
    /// 结果按主键倒序并受 `limit` 截断，是纯只读快照，
    /// 不会触发配额重算，也不会把 pending 的申购推进到 allocated。
    async fn list_user_subscriptions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinSubscriptionRead>> {
        let rows = sqlx::query_as::<_, NewCoinSubscriptionReadRow>(
            r#"SELECT id, project_id, user_id, quote_asset, quote_amount, requested_quantity,
                      allocated_quantity, status, idempotency_key, created_at
               FROM new_coin_subscriptions
               WHERE user_id = ?
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// 读取指定用户的新币分发记录，每行对应一次把认购结果落进钱包的动作。
    /// `subscription_id` 为空表示该笔分发不来自申购而是后台直接发放，
    /// `lock_position_id` 为空表示按项目规则无需锁仓，资产当时已直接进入可用余额。
    /// 按主键倒序取最新若干条，纯读路径既不会补发遗漏的分发，也不会改写分发状态，
    /// 更不校验引用的锁仓位置此刻是否仍然存在。
    async fn list_user_distributions(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinDistributionRead>> {
        let rows = sqlx::query_as::<_, NewCoinDistributionReadRow>(
            r#"SELECT id, project_id, user_id, subscription_id, asset_id, quantity,
                      lock_position_id, status, idempotency_key, created_at
               FROM new_coin_distributions
               WHERE user_id = ?
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// 读取指定用户的二级市场买入记录，返回下单时固化的价格、数量与计价总额三元快照。
    /// 该快照不随行情或项目配置变化而重算，因此可直接用于对账；
    /// 基础资产与计价资产以编号透传，`lock_position_id` 为空表示这笔买入未产生锁仓。
    /// 按主键倒序并受 `limit` 截断，`user_id` 参与查询条件，
    /// 不会串出其他用户的订单，也不会返回后台侧的撮合明细。
    async fn list_user_purchases(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinPurchaseRead>> {
        let rows = sqlx::query_as::<_, NewCoinPurchaseReadRow>(
            r#"SELECT id, project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                      quote_amount, lock_position_id, status, idempotency_key, created_at
               FROM new_coin_purchase_orders
               WHERE user_id = ?
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// 读取指定用户的解禁记录，一并带出解禁数量、计费用的解禁价格和该批次固化的整套收费口径。
    /// 收费字段包含是否启用、费率、计费基准（解禁市值或解禁收益）、支付资产与应付金额，
    /// 全部是分配当时写死的快照，后台事后调价不会追溯改写已有记录。
    /// `fee_paid_status` 表示缴费进度，`status` 表示释放进度，两者相互独立；
    /// 本查询只呈现状态，既不缴费也不释放锁仓，更不会因为已到期就自动推进状态。
    async fn list_user_unlocks(
        &self,
        user_id: u64,
        limit: u32,
    ) -> AppResult<Vec<NewCoinUnlockRead>> {
        let rows = sqlx::query_as::<_, NewCoinUnlockReadRow>(
            r#"SELECT id, user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
                      unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                      unlock_fee_amount, fee_paid_status, status, idempotency_key, created_at
               FROM asset_unlock_records
               WHERE user_id = ?
               ORDER BY id DESC
               LIMIT ?"#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

#[async_trait]
impl NewCoinUnlockFeeRepository for MySqlNewCoinReadRepository {
    /// 按解禁幂等键与 `user_id` 回读该条记录应收的手续费口径，即是否启用收费、支付资产和应付金额。
    /// 返回值刻意不含 `fee_paid_status`，只回答「应该收多少」，是否已收需另行查询，
    /// 两者分离可避免调用方把「应收」直接当成「已收」而错误放行。
    /// 记录不存在返回 `None`；查询不加行锁，结果返回后仍可能被并发缴费改写，
    /// 因此只适合做缴费前的金额比对，不能替代事务内的重复收费守卫。
    async fn find_unlock_fee_expectation(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<Option<UnlockFeeExpectation>> {
        let row = sqlx::query_as::<_, UnlockFeeExpectationRow>(
            r#"SELECT unlock_fee_enabled, unlock_fee_asset, unlock_fee_amount
               FROM asset_unlock_records
               WHERE idempotency_key = ? AND user_id = ?
               LIMIT 1"#,
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// 把匹配用户与幂等键、且当前非 paid 的解禁记录置为 paid，并覆盖记录上的费用资产与费用金额。
    /// 状态守卫写在 `WHERE` 里而不是先查后写，因此并发重复缴费只有一条 UPDATE 能命中，
    /// 返回 `true` 的调用在整个记录生命周期内至多出现一次，其余重放一律返回 `false`。
    /// 与同名的兼容入口一样，此实现只改解禁记录自身，
    /// 不扣钱包余额也不写 `wallet_ledger`，真正的资金扣减由上层在自己的事务中完成。
    async fn mark_unlock_fee_paid(&self, payment: UnlockFeePaymentWrite) -> AppResult<bool> {
        // 手续费支付状态使用幂等更新，重复支付同一解锁记录时不能重复改变业务状态。
        let result = sqlx::query(
            r#"UPDATE asset_unlock_records
               SET fee_paid_status = 'paid',
                   unlock_fee_asset = ?,
                   unlock_fee_amount = ?
               WHERE idempotency_key = ?
                 AND user_id = ?
                 AND fee_paid_status <> 'paid'"#,
        )
        .bind(payment.payment_asset_id)
        .bind(payment.amount)
        .bind(payment.unlock_idempotency_key)
        .bind(payment.user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }
}

#[async_trait]
impl NewCoinUnlockReleaseRepository for MySqlNewCoinReadRepository {
    /// 在单个事务内完成一笔到期解禁的资金释放，把锁仓额度转成可用余额并留下完整审计。
    /// 进入事务前先无锁确认该幂等键与用户存在对应记录，缺失直接返回 `NotFound`，不为非法键开事务。
    /// 事务内按固定顺序取锁：先用联表 `FOR UPDATE` 同时锁住解禁记录与其锁仓位置，
    /// 再锁钱包账户行，最后重读锁仓剩余量；解禁记录恒先于钱包加锁，
    /// 与下单路径「配置行在前、钱包行在后」的方向一致，两条资金链路不会互相等待成环。
    /// 放行条件必须同时成立：记录未释放、锁仓仍为 active、解禁时点已到、剩余量足够本次数量，
    /// 且项目未开启解禁收费或该记录已缴费。
    /// 条件不成立时若记录已是 released，判定为重放，提交空事务并以 `released = false`
    /// 回吐既有资产与数量；否则返回 `Validation` 表示未到期或未缴费，事务回滚不留痕迹。
    /// 资金只有一个流向：从 `wallet_accounts.locked` 扣减并等额加到 `available`，
    /// 全程不经过 `frozen` 中转；锁仓行同步累加 `released_amount`、扣减 `remaining_amount`，
    /// 减到零才把位置状态由 active 改为 released。
    /// 每次真实释放固定写两条 change_type 为 `new_coin_unlock_release` 的账本，
    /// 分别记录 locked 腿的负变动与 available 腿的正变动，ref_id 取解禁幂等键便于反查。
    /// 钱包账户缺失、locked 余额不足或锁仓剩余量被并发占用时整体回滚，
    /// 绝不出现只改了余额却没有账本、或只释放锁仓却没入账的中间态。
    async fn release_due_paid_unlock(
        &self,
        unlock_idempotency_key: &str,
        user_id: u64,
    ) -> AppResult<ReleaseUnlockOutcome> {
        let exists = sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM asset_unlock_records WHERE idempotency_key = ? AND user_id = ? LIMIT 1",
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        if exists.is_none() {
            return Err(AppError::NotFound);
        }

        let mut tx = self.pool.begin().await?;
        let Some(row) = sqlx::query_as::<_, ReleasableUnlockRow>(
            r#"SELECT unlocks.id AS unlock_id, unlocks.asset_id, unlocks.lock_position_id,
                      unlocks.unlock_quantity
               FROM asset_unlock_records unlocks
               INNER JOIN asset_lock_positions positions ON positions.id = unlocks.lock_position_id
               WHERE unlocks.idempotency_key = ? AND unlocks.user_id = ?
                 AND unlocks.status <> 'released'
                 AND positions.status = 'active'
                 AND positions.unlock_at <= CURRENT_TIMESTAMP(6)
                 AND positions.remaining_amount >= unlocks.unlock_quantity
                 AND (unlocks.unlock_fee_enabled = false OR unlocks.fee_paid_status = 'paid')
               LIMIT 1
               FOR UPDATE"#,
        )
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            if let Some((asset_id, unlock_quantity)) = sqlx::query_as::<_, (u64, BigDecimal)>(
                r#"SELECT asset_id, unlock_quantity
                   FROM asset_unlock_records
                   WHERE idempotency_key = ? AND user_id = ? AND status = 'released'
                   LIMIT 1"#,
            )
            .bind(unlock_idempotency_key)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            {
                tx.commit().await?;
                return Ok(ReleaseUnlockOutcome {
                    asset_id,
                    unlock_quantity,
                    released: false,
                });
            }
            return Err(AppError::Validation(
                "unlock is not releasable until unlock time is reached and required fee is paid"
                    .to_owned(),
            ));
        };

        let Some((available, frozen, locked)) =
            sqlx::query_as::<_, (BigDecimal, BigDecimal, BigDecimal)>(
                "SELECT available, frozen, locked FROM wallet_accounts WHERE user_id = ? AND asset_id = ? FOR UPDATE",
            )
            .bind(user_id)
            .bind(row.asset_id)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Err(AppError::Validation(
                "wallet account is required before unlock release".to_owned(),
            ));
        };

        if locked < row.unlock_quantity {
            return Err(AppError::Validation(
                "wallet locked balance is insufficient for unlock release".to_owned(),
            ));
        }

        let available_after = available + row.unlock_quantity.clone();
        let locked_after = locked - row.unlock_quantity.clone();

        let (remaining_before,) = sqlx::query_as::<_, (BigDecimal,)>(
            "SELECT remaining_amount FROM asset_lock_positions WHERE id = ? FOR UPDATE",
        )
        .bind(row.lock_position_id)
        .fetch_one(&mut *tx)
        .await?;
        let remaining_after = remaining_before - row.unlock_quantity.clone();
        let lock_status = if remaining_after == 0 {
            "released"
        } else {
            "active"
        };

        // 锁仓释放、解锁记录状态、钱包余额和双向流水必须在一个事务中完成，避免余额变化缺少审计记录。
        sqlx::query(
            r#"UPDATE asset_lock_positions
               SET released_amount = released_amount + ?,
                   remaining_amount = ?,
                   status = ?
               WHERE id = ? AND remaining_amount >= ?"#,
        )
        .bind(&row.unlock_quantity)
        .bind(&remaining_after)
        .bind(lock_status)
        .bind(row.lock_position_id)
        .bind(&row.unlock_quantity)
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE asset_unlock_records SET status = 'released' WHERE id = ?")
            .bind(row.unlock_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "UPDATE wallet_accounts SET available = ?, locked = ? WHERE user_id = ? AND asset_id = ?",
        )
        .bind(&available_after)
        .bind(&locked_after)
        .bind(user_id)
        .bind(row.asset_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"INSERT INTO wallet_ledger
               (user_id, asset_id, change_type, amount, balance_type, balance_after,
                available_after, frozen_after, locked_after, ref_type, ref_id)
               VALUES (?, ?, 'new_coin_unlock_release', ?, 'locked', ?, ?, ?, ?, 'new_coin_unlock', ?),
                      (?, ?, 'new_coin_unlock_release', ?, 'available', ?, ?, ?, ?, 'new_coin_unlock', ?)"#,
        )
        .bind(user_id)
        .bind(row.asset_id)
        .bind(-row.unlock_quantity.clone())
        .bind(&locked_after)
        .bind(&available_after)
        .bind(&frozen)
        .bind(&locked_after)
        .bind(unlock_idempotency_key)
        .bind(user_id)
        .bind(row.asset_id)
        .bind(&row.unlock_quantity)
        .bind(&available_after)
        .bind(&available_after)
        .bind(&frozen)
        .bind(&locked_after)
        .bind(unlock_idempotency_key)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(ReleaseUnlockOutcome {
            asset_id: row.asset_id,
            unlock_quantity: row.unlock_quantity,
            released: true,
        })
    }
}

#[async_trait]
impl NewCoinOrderRepository for MySqlNewCoinReadRepository {
    /// 按符号读取启用项目的下单规则，取的列比公开项目模型更窄，但覆盖风控判定所需的全部开关。
    /// 返回内容包含生命周期、发行价、上市时间、解禁类型与相对周期秒数、解禁费四要素，
    /// 以及上市后购买开关和后台限定的交易对编号。
    /// 该查询走连接池且不加锁，只用于下单前的预校验；
    /// 真正扣款前必须由事务内的 `FOR UPDATE` 重读再确认一次，否则会按过期规则成交。
    async fn find_project_rule_by_symbol(
        &self,
        symbol: &str,
    ) -> AppResult<Option<NewCoinProjectRuleRead>> {
        let sql = new_coin_project_rule_select_sql("symbol = ?", "LIMIT 1");
        let row = sqlx::query_as::<_, NewCoinProjectRuleReadRow>(&sql)
            .bind(symbol)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(Into::into))
    }

    /// 读取上市后购买要用的交易对，并在 SQL 层强制其 `base_asset` 等于项目资产且状态为 active。
    /// 把两个条件绑进同一条查询，可阻止调用方拿任意 `pair_id` 去买入不相干的币种，
    /// 不匹配、已下架或根本不存在时统一返回 `None`，本层不区分具体原因。
    /// 这里使用不加锁读取，返回的基础与计价资产编号仅供预校验；
    /// 真正成交时会在事务内以加锁版本重新确认，避免交易对被并发下架后仍然成交。
    async fn find_pair_for_purchase(
        &self,
        pair_id: u64,
        project_asset_id: u64,
    ) -> AppResult<Option<NewCoinPairRead>> {
        let row = sqlx::query_as::<_, NewCoinPairReadRow>(new_coin_pair_select_sql(false))
            .bind(pair_id)
            .bind(project_asset_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(Into::into))
    }

    /// 在单个事务内落地一笔新币申购：登记订单、扣计价资产、按锁仓计划分配新币，再把订单推进到 allocated。
    /// 与购买路径不同，本实现不在事务内重新锁定项目行，沿用调用方传入的项目规则快照
    /// 与其预先算好的锁仓计划，因此不防御「申购期间后台改规则」这一竞态。
    /// 事务首先以 `SELECT ... FOR UPDATE` 占位幂等键，键已存在即返回 `Conflict` 且不比较重放参数；
    /// 该行锁同时挡住同键并发请求，使「查重加插入」不会因竞态写出两张申购单。
    /// 订单先以 `pending` 与零配额落库，扣款和分配都成功后再改写为实际配额与 `allocated`，
    /// 因此中途失败回滚后不会残留一张显示已配额却没有资产到账的订单。
    /// 资金方向为计价资产 `available` 单向扣减，余额不足时整体回滚；
    /// 新币则按解禁规则进入 `locked`，无锁仓计划时直接落 `available`。
    /// 两段变动分别以 `new_coin_subscription_payment` 与 `new_coin_subscription_lock`
    /// 写入 `wallet_ledger`，ref_id 统一取申购幂等键，便于按单反查资金流。
    /// 返回首个锁仓位置编号，`None` 表示本次无需锁仓而是即时到账；本函数不发布任何事件。
    async fn create_subscription_order(
        &self,
        order: NewCoinSubscriptionOrderWrite,
    ) -> AppResult<Option<u64>> {
        let mut tx = self.pool.begin().await?;
        if idempotency_key_exists(&mut tx, "new_coin_subscriptions", &order.idempotency_key).await?
        {
            return Err(AppError::Conflict(
                "new coin subscription has already been created".to_owned(),
            ));
        }
        sqlx::query(
            r#"INSERT INTO new_coin_subscriptions
               (project_id, user_id, quote_asset, quote_amount, requested_quantity,
                allocated_quantity, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, 0, 'pending', ?)"#,
        )
        .bind(order.project.id)
        .bind(order.user_id)
        .bind(order.quote_asset_id)
        .bind(&order.quote_amount)
        .bind(&order.quantity)
        .bind(&order.idempotency_key)
        .execute(&mut *tx)
        .await?;

        debit_wallet_available(
            &mut tx,
            order.user_id,
            order.quote_asset_id,
            &order.quote_amount,
            NewCoinLedgerMetadata {
                change_type: "new_coin_subscription_payment",
                ref_type: "new_coin_subscription",
                ref_id: &order.idempotency_key,
            },
        )
        .await?;
        let lock_position_id = apply_new_coin_allocation(
            &mut tx,
            order.user_id,
            order.project.asset_id,
            &order.quantity,
            &order.lock_positions,
            &order.project.issue_price,
            &order.quote_amount,
            &order.project,
            NewCoinLedgerMetadata {
                change_type: "new_coin_subscription_lock",
                ref_type: "new_coin_subscription",
                ref_id: &order.idempotency_key,
            },
        )
        .await?;
        sqlx::query(
            "UPDATE new_coin_subscriptions SET allocated_quantity = ?, status = 'allocated' WHERE idempotency_key = ?",
        )
        .bind(&order.quantity)
        .bind(&order.idempotency_key)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(lock_position_id)
    }

    /// 在单个事务内落地一笔二级市场买入：校验项目与交易对、登记订单、扣计价资产、锁仓新币并置为 locked。
    /// 加锁顺序固定为项目行、交易对行、订单幂等键、钱包行、锁仓行，由粗粒度配置逐级下探到细粒度资金，
    /// 先锁项目可阻止后台在同一瞬间关闭购买开关或换交易对，从而杜绝按旧快照成交。
    /// 事务内重读到的项目必须仍处于 `listed` 且开启上市后购买，请求的交易对必须正是项目指定的那一个，
    /// 交易对自身还要求基础资产等于项目资产且状态 active，任一不满足即回滚并返回 `Validation` 或 `NotFound`。
    /// 锁仓计划基于事务内的项目规则与 `Utc::now()` 现算，因此相对周期类解禁以实际成交时刻为起点，
    /// 而不是以请求到达时刻为起点。
    /// 幂等键以 `FOR UPDATE` 占位，任何重复键一律返回 `Conflict`，既不比对参数也不回读既有订单。
    /// 资金方向为计价资产 `available` 单向扣减，新币按解禁规则进 `locked` 或在无锁仓计划时直接进 `available`，
    /// 分别以 `new_coin_purchase_payment` 与 `new_coin_purchase_lock` 写账本，ref_id 取购买幂等键。
    /// 订单先落 `pending` 且锁仓编号为空，成功后回填首个锁仓位置编号并置为 `locked`；
    /// 返回值即该编号，`None` 表示无锁仓的即时到账。本函数不发布任何事件。
    async fn create_purchase_order(
        &self,
        order: NewCoinPurchaseOrderWrite,
    ) -> AppResult<Option<u64>> {
        let mut tx = self.pool.begin().await?;
        // 下单事务内重新锁定项目和交易对，避免后台刚关闭认购或调整规则后用户仍按旧快照成交。
        let locked_project =
            lock_purchase_project_in_tx(&mut tx, order.project.id, order.pair_id).await?;
        let locked_pair =
            lock_pair_for_purchase_in_tx(&mut tx, order.pair_id, locked_project.asset_id).await?;
        let lock_positions = lock_positions_for_project(
            &locked_project,
            order.user_id,
            locked_project.asset_id,
            &order.idempotency_key,
            order.quantity.clone(),
            Utc::now(),
            "new_coin_purchase",
        )?;
        if idempotency_key_exists(&mut tx, "new_coin_purchase_orders", &order.idempotency_key)
            .await?
        {
            return Err(AppError::Conflict(
                "new coin purchase has already been created".to_owned(),
            ));
        }
        sqlx::query(
            r#"INSERT INTO new_coin_purchase_orders
               (project_id, user_id, pair_id, base_asset, quote_asset, price, quantity,
                quote_amount, lock_position_id, status, idempotency_key)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, 'pending', ?)"#,
        )
        .bind(locked_project.id)
        .bind(order.user_id)
        .bind(order.pair_id)
        .bind(locked_pair.base_asset_id)
        .bind(locked_pair.quote_asset_id)
        .bind(&order.price)
        .bind(&order.quantity)
        .bind(&order.quote_amount)
        .bind(&order.idempotency_key)
        .execute(&mut *tx)
        .await?;

        debit_wallet_available(
            &mut tx,
            order.user_id,
            locked_pair.quote_asset_id,
            &order.quote_amount,
            NewCoinLedgerMetadata {
                change_type: "new_coin_purchase_payment",
                ref_type: "new_coin_purchase",
                ref_id: &order.idempotency_key,
            },
        )
        .await?;
        let lock_position_id = apply_new_coin_allocation(
            &mut tx,
            order.user_id,
            locked_project.asset_id,
            &order.quantity,
            &lock_positions,
            &order.price,
            &order.quote_amount,
            &locked_project,
            NewCoinLedgerMetadata {
                change_type: "new_coin_purchase_lock",
                ref_type: "new_coin_purchase",
                ref_id: &order.idempotency_key,
            },
        )
        .await?;
        sqlx::query(
            "UPDATE new_coin_purchase_orders SET lock_position_id = ?, status = 'locked' WHERE idempotency_key = ?",
        )
        .bind(lock_position_id)
        .bind(&order.idempotency_key)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(lock_position_id)
    }
}

/// 在下单事务内以 `FOR UPDATE` 重新读取并锁定项目行，把后续校验建立在最新配置而非请求期快照上。
/// 项目缺失或已被停用返回 `NotFound`；生命周期不是 `listed` 说明尚未开放二级市场买入，返回 `Validation`。
/// 随后交由 `ensure_post_listing_purchase_enabled` 确认购买开关已打开，
/// 且请求的 `requested_pair_id` 正是项目配置的那一个交易对。
/// 该行锁一直持有到事务结束，期间后台对同一项目的配置修改会被阻塞，
/// 从而消除「校验通过之后规则被改、却仍按旧规则扣款锁仓」的时间窗口。
async fn lock_purchase_project_in_tx(
    tx: &mut Transaction<'_, MySql>,
    project_id: u64,
    requested_pair_id: u64,
) -> AppResult<NewCoinProjectRuleRead> {
    let sql = new_coin_project_rule_select_sql("id = ?", "LIMIT 1 FOR UPDATE");
    let project = sqlx::query_as::<_, NewCoinProjectRuleReadRow>(&sql)
        .bind(project_id)
        .fetch_optional(&mut **tx)
        .await?
        .map(NewCoinProjectRuleRead::from)
        .ok_or(AppError::NotFound)?;
    if lifecycle_status(&project.lifecycle_status)? != LifecycleStatus::Listed {
        return Err(AppError::Validation(
            "post-listing new coin purchase is not open for this project".to_owned(),
        ));
    }
    ensure_post_listing_purchase_enabled(&project, requested_pair_id)?;
    Ok(project)
}

/// 在下单事务内以 `FOR UPDATE` 锁定交易对行，并要求其基础资产恰为项目资产、状态为 active。
/// 加锁位置固定排在项目行之后、钱包行之前，保证同一笔买入涉及的行按由粗到细的单一方向获取。
/// 交易对不存在、已下架或基础资产与项目不符时返回 `NotFound`；
/// 返回的基础与计价资产编号会直接写进订单行，成为该笔买入的资产口径。
async fn lock_pair_for_purchase_in_tx(
    tx: &mut Transaction<'_, MySql>,
    pair_id: u64,
    project_asset_id: u64,
) -> AppResult<NewCoinPairRead> {
    sqlx::query_as::<_, NewCoinPairReadRow>(new_coin_pair_select_sql(true))
        .bind(pair_id)
        .bind(project_asset_id)
        .fetch_optional(&mut **tx)
        .await?
        .map(NewCoinPairRead::from)
        .ok_or(AppError::NotFound)
}

/// 拼装项目下单规则的查询语句，让不加锁的预校验与事务内的 `FOR UPDATE` 重读共用同一份列清单。
/// `predicate` 提供主键或符号等定位条件，`suffix` 追加 `LIMIT` 与可选的 `FOR UPDATE`。
/// 语句固定附加 `status = 'active'`，因此被停用的项目在任何调用点都读不到。
/// 两个参数都由本模块以字面量传入，不接受任何外部输入，不存在 SQL 注入面。
fn new_coin_project_rule_select_sql(predicate: &str, suffix: &str) -> String {
    format!(
        r#"SELECT id, asset_id, lifecycle_status, issue_price, listed_at, unlock_type,
                  fixed_unlock_at, relative_unlock_seconds, unlock_fee_enabled,
                  unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
                  post_listing_purchase_enabled, post_listing_pair_id
           FROM new_coin_projects
           WHERE {predicate} AND status = 'active'
           {suffix}"#,
    )
}

/// 返回交易对查询的静态语句，`for_update` 只决定是否追加行锁，其余列与过滤条件两版完全一致。
/// 不加锁版本供下单前预校验使用，加锁版本供事务内重读使用，
/// 共用同一份 SQL 可避免两处的资产匹配与状态条件随时间漂移到不一致。
/// 两版都要求交易对状态为 active 且基础资产等于传入的项目资产，参数一律以占位符绑定。
fn new_coin_pair_select_sql(for_update: bool) -> &'static str {
    if for_update {
        r#"SELECT base_asset AS base_asset_id, quote_asset AS quote_asset_id
           FROM trading_pairs
           WHERE id = ? AND base_asset = ? AND status = 'active'
           LIMIT 1
           FOR UPDATE"#
    } else {
        r#"SELECT base_asset AS base_asset_id, quote_asset AS quote_asset_id
           FROM trading_pairs
           WHERE id = ? AND base_asset = ? AND status = 'active'
           LIMIT 1"#
    }
}

/// 在事务内以 `SELECT ... LIMIT 1 FOR UPDATE` 探测目标表是否已存在该幂等键，同时兼作并发占位。
/// 命中时取到的行锁、未命中时取到的间隙锁都会持有到事务结束，使同键并发请求被迫串行，
/// 因此调用方的「先查重再插入」两步操作不会因竞态写出两条订单。
/// 表名由本模块以字面量传入并直接拼进 SQL，幂等键则走占位符绑定，调用方不得传入外部字符串作为表名。
async fn idempotency_key_exists(
    tx: &mut Transaction<'_, MySql>,
    table_name: &str,
    idempotency_key: &str,
) -> AppResult<bool> {
    let mut query = QueryBuilder::<MySql>::new("SELECT id FROM ");
    query
        .push(table_name)
        .push(" WHERE idempotency_key = ")
        .push_bind(idempotency_key)
        .push(" LIMIT 1 FOR UPDATE");
    let exists: Option<(u64,)> = query.build_query_as().fetch_optional(&mut **tx).await?;
    Ok(exists.is_some())
}

/// 把一笔已付款订单对应的新币额度落到用户钱包，按有无锁仓计划走两条互斥路径。
/// 锁仓计划为空表示该项目无需锁定，全额直接进 `available` 并写一条 available 腿账本，返回 `None`。
/// 存在锁仓计划时先锁定或创建钱包行，把全部数量一次性计入 `locked` 并写一条 locked 腿账本；
/// 账本里的 available 与 frozen 快照取自加锁时读到的值，因此与本事务提交后的钱包三态一致。
/// 钱包余额只加计一次，随后才逐条 upsert 锁仓位置并为每条位置补建解禁记录，
/// 由此保证「钱包锁定总额」恒等于「各解禁批次金额之和」。
/// 入参的 `unlock_price` 与 `purchase_cost` 会原样写进每条解禁记录，
/// 成为日后按解禁市值或按解禁收益计费时的固定口径，本函数不做单位换算。
/// 返回首个锁仓位置编号供订单行回填；多批次解禁时其余编号不外露，需要时应按 merge_key 反查锁仓表。
/// 本函数不校验数量正负也不检查项目状态，全部前置约束由调用它的下单事务负责。
#[allow(clippy::too_many_arguments)]
async fn apply_new_coin_allocation(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    quantity: &BigDecimal,
    lock_positions: &[NewCoinLockPositionWrite],
    unlock_price: &BigDecimal,
    purchase_cost: &BigDecimal,
    project: &NewCoinProjectRuleRead,
    ledger: NewCoinLedgerMetadata<'_>,
) -> AppResult<Option<u64>> {
    if lock_positions.is_empty() {
        credit_wallet_available(
            tx,
            user_id,
            asset_id,
            quantity,
            ledger.change_type,
            ledger.ref_type,
            ledger.ref_id,
        )
        .await?;
        return Ok(None);
    }

    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let locked_after = wallet.locked.clone() + quantity.clone();
    sqlx::query("UPDATE wallet_accounts SET locked = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&locked_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_new_coin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        quantity.clone(),
        "locked",
        &locked_after,
        &wallet.available,
        &wallet.frozen,
        &locked_after,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await?;

    let mut first_lock_position_id = None;
    for position in lock_positions {
        let position_id = upsert_lock_position(tx, position).await?;
        ensure_unlock_record(
            tx,
            user_id,
            asset_id,
            position_id,
            &position.amount,
            unlock_price,
            purchase_cost,
            project,
            &position.source_id,
        )
        .await?;
        if first_lock_position_id.is_none() {
            first_lock_position_id = Some(position_id);
        }
    }
    Ok(first_lock_position_id)
}

/// 为一条锁仓位置补建解禁记录，把该批次将来解禁时适用的收费口径在此刻一次性固化。
/// 收费字段由 `unlock_fee_fields` 依据项目规则、本批数量、解禁价与购买成本算出，
/// 同时给出初始 `fee_paid_status`：未开启收费为 not_required，需要收费则为 pending。
/// 记录以 `source_id` 作为幂等键落库，`ON DUPLICATE KEY UPDATE updated_at = updated_at`
/// 使重复调用退化为空写，因此同一订单重跑既不会重复登记应收费用，
/// 也不会把已经缴过费的记录退回 pending。
/// 记录的释放状态初始为 pending，真正的资产释放由 `release_due_paid_unlock` 在到期并缴费后完成。
#[allow(clippy::too_many_arguments)]
async fn ensure_unlock_record(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    quantity: &BigDecimal,
    unlock_price: &BigDecimal,
    purchase_cost: &BigDecimal,
    project: &NewCoinProjectRuleRead,
    source_id: &str,
) -> AppResult<()> {
    let (fee_paid_status, unlock_fee_amount) =
        unlock_fee_fields(project, quantity, unlock_price, purchase_cost)?;
    sqlx::query(
        r#"INSERT INTO asset_unlock_records
           (user_id, asset_id, lock_position_id, unlock_quantity, unlock_price,
            unlock_fee_enabled, unlock_fee_rate, unlock_fee_basis, unlock_fee_asset,
            unlock_fee_amount, fee_paid_status, status, idempotency_key)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(lock_position_id)
    .bind(quantity)
    .bind(unlock_price)
    .bind(project.unlock_fee_enabled)
    .bind(&project.unlock_fee_rate)
    .bind(&project.unlock_fee_basis)
    .bind(project.unlock_fee_asset)
    .bind(&unlock_fee_amount)
    .bind(fee_paid_status)
    .bind(source_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在事务内从用户某资产的 `available` 单向扣减指定金额，是新币申购与购买唯一的付款出口。
/// 先以 `FOR UPDATE` 锁定钱包行再比较余额，把「读余额」与「写余额」压在同一把行锁内，杜绝并发超扣。
/// 钱包行必须已经存在，缺失时返回 `Validation` 而不是隐式建号，
/// 避免为本不该持有该资产的用户凭空开户后再扣款。
/// 余额不足同样返回 `Validation`，错误信息带上请求额、可用额与锁仓额，
/// 便于前端区分「钱不够」和「钱被锁仓占住」两种情况。
/// 扣款后立即写一条 available 腿账本，金额取负值，`frozen` 与 `locked` 沿用本次未变动的原值。
/// 资金不经过 `frozen` 中转，一步从可用余额离开账户。
async fn debit_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    ledger: NewCoinLedgerMetadata<'_>,
) -> AppResult<()> {
    let wallet = lock_wallet_row(tx, user_id, asset_id).await?;
    if wallet.available < *amount {
        return Err(AppError::Validation(format!(
            "insufficient available balance for new coin order: requested {}, available {}, locked {}",
            amount, wallet.available, wallet.locked
        )));
    }
    let available_after = wallet.available.clone() - amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_new_coin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        -amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        ledger.change_type,
        ledger.ref_type,
        ledger.ref_id,
    )
    .await
}

/// 在事务内向用户某资产的 `available` 单向加计金额，用于项目无需锁仓时把新币直接发到用户手上。
/// 与付款路径相反，此处使用「锁定或创建」钱包行，用户首次持有该资产时自动开号，不会因缺号而失败。
/// 入账不做任何上限校验，调用方须自行保证金额为正，
/// 传入负数会写出反向变动并让账本与实际业务语义脱节。
/// 变动后写一条 available 腿账本，金额取正值，`frozen` 与 `locked` 沿用加锁时读到的快照。
/// 账本的 change_type 与 ref 由调用方按申购或购买场景分别传入，本函数不做场景判断。
async fn credit_wallet_available(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    let wallet = lock_or_create_wallet_row(tx, user_id, asset_id).await?;
    let available_after = wallet.available.clone() + amount.clone();
    sqlx::query("UPDATE wallet_accounts SET available = ? WHERE user_id = ? AND asset_id = ?")
        .bind(&available_after)
        .bind(user_id)
        .bind(asset_id)
        .execute(&mut **tx)
        .await?;
    insert_new_coin_wallet_ledger(
        tx,
        user_id,
        asset_id,
        amount.clone(),
        "available",
        &available_after,
        &available_after,
        &wallet.frozen,
        &wallet.locked,
        change_type,
        ref_type,
        ref_id,
    )
    .await
}

/// 以 `FOR UPDATE` 锁定并读取用户某资产的钱包行，一次取回 available、frozen、locked 三态余额。
/// 行锁持有到事务结束，是本模块所有资金写入的强制前置动作，
/// 确保读到的余额在本事务提交之前不会被其他会话改写。
/// 钱包行不存在时返回 `Validation` 而不是自动创建，付款路径正是依赖这一点拒绝为无账户用户扣款。
/// 需要自动开户的入账场景应改用 `lock_or_create_wallet_row`，两者的锁语义完全相同。
async fn lock_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<NewCoinWalletRead> {
    sqlx::query_as::<_, NewCoinWalletReadRow>(
        r#"SELECT available, frozen, locked
           FROM wallet_accounts
           WHERE user_id = ? AND asset_id = ?
           LIMIT 1
           FOR UPDATE"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(NewCoinWalletRead::from)
    .ok_or_else(|| AppError::Validation("wallet account is required for new coin order".to_owned()))
}

/// 确保用户在该资产上存在钱包行之后再加锁读取，供入账与锁仓等不应因缺号而失败的路径使用。
/// 先执行 `INSERT ... ON DUPLICATE KEY UPDATE updated_at = updated_at`：
/// 行已存在时是空写，绝不会把任何一态余额重置为零；行不存在时按列默认值建出三态全零的新账户。
/// 随后复用 `lock_wallet_row` 取行锁并回读余额，
/// 因此并发首次开户由唯一键收敛，最终至多留下一行，且后续读到的必定是加锁后的最新值。
async fn lock_or_create_wallet_row(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
) -> AppResult<NewCoinWalletRead> {
    sqlx::query(
        r#"INSERT INTO wallet_accounts (user_id, asset_id)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .execute(&mut **tx)
    .await?;
    lock_wallet_row(tx, user_id, asset_id).await
}

/// 按 merge_key 归并锁仓位置并登记一条来源明细，是新币批次解禁在存储层的幂等落点。
/// 先用 upsert 建出或命中位置行：新建时三个金额列都是零，命中时只碰 `updated_at` 而不动金额；
/// 命中分支再按 merge_key 加 `FOR UPDATE` 回读主键，使并发写同一位置的请求排队而不是各自累加。
/// 真正决定金额是否累加的是来源表的 `INSERT IGNORE`：同一 `source_id` 只能插入一次，
/// 只有本次确实插入了新来源，才把该来源金额同时累加到位置的 `locked_amount` 与 `remaining_amount`。
/// 因此同一订单重跑不会把额度翻倍，而不同订单命中同一 merge_key 时会正确合并成同一个解禁批次。
/// 累加时把状态一并重置为 active，使此前已释放完毕的位置在收到新来源后重新变为可解禁。
/// 本函数只维护锁仓位置与来源两张表，不改钱包余额，钱包侧的 `locked` 由调用方一次性加计。
async fn upsert_lock_position(
    tx: &mut Transaction<'_, MySql>,
    position: &NewCoinLockPositionWrite,
) -> AppResult<u64> {
    let result = sqlx::query(
        r#"INSERT INTO asset_lock_positions
           (user_id, asset_id, unlock_type, unlock_at, locked_amount,
            released_amount, remaining_amount, merge_key, status)
           VALUES (?, ?, ?, ?, 0, 0, 0, ?, 'active')
           ON DUPLICATE KEY UPDATE updated_at = updated_at"#,
    )
    .bind(position.user_id)
    .bind(position.asset_id)
    .bind(&position.unlock_type)
    .bind(position.unlock_at.naive_utc())
    .bind(&position.merge_key)
    .execute(&mut **tx)
    .await?;

    let position_id = if result.last_insert_id() == 0 {
        sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM asset_lock_positions WHERE merge_key = ? LIMIT 1 FOR UPDATE",
        )
        .bind(&position.merge_key)
        .fetch_one(&mut **tx)
        .await?
        .0
    } else {
        result.last_insert_id()
    };

    let inserted = sqlx::query(
        r#"INSERT IGNORE INTO asset_lock_position_sources
           (lock_position_id, source_type, source_id, source_amount, source_time)
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(position_id)
    .bind(&position.source_type)
    .bind(&position.source_id)
    .bind(&position.amount)
    .bind(position.unlock_at.naive_utc())
    .execute(&mut **tx)
    .await?;

    if inserted.rows_affected() > 0 {
        sqlx::query(
            r#"UPDATE asset_lock_positions
               SET locked_amount = locked_amount + ?,
                   remaining_amount = remaining_amount + ?,
                   status = 'active'
               WHERE id = ?"#,
        )
        .bind(&position.amount)
        .bind(&position.amount)
        .bind(position_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(position_id)
}

/// 向 `wallet_ledger` 写入一条新币资金流水，是本模块所有余额变动的统一审计出口。
/// `amount` 是带符号的本次变动量，`balance_type` 标注这次变动落在 available、frozen 还是 locked 哪条腿，
/// `balance_after` 是该腿变动后的值，紧随其后的三个 after 参数则是变动后完整的三态快照。
/// 调用方必须传入与钱包更新语句完全一致的数值，本函数不回读钱包核对，
/// 传错会直接写出对不上账的流水且不会报错。
/// change_type 用于区分付款、锁仓、释放等场景，ref_type 与 ref_id 通常取业务类型与幂等键，
/// 便于按订单反查整条资金链路。
/// 一次调用只写一条记录，同时影响两条腿的动作需要调用方分别写入两次。
#[allow(clippy::too_many_arguments)]
async fn insert_new_coin_wallet_ledger(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
    asset_id: u64,
    amount: BigDecimal,
    balance_type: &str,
    balance_after: &BigDecimal,
    available_after: &BigDecimal,
    frozen_after: &BigDecimal,
    locked_after: &BigDecimal,
    change_type: &str,
    ref_type: &str,
    ref_id: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO wallet_ledger
           (user_id, asset_id, change_type, amount, balance_type, balance_after,
            available_after, frozen_after, locked_after, ref_type, ref_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(user_id)
    .bind(asset_id)
    .bind(change_type)
    .bind(amount)
    .bind(balance_type)
    .bind(balance_after)
    .bind(available_after)
    .bind(frozen_after)
    .bind(locked_after)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinProjectReadRow {
    id: u64,
    asset_id: u64,
    symbol: String,
    lifecycle_status: String,
    total_supply: BigDecimal,
    issue_price: BigDecimal,
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    unlock_type: String,
    fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    relative_unlock_seconds: Option<u64>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    post_listing_purchase_enabled: bool,
    post_listing_pair_id: Option<u64>,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinSubscriptionReadRow {
    id: u64,
    project_id: u64,
    user_id: u64,
    quote_asset: u64,
    quote_amount: BigDecimal,
    requested_quantity: BigDecimal,
    allocated_quantity: BigDecimal,
    status: String,
    idempotency_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinDistributionReadRow {
    id: u64,
    project_id: u64,
    user_id: u64,
    subscription_id: Option<u64>,
    asset_id: u64,
    quantity: BigDecimal,
    lock_position_id: Option<u64>,
    status: String,
    idempotency_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinPurchaseReadRow {
    id: u64,
    project_id: u64,
    user_id: u64,
    pair_id: u64,
    base_asset: u64,
    quote_asset: u64,
    price: BigDecimal,
    quantity: BigDecimal,
    quote_amount: BigDecimal,
    lock_position_id: Option<u64>,
    status: String,
    idempotency_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinUnlockReadRow {
    id: u64,
    user_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
    unlock_price: Option<BigDecimal>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    unlock_fee_amount: Option<BigDecimal>,
    fee_paid_status: String,
    status: String,
    idempotency_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct UnlockFeeExpectationRow {
    unlock_fee_enabled: bool,
    unlock_fee_asset: Option<u64>,
    unlock_fee_amount: Option<BigDecimal>,
}

#[derive(Debug, sqlx::FromRow)]
struct ReleasableUnlockRow {
    unlock_id: u64,
    asset_id: u64,
    lock_position_id: u64,
    unlock_quantity: BigDecimal,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinProjectRuleReadRow {
    id: u64,
    asset_id: u64,
    lifecycle_status: String,
    issue_price: BigDecimal,
    listed_at: Option<chrono::DateTime<chrono::Utc>>,
    unlock_type: String,
    fixed_unlock_at: Option<chrono::DateTime<chrono::Utc>>,
    relative_unlock_seconds: Option<u64>,
    unlock_fee_enabled: bool,
    unlock_fee_rate: Option<BigDecimal>,
    unlock_fee_basis: Option<String>,
    unlock_fee_asset: Option<u64>,
    post_listing_purchase_enabled: bool,
    post_listing_pair_id: Option<u64>,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinPairReadRow {
    base_asset_id: u64,
    quote_asset_id: u64,
}

#[derive(Debug, sqlx::FromRow)]
struct NewCoinWalletReadRow {
    available: BigDecimal,
    frozen: BigDecimal,
    locked: BigDecimal,
}

impl From<NewCoinProjectReadRow> for NewCoinProjectRead {
    /// 把公开项目查询行逐字段搬进只读模型，字段一一对应，不做默认值填充、单位换算或状态推断。
    /// 解禁与收费相关列在数据库中允许为空，这里原样保留 `Option`，
    /// 由上层按解禁类型自行判断哪些组合有效，例如相对周期解禁才关心 `relative_unlock_seconds`。
    fn from(row: NewCoinProjectReadRow) -> Self {
        Self {
            id: row.id,
            asset_id: row.asset_id,
            symbol: row.symbol,
            lifecycle_status: row.lifecycle_status,
            total_supply: row.total_supply,
            issue_price: row.issue_price,
            listed_at: row.listed_at,
            unlock_type: row.unlock_type,
            fixed_unlock_at: row.fixed_unlock_at,
            relative_unlock_seconds: row.relative_unlock_seconds,
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_rate: row.unlock_fee_rate,
            unlock_fee_basis: row.unlock_fee_basis,
            unlock_fee_asset: row.unlock_fee_asset,
            post_listing_purchase_enabled: row.post_listing_purchase_enabled,
            post_listing_pair_id: row.post_listing_pair_id,
            status: row.status,
        }
    }
}

impl From<NewCoinSubscriptionReadRow> for NewCoinSubscriptionRead {
    /// 平移申购单查询行，保留申请数量与实际配额数量两个独立字段，不在此处推算中签比例。
    /// `status` 与 `idempotency_key` 原样透传，供上层区分 pending 与 allocated，并按幂等键对账。
    fn from(row: NewCoinSubscriptionReadRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            user_id: row.user_id,
            quote_asset: row.quote_asset,
            quote_amount: row.quote_amount,
            requested_quantity: row.requested_quantity,
            allocated_quantity: row.allocated_quantity,
            status: row.status,
            idempotency_key: row.idempotency_key,
            created_at: row.created_at,
        }
    }
}

impl From<NewCoinDistributionReadRow> for NewCoinDistributionRead {
    /// 平移分发记录查询行，两个可空外键的语义在转换中被完整保留而不折叠成零值。
    /// `subscription_id` 为空表示这笔分发不来自申购，
    /// `lock_position_id` 为空表示资产当时未锁仓而是直接进入了可用余额。
    fn from(row: NewCoinDistributionReadRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            user_id: row.user_id,
            subscription_id: row.subscription_id,
            asset_id: row.asset_id,
            quantity: row.quantity,
            lock_position_id: row.lock_position_id,
            status: row.status,
            idempotency_key: row.idempotency_key,
            created_at: row.created_at,
        }
    }
}

impl From<NewCoinPurchaseReadRow> for NewCoinPurchaseRead {
    /// 平移二级市场买入单查询行，价格、数量与计价总额均为成交时固化的快照，转换中不重新相乘校验。
    /// 基础资产与计价资产以数值编号透传，不在此处解析成符号，避免映射层依赖资产字典。
    /// `lock_position_id` 为空表示这笔买入按项目规则无需锁仓。
    fn from(row: NewCoinPurchaseReadRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            user_id: row.user_id,
            pair_id: row.pair_id,
            base_asset: row.base_asset,
            quote_asset: row.quote_asset,
            price: row.price,
            quantity: row.quantity,
            quote_amount: row.quote_amount,
            lock_position_id: row.lock_position_id,
            status: row.status,
            idempotency_key: row.idempotency_key,
            created_at: row.created_at,
        }
    }
}

impl From<NewCoinUnlockReadRow> for NewCoinUnlockRead {
    /// 平移解禁记录查询行，把该批次固化的整套收费口径连同两个状态字段一并带出。
    /// 收费列允许为空以表示项目未配置收费，转换保留空值而不折叠为零，
    /// 避免把「未配置收费」和「费率为零」这两种业务含义混为一谈。
    /// `fee_paid_status` 表示缴费进度，`status` 表示释放进度，两者相互独立，转换不做一致性推断。
    fn from(row: NewCoinUnlockReadRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            asset_id: row.asset_id,
            lock_position_id: row.lock_position_id,
            unlock_quantity: row.unlock_quantity,
            unlock_price: row.unlock_price,
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_rate: row.unlock_fee_rate,
            unlock_fee_basis: row.unlock_fee_basis,
            unlock_fee_asset: row.unlock_fee_asset,
            unlock_fee_amount: row.unlock_fee_amount,
            fee_paid_status: row.fee_paid_status,
            status: row.status,
            idempotency_key: row.idempotency_key,
            created_at: row.created_at,
        }
    }
}

impl From<UnlockFeeExpectationRow> for UnlockFeeExpectation {
    /// 平移解禁应收费用的三列查询结果，只回答「是否收费、收什么资产、收多少」。
    /// 刻意不携带缴费状态，使调用方无法把「应收」误当成「已收」，是否已缴需要另行查询确认。
    fn from(row: UnlockFeeExpectationRow) -> Self {
        Self {
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_asset: row.unlock_fee_asset,
            unlock_fee_amount: row.unlock_fee_amount,
        }
    }
}

impl From<NewCoinProjectRuleReadRow> for NewCoinProjectRuleRead {
    /// 平移下单规则查询行，供不加锁预校验与事务内加锁重读共用同一份内存表示。
    /// 与公开项目模型相比少了符号、总供应量和状态列，多出的部分正是下单必须判定的解禁与购买开关配置。
    /// 所有可空列原样保留，例如未上市项目的 `listed_at` 为空、
    /// 非相对周期解禁的 `relative_unlock_seconds` 为空，转换不为它们编造默认值。
    fn from(row: NewCoinProjectRuleReadRow) -> Self {
        Self {
            id: row.id,
            asset_id: row.asset_id,
            lifecycle_status: row.lifecycle_status,
            issue_price: row.issue_price,
            listed_at: row.listed_at,
            unlock_type: row.unlock_type,
            fixed_unlock_at: row.fixed_unlock_at,
            relative_unlock_seconds: row.relative_unlock_seconds,
            unlock_fee_enabled: row.unlock_fee_enabled,
            unlock_fee_rate: row.unlock_fee_rate,
            unlock_fee_basis: row.unlock_fee_basis,
            unlock_fee_asset: row.unlock_fee_asset,
            post_listing_purchase_enabled: row.post_listing_purchase_enabled,
            post_listing_pair_id: row.post_listing_pair_id,
        }
    }
}

impl From<NewCoinPairReadRow> for NewCoinPairRead {
    /// 平移交易对查询行，把 SQL 中已别名为基础资产与计价资产的两列搬进只读模型。
    /// 查询本身已强制基础资产等于项目资产，因此转换不再重复校验这两个编号之间的关系。
    fn from(row: NewCoinPairReadRow) -> Self {
        Self {
            base_asset_id: row.base_asset_id,
            quote_asset_id: row.quote_asset_id,
        }
    }
}

impl From<NewCoinWalletReadRow> for NewCoinWalletRead {
    /// 平移钱包三态余额查询行：available 是可动用额，frozen 是挂单等场景的冻结额，
    /// locked 是新币锁仓额，三者互不重叠，转换既不求和也不校验总量。
    /// 调用方拿到的是加锁那一刻的快照，一旦离开事务即可能过期，不得缓存复用。
    fn from(row: NewCoinWalletReadRow) -> Self {
        Self {
            available: row.available,
            frozen: row.frozen,
            locked: row.locked,
        }
    }
}
