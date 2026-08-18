//! events bounded context infrastructure layer.
//!
//! 基础设施层：封装事件 outbox / inbox 的 SQLx 持久化与并发保护细节。
//!
//! 整体投递语义为 at-least-once。outbox 侧的去重键是 `idempotency_key` 的唯一约束，
//! 业务重复写入同一事件只会得到既有记录；但发布扫描不加领取锁，多实例可能同时读到同一行并各发一次，
//! 因此重复投递由下游 inbox 的去重承担而不是靠 outbox 阻止。
//! inbox 侧的去重键是 `consumer_name` 加 `message_id` 或 `idempotency_key`，终态消息重复投递一律判为重复。
//!
//! 并发所有权用「乐观租约」表达：领取时把 `updated_at` 的微秒时间戳编成处理令牌，
//! 后续推进 consumed、retry、dead_letter 都要把该令牌作为 `updated_at` 的等值条件；
//! 影响行数为零即说明租约已被他人接管，操作被拒。租约超过 300 秒未推进即可被重新竞争，
//! 使 worker 崩溃后的消息不会永久卡在 processing。
//!
//! 顺序保证的边界：发布扫描按 `id` 升序取批，但由于不加锁、多实例并发且失败会退避重排，
//! 全局有序不成立，消费方不得依赖事件到达顺序。
//! 死信不会被自动重投，也不转发到独立死信交换机，只能通过带管理员审计的显式重排用例恢复。

use crate::error::{AppError, AppResult};
use axum::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::{
    Error as SqlxError, MySql, Pool, Transaction, error::DatabaseError, types::Json as SqlxJson,
};

use crate::modules::events::domain::{
    INBOX_CONSUMED, INBOX_DEAD_LETTER, INBOX_PROCESSING, INBOX_PROCESSING_LEASE_SECONDS,
    INBOX_PROCESSING_TOKEN_FORMAT, INBOX_RETRY, OUTBOX_DEAD_LETTER, OUTBOX_PENDING,
    OUTBOX_PUBLISHED, OUTBOX_RETRY,
};
use crate::modules::events::repository::{
    EventInboxRepository, EventOutboxRepository, UserWalletInitializer,
};
use crate::modules::events::{
    InboxClaim, InboxRetryDecision, NewInboxMessage, NewOutboxEvent, OutboxInsertResult,
    OutboxMessage, PendingInboxRetry,
};

/// `EventOutboxRepository` 的 MySQL 适配器，把 trait 调用转发到本模块的自由函数实现。
/// 只持有连接池句柄，克隆代价等同于克隆池引用，因此可以随 worker 与应用状态自由复制。
#[derive(Debug, Clone)]
pub struct MySqlEventOutboxRepository {
    pool: Pool<MySql>,
}

