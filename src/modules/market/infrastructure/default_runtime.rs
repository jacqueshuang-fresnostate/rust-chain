//! 平台交易对生成器的跨实例互斥；人工策略和默认行情共用同一锁，管理员整体暂停等待在途发布完成。

use sha2::{Digest, Sha256};
use sqlx::{MySql, MySqlConnection, Pool};
use std::sync::{Arc, LazyLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

static PAIR_LOCK_SLOTS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(8)));

use crate::error::{AppError, AppResult};

mod state;
pub(crate) use state::*;
mod reference;
pub(crate) use reference::*;

/// 持有脱离主池计数的专用 MySQL 连接与有界并发名额；连接不回池，取消或异常退出也由连接关闭释放命名锁。
/// 锁覆盖归档、缓存、推送和检查点，避免仅锁配置事务而让两种生成器同时执行下游副作用。
pub struct PairGenerationLock {
    connection: MySqlConnection,
    _permit: OwnedSemaphorePermit,
    name: String,
    connection_id: u64,
}

impl PairGenerationLock {
    /// 按数据库和交易对取得命名锁；worker 使用零等待，管理员使用有界等待。
    /// 超时返回 None 且不修改配置；锁连接从主池分离且总量有界，避免事务等待池容量或带锁连接泄漏回池。
    pub async fn acquire(
        pool: &Pool<MySql>,
        pair_id: u64,
        wait_seconds: u32,
    ) -> AppResult<Option<Self>> {
        let permit = if wait_seconds == 0 {
            match PAIR_LOCK_SLOTS.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => return Ok(None),
            }
        } else {
            match tokio::time::timeout(
                std::time::Duration::from_secs(u64::from(wait_seconds.min(5))),
                PAIR_LOCK_SLOTS.clone().acquire_owned(),
            )
            .await
            {
                Ok(Ok(permit)) => permit,
                _ => return Ok(None),
            }
        };
        // 脱离业务池的计数，避免多名持锁者耗尽主池后同时等待第二条事务连接；并发锁连接另有总量限制。
        let mut connection = pool.acquire().await?.detach();
        let (database, connection_id) =
            sqlx::query_as::<_, (String, u64)>("SELECT DATABASE(), CONNECTION_ID()")
                .fetch_one(&mut connection)
                .await?;
        let hash = hex::encode(Sha256::digest(format!("{database}:{pair_id}")));
        let name = format!("market-pair:{}", &hash[..48]);
        let acquired = sqlx::query_scalar::<_, Option<i32>>("SELECT GET_LOCK(?, ?)")
            .bind(&name)
            .bind(wait_seconds.min(5))
            .fetch_one(&mut connection)
            .await?;
        match acquired {
            Some(1) => Ok(Some(Self {
                connection,
                _permit: permit,
                name,
                connection_id,
            })),
            Some(0) => Ok(None),
            _ => Err(AppError::Internal(
                "synthetic pair lock could not be acquired".into(),
            )),
        }
    }

    /// 在发布阶段之间验证锁连接仍然存活且所有者未变；失败立即结束本轮，不以续租掩盖已丢失的互斥。
    pub async fn ensure_owned(&mut self) -> AppResult<()> {
        let owner = sqlx::query_scalar::<_, Option<u64>>("SELECT IS_USED_LOCK(?)")
            .bind(&self.name)
            .fetch_one(&mut self.connection)
            .await?;
        if owner != Some(self.connection_id) {
            return Err(AppError::Conflict(
                "synthetic pair generation ownership changed".into(),
            ));
        }
        Ok(())
    }

    /// 返回归档事务可再次核对的锁名；数据库名称参与哈希，隔离同实例上的不同测试/业务库。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回 MySQL 连接身份，归档事务使用 IS_USED_LOCK 校验而非仅信任 worker 自报 owner。
    pub fn connection_id(&self) -> u64 {
        self.connection_id
    }

    /// 在正常完成后显式释放锁；提前退出由专用连接关闭清理，绝不把持锁连接放回连接池。
    pub async fn release(mut self) -> AppResult<()> {
        let released = sqlx::query_scalar::<_, Option<i32>>("SELECT RELEASE_LOCK(?)")
            .bind(&self.name)
            .fetch_one(&mut self.connection)
            .await?;
        if released != Some(1) {
            return Err(AppError::Conflict(
                "synthetic pair generation lock was lost".into(),
            ));
        }
        Ok(())
    }
}