impl MySqlEventOutboxRepository {
    /// 绑定承载 `event_outbox` 的 MySQL 连接池构造适配器。
    /// 构造过程不从池中取连接、不扫描待发消息、不开启事务，因此可在启动早期安全创建。
    /// 池句柄按引用共享，克隆本适配器不会额外占用数据库连接。
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventOutboxRepository for MySqlEventOutboxRepository {
    /// 以自动提交方式写入一条待发布事件，幂等键冲突时返回既有记录编号而非报错。
    /// 该路径不参与业务事务，若要求事件与业务写入原子生效，应改用 `insert_event_in_tx`。
    async fn insert_event(&self, event: NewOutboxEvent) -> AppResult<OutboxInsertResult> {
        insert_event(&self.pool, &event).await
    }

    /// 取一批到期可发布的事件，只做只读扫描而不占用或改写任何行。
    /// 因此多个发布实例可能取到同一批消息并重复投递，重复吸收依赖下游 inbox 去重。
    async fn fetch_publishable_batch(
        &self,
        limit: u32,
        now: DateTime<Utc>,
    ) -> AppResult<Vec<OutboxMessage>> {
        fetch_publishable_batch(&self.pool, limit, now).await
    }

    /// 把消息推进为已发布终态并记录发布时刻，使后续扫描不再选中该行。
    /// 该终态只代表本进程已把消息交给 broker，并不代表 broker 已确认持久化。
    async fn mark_published(&self, id: u64, published_at: DateTime<Utc>) -> AppResult<()> {
        mark_published(&self.pool, id, published_at).await
    }

    /// 记录一次可重试的发布失败，写入累计失败次数与下次到期时间，消息回到待发布集合等待退避后重投。
    /// 本方法只改状态，不会重新发送消息，重发由后续扫描轮次自然触发。
    async fn mark_retry(
        &self,
        id: u64,
        retry_count: u32,
        next_retry_at: DateTime<Utc>,
    ) -> AppResult<()> {
        mark_retry(&self.pool, id, retry_count, next_retry_at).await
    }

    /// 把重试次数耗尽的消息打入死信终态，从此不再被发布扫描选中。
    /// 不会触发告警也不会转投死信交换机，恢复必须走带管理员审计的显式重排。
    async fn mark_dead_letter(
        &self,
        id: u64,
        retry_count: u32,
        failed_at: DateTime<Utc>,
    ) -> AppResult<()> {
        mark_dead_letter(&self.pool, id, retry_count, failed_at).await
    }
}

/// `EventInboxRepository` 的 MySQL 适配器，把 trait 调用转发到本模块的自由函数实现。
/// 与 outbox 适配器的关键差别是这里的写操作都带处理令牌条件，用以保护消费租约不被旧 worker 覆盖。
#[derive(Debug, Clone)]
pub struct MySqlEventInboxRepository {
    pool: Pool<MySql>,
}

impl MySqlEventInboxRepository {
    /// 绑定承载 `event_inbox` 的 MySQL 连接池构造适配器。
    /// 构造过程不领取任何消息、不创建处理租约、不执行消费副作用，仅保存池句柄。
    /// 租约与状态推进只发生在具体的领取和标记方法中。
    pub fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventInboxRepository for MySqlEventInboxRepository {
    /// 扫描该消费者名下到期待重试以及租约超时的消息，用于 broker 已 ACK 后的本地补偿重放。
    /// 只做只读扫描不取得处理权，返回的消息仍须经 `claim_message` 重新竞争租约才能处理。
    async fn fetch_due_retries(
        &self,
        consumer_name: &str,
        limit: u32,
        now: DateTime<Utc>,
    ) -> AppResult<Vec<PendingInboxRetry>> {
        fetch_due_retries(&self.pool, consumer_name, limit, now).await
    }

    /// 竞争一条消息的处理权，成功时返回处理令牌，重复投递或终态消息返回重复标记。
    /// 这是 inbox 去重与并发互斥的唯一入口，处理消息前必须先经过它。
    async fn claim_message(&self, message: NewInboxMessage) -> AppResult<InboxClaim> {
        claim_message(&self.pool, message).await
    }

    /// 在业务处理成功后把消息推进为已消费终态，必须携带领取时拿到的处理令牌。
    /// 令牌失配说明租约已被接管，此时更新会被拒绝，从而避免旧 worker 覆盖新一轮处理结果。
    async fn mark_consumed(
        &self,
        consumer_name: &str,
        message_id: &str,
        processing_token: &str,
    ) -> AppResult<()> {
        mark_consumed(&self.pool, consumer_name, message_id, processing_token).await
    }

    /// 在业务处理失败后按重试策略把消息落为待重试或死信，并保存错误摘要与累计失败次数。
    /// 是重试还是死信由调用方传入的决策对象给出，本方法不自行判断阈值。
    /// 同样以处理令牌为更新条件，失配即拒绝；本方法不重执业务逻辑也不做 broker ACK。
    async fn mark_failure(
        &self,
        consumer_name: &str,
        message_id: &str,
        processing_token: &str,
        decision: InboxRetryDecision,
        error_message: &str,
        now: DateTime<Utc>,
    ) -> AppResult<()> {
        mark_failure(
            &self.pool,
            consumer_name,
            message_id,
            processing_token,
            decision,
            error_message,
            now,
        )
        .await
    }
}

/// 以自动提交方式把领域事件写入 outbox，初始状态为待发布。
/// 去重靠 `idempotency_key` 上的唯一约束：`ON DUPLICATE KEY UPDATE` 把该列赋回自身，
/// 使冲突时既不改动既有行也不报错，随后回查主键返回重复标记，调用方据此区分首次写入与重放。
/// 判定是否为新插入的依据是自增主键是否非零，冲突分支下 MySQL 不产生新主键。
/// 本函数不发布任何外部消息，投递由发布 worker 后续扫描完成；数据库错误原样上抛。
pub(crate) async fn insert_event(
    pool: &Pool<MySql>,
    event: &NewOutboxEvent,
) -> AppResult<OutboxInsertResult> {
    let result = sqlx::query(
        r#"INSERT INTO event_outbox
           (aggregate_type, aggregate_id, event_type, routing_key, idempotency_key, payload_json, status, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE idempotency_key = idempotency_key"#,
    )
    .bind(&event.aggregate_type)
    .bind(&event.aggregate_id)
    .bind(&event.event_type)
    .bind(&event.routing_key)
    .bind(&event.idempotency_key)
    .bind(SqlxJson(event.payload.clone()))
    .bind(OUTBOX_PENDING)
    .bind(event.created_at.naive_utc())
    .execute(pool)
    .await?;

    if result.last_insert_id() != 0 {
        return Ok(OutboxInsertResult::Inserted {
            id: result.last_insert_id(),
        });
    }

    let id = sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM event_outbox WHERE idempotency_key = ? LIMIT 1",
    )
    .bind(&event.idempotency_key)
    .fetch_one(pool)
    .await?
    .0;

    Ok(OutboxInsertResult::Duplicate { id })
}

/// 在调用方业务事务内写入 outbox 事件，这正是 outbox 模式的核心：业务数据与待发事件同生共死。
/// 事务回滚时事件记录随之消失，绝不会出现业务未成功却发出了事件的情况；
/// 反过来事务提交后事件必定已落库，即便进程随即崩溃，发布 worker 也会在重启后接着投递。
/// 去重与返回值语义同自动提交版本：幂等键冲突时不改既有行，回查主键并返回重复标记。
/// 本函数不提交事务，也不在提交前产生任何外部发布。
pub(crate) async fn insert_event_in_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    event: &NewOutboxEvent,
) -> AppResult<OutboxInsertResult> {
    let result = sqlx::query(
        r#"INSERT INTO event_outbox
           (aggregate_type, aggregate_id, event_type, routing_key, idempotency_key, payload_json, status, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE idempotency_key = idempotency_key"#,
    )
    .bind(&event.aggregate_type)
    .bind(&event.aggregate_id)
    .bind(&event.event_type)
    .bind(&event.routing_key)
    .bind(&event.idempotency_key)
    .bind(SqlxJson(event.payload.clone()))
    .bind(OUTBOX_PENDING)
    .bind(event.created_at.naive_utc())
    .execute(&mut **tx)
    .await?;

    if result.last_insert_id() != 0 {
        return Ok(OutboxInsertResult::Inserted {
            id: result.last_insert_id(),
        });
    }

    let id = sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM event_outbox WHERE idempotency_key = ? LIMIT 1",
    )
    .bind(&event.idempotency_key)
    .fetch_one(&mut **tx)
    .await?
    .0;

    Ok(OutboxInsertResult::Duplicate { id })
}

/// 在调用方事务内为指定用户补齐全部资产的钱包账户，用一条 `INSERT IGNORE ... SELECT` 批量完成。
/// `INSERT IGNORE` 让已存在的账户被静默跳过，因此该操作天然幂等，用户创建事件重放或并发执行都不会出错，
/// 也不会把已有余额重置为零。
/// 会为 `assets` 表中的每一条资产建账，不区分资产是否启用，新用户因此对所有币种都有可用账户。
/// 只建零余额账户，不写任何资金流水也不产生余额变动；错误交由调用方回滚。
pub(crate) async fn create_wallet_accounts_for_user_in_tx(
    tx: &mut Transaction<'_, MySql>,
    user_id: u64,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT IGNORE INTO wallet_accounts (user_id, asset_id, available, frozen, locked)
           SELECT ?, id, 0, 0, 0
           FROM assets"#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// 生产环境用户钱包初始化适配器；持有 MySQL 池并为每次事件建立独立原子事务。
#[derive(Debug, Clone)]
pub(crate) struct MySqlUserWalletInitializer {
    pool: Pool<MySql>,
}

impl MySqlUserWalletInitializer {
    /// 绑定 MySQL 池构造适配器；构造过程不访问数据库、不建连接，也不预创建任何账户。
    /// 真正的建账发生在每次事件处理调用初始化方法时。
    pub(crate) fn new(pool: Pool<MySql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserWalletInitializer for MySqlUserWalletInitializer {
    /// 在单事务内执行 `INSERT IGNORE ... SELECT assets`，确保用户事件重放不会重复创建钱包。
    /// 任一 SQL 或 commit 失败均返回错误并回滚；成功只产生缺失账户，不写余额流水或发布事件。
    async fn initialize_user_wallets(&self, user_id: u64) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        create_wallet_accounts_for_user_in_tx(&mut tx, user_id).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// 按 ID 升序读取至多 `limit` 条 `pending` 或 `next_retry_at <= now` 的 `retry` outbox，作为本轮可发布快照。
/// 两类消息共用一条查询：从未发过的待发布行，以及退避时间已到的重试行，`next_retry_at` 为空视为立即可发。
/// 按主键升序取批意味着单实例内大体遵循写入先后，但这不构成投递顺序保证：
/// 查询不加领取锁也不预改状态，多实例会读到同一行，失败重排又会打乱相对次序，消费方不得依赖到达顺序。
/// 重复发布的吸收责任落在 broker 的 message_id 与下游 inbox 去重上，而非本查询。
/// 返回结构中的失败次数经 `max(0)` 兜底再转无符号，防止库中出现负值时发生转换回绕。
pub(crate) async fn fetch_publishable_batch(
    pool: &Pool<MySql>,
    limit: u32,
    now: DateTime<Utc>,
) -> AppResult<Vec<OutboxMessage>> {
    type OutboxRow = (
        u64,
        String,
        String,
        String,
        String,
        String,
        SqlxJson<Value>,
        i32,
    );

    let rows = sqlx::query_as::<_, OutboxRow>(
        r#"SELECT id, aggregate_type, aggregate_id, event_type, routing_key, idempotency_key, payload_json, retry_count
           FROM event_outbox
           WHERE status IN ('pending', 'retry') AND (next_retry_at IS NULL OR next_retry_at <= ?)
           ORDER BY id ASC
           LIMIT ?"#,
    )
    .bind(now.naive_utc())
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                aggregate_type,
                aggregate_id,
                event_type,
                routing_key,
                idempotency_key,
                SqlxJson(payload),
                retry_count,
            )| OutboxMessage {
                id,
                aggregate_type,
                aggregate_id,
                event_type,
                routing_key,
                idempotency_key,
                payload,
                retry_count: retry_count.max(0) as u32,
            },
        )
        .collect())
}

/// 在 publisher 报告 `basic_publish` 完成后把指定 outbox 行标记为 `published`，同时记录该时间与更新时间。
/// 发布时刻与更新时刻绑定同一个入参而非各取一次当前时间，使两列严格一致便于按时间对账。
/// 更新不带前态条件，因此重复标记同一行是安全的幂等操作，但也意味着它会覆盖任何中间状态。
/// 既有的失败次数与下次重试时间不会被清空，从而保留该消息此前经历过多少次重试的痕迹。
/// 当前 publisher 未启用发布确认，调用方不得把该终态解释为 broker 已持久接收。
pub(crate) async fn mark_published(
    pool: &Pool<MySql>,
    id: u64,
    published_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE event_outbox SET status = ?, published_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(OUTBOX_PUBLISHED)
    .bind(published_at.naive_utc())
    .bind(published_at.naive_utc())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 把一次失败的发布持久化为 `retry`，写入失败次数与下次到期时间，使该行在退避期满后重新进入发布扫描。
/// 失败次数从无符号转有符号时做饱和处理，异常大的取值会被截到上限而不是回绕成负数，
/// 因为负数会让后续的死信阈值判断彻底失效。
/// 更新时刻取当前时间而非入参，与到期时间来自不同时钟基准，对账时不应假定两者一致。
/// 本函数只改状态不重发消息，重发由后续扫描轮次自然触发；
/// 更新失败时原行保持原态，由上层中止本轮并在下次扫描重新判断。
pub(crate) async fn mark_retry(
    pool: &Pool<MySql>,
    id: u64,
    retry_count: u32,
    next_retry_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE event_outbox SET status = ?, retry_count = ?, next_retry_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(OUTBOX_RETRY)
    .bind(i32::try_from(retry_count).unwrap_or(i32::MAX))
    .bind(next_retry_at.naive_utc())
    .bind(Utc::now().naive_utc())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 把重试预算耗尽的 outbox 行标记为 `dead_letter` 并保存最终失败次数，使普通发布扫描不再选中它。
/// 失败次数同样做饱和转换以防回绕；失败时刻被写入更新时刻列，因此该列即最后一次判定死信的时间。
/// 注意下次重试时间不会被清空，残留的旧值不影响判定，因为死信状态本身已不在扫描条件内。
/// 不转发到独立死信交换机，也不自动告警或重排；恢复只能经带管理员审计的显式重排用例完成。
pub(crate) async fn mark_dead_letter(
    pool: &Pool<MySql>,
    id: u64,
    retry_count: u32,
    failed_at: DateTime<Utc>,
) -> AppResult<()> {
    sqlx::query("UPDATE event_outbox SET status = ?, retry_count = ?, updated_at = ? WHERE id = ?")
        .bind(OUTBOX_DEAD_LETTER)
        .bind(i32::try_from(retry_count).unwrap_or(i32::MAX))
        .bind(failed_at.naive_utc())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 按 `consumer_name` 读取至多 `limit` 条到期 retry，或超过 300 秒租约的 processing 行，供 broker 确认之后的本地补偿重放。
/// 两类记录合并在一条查询里：退避已到期的待重试行，以及更新时刻早于租约起算点的处理中行，
/// 后者代表持有者已失联，其租约可被重新竞争。
/// 结果携带存档载荷，这是重放能够脱离原始投递独立成立的关键。
/// 排序取下次重试时刻，为空时回落到更新时刻，再以主键兜底保证顺序稳定。
/// 该查询不加锁也不改状态，因此多个实例会读到同一批记录，真正的互斥由后续的领取动作完成。
pub(crate) async fn fetch_due_retries(
    pool: &Pool<MySql>,
    consumer_name: &str,
    limit: u32,
    now: DateTime<Utc>,
) -> AppResult<Vec<PendingInboxRetry>> {
    let stale_processing_before =
        (now - TimeDelta::seconds(INBOX_PROCESSING_LEASE_SECONDS)).naive_utc();
    let rows = sqlx::query_as::<_, (String, String, SqlxJson<Value>)>(
        r#"SELECT message_id, idempotency_key, payload_json
           FROM event_inbox
           WHERE consumer_name = ?
             AND (
                (status = ? AND (next_retry_at IS NULL OR next_retry_at <= ?))
                OR (status = ? AND updated_at <= ?)
             )
           ORDER BY COALESCE(next_retry_at, updated_at) ASC, id ASC
           LIMIT ?"#,
    )
    .bind(consumer_name)
    .bind(INBOX_RETRY)
    .bind(now.naive_utc())
    .bind(INBOX_PROCESSING)
    .bind(stale_processing_before)
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(message_id, idempotency_key, SqlxJson(payload))| PendingInboxRetry {
                consumer_name: consumer_name.to_owned(),
                message_id,
                idempotency_key,
                payload,
            },
        )
        .collect())
}

/// 在独立事务内竞争一条 inbox 消息的处理权，是消费侧去重与并发互斥的唯一入口。
/// 先按消费者名加「message_id 或 idempotency_key」加锁查找既有行，两个去重键取并集，
/// 因此同一业务事件即便以不同 message_id 重复投递也会被识别为重复。
/// 命中既有行时交给纯决策函数判断能否重新领取，可领取才把状态改回 processing 并刷新时间戳。
/// 未命中则直接插入 processing 行；插入若撞上唯一键冲突，说明并发消费者刚插入同一条，
/// 此时改为加锁回查既有行并走与命中分支相同的决策，这条竞态补偿路径与主路径逻辑完全一致。
/// 处理令牌取自本次写入的微秒级 `updated_at`，后续状态推进都要拿它做乐观条件。
/// 全过程在一个事务内完成并提交，行锁在提交时释放；本函数只管所有权，不执行任何业务副作用。
pub(crate) async fn claim_message(
    pool: &Pool<MySql>,
    message: NewInboxMessage,
) -> AppResult<InboxClaim> {
    let mut tx = pool.begin().await?;
    let existing = sqlx::query_as::<
        _,
        (
            String,
            i32,
            String,
            Option<chrono::NaiveDateTime>,
            chrono::NaiveDateTime,
        ),
    >(
        r#"SELECT status, retry_count, message_id, CAST(next_retry_at AS DATETIME(6)), CAST(updated_at AS DATETIME(6))
           FROM event_inbox
           WHERE consumer_name = ? AND (message_id = ? OR idempotency_key = ?) LIMIT 1 FOR UPDATE"#,
    )
    .bind(&message.consumer_name)
    .bind(&message.message_id)
    .bind(&message.idempotency_key)
    .fetch_optional(&mut *tx)
    .await?;

    let claim = if let Some(existing) = existing {
        let existing = ExistingInboxMessage::from(existing);
        let claimed_at = Utc::now().naive_utc();
        let claim =
            decide_existing_inbox_claim(&message, existing.clone(), processing_token(claimed_at))?;
        if matches!(claim, InboxClaim::Claimed { .. }) {
            sqlx::query(
                "UPDATE event_inbox
                 SET status = ?, error_message = NULL, updated_at = ?
                 WHERE consumer_name = ? AND message_id = ?",
            )
            .bind(INBOX_PROCESSING)
            .bind(claimed_at)
            .bind(&message.consumer_name)
            .bind(&existing.message_id)
            .execute(&mut *tx)
            .await?;
        }
        claim
    } else {
        let claimed_at = Utc::now().naive_utc();
        let inserted = sqlx::query(
            r#"INSERT INTO event_inbox
               (consumer_name, message_id, idempotency_key, payload_hash, payload_json, status, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&message.consumer_name)
        .bind(&message.message_id)
        .bind(&message.idempotency_key)
        .bind(&message.payload_hash)
        .bind(SqlxJson(message.payload.clone()))
        .bind(INBOX_PROCESSING)
        .bind(claimed_at)
        .execute(&mut *tx)
        .await;

        match inserted {
            Ok(_) => InboxClaim::Claimed {
                attempt_count: 0,
                processing_token: processing_token(claimed_at),
            },
            Err(error) if is_unique_violation(&error) => {
                let existing = sqlx::query_as::<
                    _,
                    (
                        String,
                        i32,
                        String,
                        Option<chrono::NaiveDateTime>,
                        chrono::NaiveDateTime,
                    ),
                >(r#"SELECT status, retry_count, message_id, CAST(next_retry_at AS DATETIME(6)), CAST(updated_at AS DATETIME(6))
                   FROM event_inbox
                   WHERE consumer_name = ? AND (message_id = ? OR idempotency_key = ?) LIMIT 1 FOR UPDATE"#)
                .bind(&message.consumer_name)
                .bind(&message.message_id)
                .bind(&message.idempotency_key)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| {
                    AppError::Internal("event inbox unique conflict row was not found".to_owned())
                })?;
                let existing = ExistingInboxMessage::from(existing);
                let claimed_at = Utc::now().naive_utc();
                let claim = decide_existing_inbox_claim(
                    &message,
                    existing.clone(),
                    processing_token(claimed_at),
                )?;
                if matches!(claim, InboxClaim::Claimed { .. }) {
                    sqlx::query(
                        "UPDATE event_inbox
                         SET status = ?, error_message = NULL, updated_at = ?
                         WHERE consumer_name = ? AND message_id = ?",
                    )
                    .bind(INBOX_PROCESSING)
                    .bind(claimed_at)
                    .bind(&message.consumer_name)
                    .bind(&existing.message_id)
                    .execute(&mut *tx)
                    .await?;
                }
                claim
            }
            Err(error) => return Err(error.into()),
        }
    };

    tx.commit().await?;
    Ok(claim)
}

/// 仅当 consumer、message_id、`processing` 状态和微秒级处理令牌全部匹配时，把 inbox 原子推进为 `consumed`。
/// 四个条件缺一不可，其中令牌作为 `updated_at` 的等值条件构成乐观锁：
/// 若期间租约已被其他实例接管，该列已变，本次更新影响零行从而被拒。
/// 这一机制专门防止崩溃恢复后的旧 worker 把新持有者的处理结果覆盖成已消费。
/// 成功时清空错误摘要并同时写入消费时刻与更新时刻，两列取同一个时间值。
/// 影响零行返回陈旧令牌错误；本函数只提交 inbox 终态，不做 broker 确认也不触碰业务数据。
pub(crate) async fn mark_consumed(
    pool: &Pool<MySql>,
    consumer_name: &str,
    message_id: &str,
    processing_token: &str,
) -> AppResult<()> {
    let now = Utc::now().naive_utc();
    let processing_updated_at = parse_processing_token(processing_token)?;
    let result = sqlx::query(
        "UPDATE event_inbox SET status = ?, error_message = NULL, consumed_at = ?, updated_at = ? WHERE consumer_name = ? AND message_id = ? AND status = ? AND updated_at = ?",
    )
    .bind(INBOX_CONSUMED)
    .bind(now)
    .bind(now)
    .bind(consumer_name)
    .bind(message_id)
    .bind(INBOX_PROCESSING)
    .bind(processing_updated_at)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(processing_token_is_stale_error());
    }

    Ok(())
}

/// 以处理令牌为条件，把 handler 失败原子落为带到期时间的 `retry` 或无下次时间的 `dead_letter`，并保存错误摘要与失败次数。
/// 目标状态与到期时间由调用方传入的决策对象给出，本函数不自行判断阈值：
/// 重试分支写入下次到期时刻，死信分支把该列置空，因为死信不会再被扫描选中，留着排期只会产生误导。
/// 失败次数从无符号转有符号时做饱和处理，防止异常大值回绕成负数破坏后续阈值判断。
/// 错误摘要原样写入，供运维直接从记录判断失败原因，无需翻日志。
/// 与标记已消费一样以令牌作为乐观条件，影响零行即返回陈旧令牌错误，避免旧 worker 覆盖新租约。
/// 本入口只持久化消费状态，不重执业务、不重放消息、不做 broker 确认。
pub(crate) async fn mark_failure(
    pool: &Pool<MySql>,
    consumer_name: &str,
    message_id: &str,
    processing_token: &str,
    decision: InboxRetryDecision,
    error_message: &str,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let (status, attempt_count, next_retry_at) = match decision {
        InboxRetryDecision::Retry {
            attempt_count,
            next_retry_at,
        } => (INBOX_RETRY, attempt_count, Some(next_retry_at)),
        InboxRetryDecision::DeadLetter { attempt_count } => {
            (INBOX_DEAD_LETTER, attempt_count, None)
        }
    };

    let processing_updated_at = parse_processing_token(processing_token)?;
    let result = sqlx::query(
        "UPDATE event_inbox SET status = ?, error_message = ?, retry_count = ?, next_retry_at = ?, updated_at = ? WHERE consumer_name = ? AND message_id = ? AND status = ? AND updated_at = ?",
    )
    .bind(status)
    .bind(error_message)
    .bind(i32::try_from(attempt_count).unwrap_or(i32::MAX))
    .bind(next_retry_at.map(|value| value.naive_utc()))
    .bind(now.naive_utc())
    .bind(consumer_name)
    .bind(message_id)
    .bind(INBOX_PROCESSING)
    .bind(processing_updated_at)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(processing_token_is_stale_error());
    }

    Ok(())
}

/// 判断 SQLx 错误是否为 MySQL 1062 唯一键冲突，用于把并发插入 inbox 的竞态与真实故障区分开。
/// 只认错误码而不匹配消息文本，避免受数据库版本或语言设置影响；非数据库错误一律返回假。
/// 命中时调用方应转入加锁回查既有行的补偿路径，而不是把冲突当成失败上抛。
pub(crate) fn is_unique_violation(error: &SqlxError) -> bool {
    error
        .as_database_error()
        .and_then(DatabaseError::code)
        .as_deref()
        == Some("1062")
}

/// 判断重试消息是否尚未到达退避到期时间，为真表示应当继续等待而不是现在重放。
/// 库中时间按 UTC 无时区存储，比较前显式补上 UTC 时区；`None` 视为已到期，即没有排期就可以立刻重试。
pub(crate) fn retry_is_not_due(next_retry_at: Option<chrono::NaiveDateTime>) -> bool {
    next_retry_at.is_some_and(|value| value.and_utc() > Utc::now())
}

/// 判断处理中记录的租约是否已过期，即最后更新时刻加上租约时长是否已不晚于当前时间。
/// 为真只表示允许其他实例重新竞争该消息，本函数不转移所有权也不修改任何数据。
/// 库中时间无时区，比较前显式按 UTC 解释；若各实例时钟漂移较大，判定可能提前或延后。
pub(crate) fn processing_is_stale(updated_at: chrono::NaiveDateTime) -> bool {
    updated_at.and_utc() + TimeDelta::seconds(INBOX_PROCESSING_LEASE_SECONDS) <= Utc::now()
}

/// 把领取时写入的微秒级时间戳编码成处理令牌文本，后续所有状态推进都以它作为乐观租约条件。
/// 令牌之所以直接取自更新时刻而非另生成随机值，是为了让条件更新可以直接比对已有列，无需额外增列。
/// 编码格式必须与解析端严格一致，且精度不可降级，否则同一秒内的两次领取会产生相同令牌而失去互斥。
pub(crate) fn processing_token(value: chrono::NaiveDateTime) -> String {
    value.format(INBOX_PROCESSING_TOKEN_FORMAT).to_string()
}

/// 按微秒精度格式把处理令牌解析回时间戳，供状态更新语句作为乐观条件绑定。
/// 解析失败不区分具体原因，一律折算成陈旧租约错误，从而使伪造或损坏的令牌无法更新任何 inbox 行。
/// 精度必须保持微秒级，截断到秒会让同一秒内的两次领取产生相同令牌而失去互斥效果。
pub(crate) fn parse_processing_token(value: &str) -> AppResult<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(value, INBOX_PROCESSING_TOKEN_FORMAT)
        .map_err(|_| processing_token_is_stale_error())
}

/// 构造统一的「处理令牌已陈旧」错误，令牌解析失败与条件更新影响零行两种情形共用它。
/// 两者对调用方的含义相同：本 worker 已失去该消息的处理权，应放弃后续状态推进并让新持有者接手。
/// 归为内部错误而非校验错误，因为这属于并发协调结果而不是外部输入问题。
pub(crate) fn processing_token_is_stale_error() -> AppError {
    AppError::Internal("event inbox processing token is stale".to_owned())
}

/// 加锁读到的既有 inbox 行快照，是领取决策的全部输入。
/// 单独建模而不直接传元组，是为了让纯决策函数可以脱离数据库单独测试。
#[derive(Debug, Clone)]
pub(crate) struct ExistingInboxMessage {
    /// 当前状态，取值为 processing、consumed、retry 或 dead_letter。
    pub status: String,
    /// 已累计的失败次数，库中理论上非负，决策时仍会做下界兜底。
    pub retry_count: i32,
    /// 该行原有的 message_id；与本次投递不一致说明是同一业务事件的另一条消息。
    pub message_id: String,
    /// 下次可重试时刻，为空表示没有排期即可立即重试。
    pub next_retry_at: Option<chrono::NaiveDateTime>,
    /// 最后一次状态推进时刻，既是租约起算点也是处理令牌的取值来源。
    pub updated_at: chrono::NaiveDateTime,
}

impl
    From<(
        String,
        i32,
        String,
        Option<chrono::NaiveDateTime>,
        chrono::NaiveDateTime,
    )> for ExistingInboxMessage
{
    /// 把加锁查询返回的五元组按位置映射成具名字段，两处查询语句共用这一转换。
    /// 元组顺序与 SELECT 列顺序严格对应，依次为状态、失败次数、消息号、下次重试时刻和更新时刻；
    /// 调整任一 SELECT 的列顺序都必须同步改这里，否则字段会被静默错位赋值。
    /// 纯搬运，不做默认值填充也不做取值校验。
    fn from(
        value: (
            String,
            i32,
            String,
            Option<chrono::NaiveDateTime>,
            chrono::NaiveDateTime,
        ),
    ) -> Self {
        Self {
            status: value.0,
            retry_count: value.1,
            message_id: value.2,
            next_retry_at: value.3,
            updated_at: value.4,
        }
    }
}

/// 对已锁定的 inbox 行判断本次能否重新领取，是消费侧并发控制的核心决策，且不含任何 I/O。
/// 待重试状态下，只有消息标识与本次投递一致且退避已到期才允许领取，
/// 标识不一致说明这是同一业务事件的另一条投递，按重复处理以免绕过退避。
/// 处理中状态下，标识不一致同样判重复；标识一致时再看租约是否超时，
/// 超时才允许接管，未超时则返回明确错误以阻止两个处理器同时执行同一条消息。
/// 已消费与死信这两个终态一律判为重复，不再有任何重放机会。
/// 允许领取时沿用既有失败次数作为退避基数，并做下界兜底防止库中负值影响计算。
/// 本函数只给结论不改数据，真正的状态写入与所有权转移由外层事务完成。
pub(crate) fn decide_existing_inbox_claim(
    message: &NewInboxMessage,
    existing: ExistingInboxMessage,
    processing_token: String,
) -> AppResult<InboxClaim> {
    if existing.status == INBOX_RETRY {
        if existing.message_id != message.message_id || retry_is_not_due(existing.next_retry_at) {
            Ok(InboxClaim::Duplicate)
        } else {
            Ok(InboxClaim::Claimed {
                attempt_count: existing.retry_count.max(0) as u32,
                processing_token,
            })
        }
    } else if existing.status == INBOX_PROCESSING {
        if existing.message_id != message.message_id {
            Ok(InboxClaim::Duplicate)
        } else if processing_is_stale(existing.updated_at) {
            Ok(InboxClaim::Claimed {
                attempt_count: existing.retry_count.max(0) as u32,
                processing_token,
            })
        } else {
            Err(AppError::Internal(
                "event inbox message is already processing".to_owned(),
            ))
        }
    } else {
        Ok(InboxClaim::Duplicate)
    }
}

#[derive(Debug, Clone, Copy)]
/// 持久化层的事件记录筛选参数，值已由表现层完成边界归一化。
/// outbox 与 inbox 两个运维查询共用该结构，因此状态取值的合法集合取决于查的是哪张表。
pub(crate) struct EventRecordListFilter<'a> {
    /// 按状态精确筛选，`None` 表示不限状态返回全部记录。
    pub(crate) status: Option<&'a str>,
    /// 单页条数，已在表现层夹取到合理范围。
    pub(crate) limit: u32,
    /// 分页偏移，已在表现层截断。
    pub(crate) offset: u32,
}

#[derive(Debug, sqlx::FromRow)]
/// outbox SQL 行模型，只在 infrastructure/application 边界内流转。
/// 刻意不含事件载荷，运维列表只需状态与路由信息，避免把业务数据带进面板和日志。
pub(crate) struct OutboxRecordRow {
    /// 事件主键，也是死信重排接口的定位标识。
    pub(crate) id: u64,
    /// 聚合类型，标明该事件属于哪类业务对象。
    pub(crate) aggregate_type: String,
    /// 聚合实例标识，与聚合类型共同定位事件来源。
    pub(crate) aggregate_id: String,
    /// 事件类型名，消费方据此分派处理逻辑。
    pub(crate) event_type: String,
    /// 消息路由键，决定 broker 把消息投给哪些队列。
    pub(crate) routing_key: String,
    /// 发布状态，取值为 pending、retry、published 或 dead_letter。
    pub(crate) status: String,
    /// 已累计的发布失败次数。
    pub(crate) retry_count: i32,
    /// 下次可发布时刻，为空表示立即可发；只在重试状态下有意义。
    pub(crate) next_retry_at: Option<DateTime<Utc>>,
    /// 发布完成时刻，未发布成功时为空。
    pub(crate) published_at: Option<DateTime<Utc>>,
    /// 事件写入时刻，与发布时刻之差即端到端投递延迟。
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
/// inbox SQL 行模型，只在 infrastructure/application 边界内流转。
/// 同样不含消息载荷与处理令牌，前者体量大，后者是并发控制凭据不应出现在运维列表里。
pub(crate) struct InboxRecordRow {
    /// 记录主键。
    pub(crate) id: u64,
    /// 消费者名称，同一条消息可被多个消费者各自独立消费和去重。
    pub(crate) consumer_name: String,
    /// 消息标识，与消费者名共同构成主要去重键。
    pub(crate) message_id: String,
    /// 消费状态，取值为 processing、consumed、retry 或 dead_letter。
    pub(crate) status: String,
    /// 已累计的消费失败次数。
    pub(crate) retry_count: i32,
    /// 最近一次失败的错误摘要，成功消费时会被清空。
    pub(crate) error_message: Option<String>,
    /// 消费完成时刻，未进入终态时为空。
    pub(crate) consumed_at: Option<DateTime<Utc>>,
    /// 消息首次落入 inbox 的时刻。
    pub(crate) created_at: DateTime<Utc>,
}

/// 为运维面板分页读取 outbox 记录与匹配总数，用于观察积压与死信情况。
/// 状态筛选用 `(? IS NULL OR status = ?)` 表达，同一个值绑定两次，
/// 这样传空即返回全部状态，无需为「有筛选」和「无筛选」维护两条 SQL，也保证行查询与计数口径一致。
/// 按主键倒序排列，最新事件排在前面；条数与偏移由调用方在表现层归一后传入。
/// 行查询与计数分两次执行，并发写入时总数与行集可能短暂不一致；任一查询失败即整体返回错误，
/// 不会返回只有行没有总数的半截结果。
pub(crate) async fn list_outbox_records(
    pool: &Pool<MySql>,
    filter: EventRecordListFilter<'_>,
) -> AppResult<(Vec<OutboxRecordRow>, i64)> {
    let rows = sqlx::query_as::<_, OutboxRecordRow>(
        r#"SELECT id, aggregate_type, aggregate_id, event_type, routing_key, status,
                  retry_count, next_retry_at, published_at, created_at
           FROM event_outbox
           WHERE (? IS NULL OR status = ?)
           ORDER BY id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(filter.status)
    .bind(filter.status)
    .bind(i64::from(filter.limit))
    .bind(i64::from(filter.offset))
    .fetch_all(pool)
    .await?;
    let (total,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM event_outbox WHERE (? IS NULL OR status = ?)")
            .bind(filter.status)
            .bind(filter.status)
            .fetch_one(pool)
            .await?;

    Ok((rows, total))
}

/// 为运维面板分页读取 inbox 记录与匹配总数，用于观察消费失败与死信堆积。
/// 与 outbox 版本采用相同的可空状态筛选写法，但返回列不同：这里带消费者名、错误摘要和消费完成时刻，
/// 便于直接定位是哪个消费者、因为什么原因反复失败。
/// 需要注意筛选维度只有状态，消费者名虽在返回列中却不参与过滤条件。
/// 按主键倒序排列。本函数纯读取，不领取消息、不推进重试、不改动任何状态。
pub(crate) async fn list_inbox_records(
    pool: &Pool<MySql>,
    filter: EventRecordListFilter<'_>,
) -> AppResult<(Vec<InboxRecordRow>, i64)> {
    let rows = sqlx::query_as::<_, InboxRecordRow>(
        r#"SELECT id, consumer_name, message_id, status, retry_count, error_message,
                  consumed_at, created_at
           FROM event_inbox
           WHERE (? IS NULL OR status = ?)
           ORDER BY id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(filter.status)
    .bind(filter.status)
    .bind(i64::from(filter.limit))
    .bind(i64::from(filter.offset))
    .fetch_all(pool)
    .await?;
    let (total,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM event_inbox WHERE (? IS NULL OR status = ?)")
            .bind(filter.status)
            .bind(filter.status)
            .fetch_one(pool)
            .await?;

    Ok((rows, total))
}

/// 在调用方事务中把一条死信事件重新投入待发布队列，并同步写入管理员审计。
///
/// 先以 `FOR UPDATE` 锁定目标行再判状态，避免两名管理员并发重排同一条死信导致重复审计。
/// 只有处于 `dead_letter` 的记录可被重排，其余状态返回 `AppError::Conflict`，
/// 因此对同一条记录重复点击重排时，第二次会被拒绝且不会新增审计记录。
/// 重排会把状态改回待发布、失败次数清零、下次重试时间清空，相当于让该事件从头开始一轮完整的重试预算。
/// 返回的是按预期变更推导出的后置快照，而非重新查库的结果。
/// 本函数不提交事务，审计与状态变更必须由调用方一并提交，且不会直接向 broker 投递消息。
pub(crate) async fn requeue_outbox_dead_letter_in_tx(
    tx: &mut Transaction<'_, MySql>,
    admin_id: u64,
    id: u64,
    reason: &str,
) -> AppResult<OutboxRecordRow> {
    let before = sqlx::query_as::<_, OutboxRecordRow>(
        r#"SELECT id, aggregate_type, aggregate_id, event_type, routing_key, status,
                  retry_count, next_retry_at, published_at, created_at
           FROM event_outbox WHERE id = ? FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if before.status != OUTBOX_DEAD_LETTER {
        return Err(AppError::Conflict(
            "only dead-lettered outbox events can be requeued".to_owned(),
        ));
    }

    sqlx::query(
        "UPDATE event_outbox SET status = ?, retry_count = 0, next_retry_at = NULL WHERE id = ?",
    )
    .bind(OUTBOX_PENDING)
    .bind(id)
    .execute(&mut **tx)
    .await?;
    insert_event_admin_audit_log_in_tx(
        tx,
        admin_id,
        "event_outbox.requeue",
        "event_outbox",
        &id.to_string(),
        &before,
        reason,
    )
    .await?;
    let after = OutboxRecordRow {
        status: OUTBOX_PENDING.to_owned(),
        retry_count: 0,
        next_retry_at: None,
        ..before
    };
    Ok(after)
}

/// 为死信重排写一条管理员审计记录，与状态变更处于同一事务，确保运维动作不会无痕发生。
/// 前后镜像只保留状态与失败次数两项，因为重排恰好只改这两个维度加下次重试时间，
/// 记录完整事件体既冗余又可能把业务负载复制进审计表。
/// 后置镜像由重排的固定语义直接写死为待发布与零次失败，而不是回查数据库，
/// 因此它表达的是本次操作的意图；若同事务后续回滚，审计与状态会一并撤销，二者不会失配。
async fn insert_event_admin_audit_log_in_tx(
    tx: &mut sqlx::Transaction<'_, MySql>,
    admin_id: u64,
    action: &str,
    target_type: &str,
    target_id: &str,
    before: &OutboxRecordRow,
    reason: &str,
) -> AppResult<()> {
    let request_context = crate::infra::admin_request_context::current_admin_request_context();
    sqlx::query(
        r#"INSERT INTO admin_audit_logs
           (admin_id, action, target_type, target_id, before_json, after_json, reason, ip, request_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(SqlxJson(serde_json::json!({
        "status": before.status,
        "retry_count": before.retry_count,
    })))
    .bind(SqlxJson(serde_json::json!({
        "status": OUTBOX_PENDING,
        "retry_count": 0,
    })))
    .bind(reason)
    .bind(
        request_context
            .as_ref()
            .and_then(|context| context.source_ip.as_deref()),
    )
    .bind(
        request_context
            .as_ref()
            .map(|context| context.request_id.as_str()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
