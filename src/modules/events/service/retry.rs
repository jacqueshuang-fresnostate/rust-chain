//! inbox/outbox 共用的纯重试与死信决策。
//!
//! 退避曲线是等距的固定间隔而非指数增长：每次失败都把下次执行点推到「失败时刻加同一个退避量」，
//! 因此第 n 次重试与第 n+1 次之间的间隔恒定。这个取舍适合以瞬时故障为主的投递场景，
//! 代价是持续性故障不会自动拉长间隔，只能靠最大次数封顶。
//! 达到最大次数即判死信，之后不再自动重试。
//! 本文件不读系统时钟，失败时刻一律由调用方传入，使决策可复现也便于测试。
//! 两个类型对应两种用法：`RetryMetadata` 持有并推进完整状态，`InboxRetryPolicy` 只持策略、
//! 每次从外部传入当前次数做无状态判定，后者更适合次数已持久化在数据库里的场景。

use chrono::{DateTime, TimeDelta, Utc};
use thiserror::Error;

/// 携带完整重试状态的值对象，每次记录失败都派生出新实例而不原地修改。
/// 适用于在内存中连续推进重试的场景；次数已落库时应改用只持策略的无状态版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryMetadata {
    /// 允许的最大失败次数，达到即判死信，构造时要求为正。
    max_attempts: u32,
    /// 已累计的失败次数，从零开始。
    attempt_count: u32,
    /// 每次失败后的固定退避量，构造时要求为正。
    backoff: TimeDelta,
    /// 下次可执行时刻，尚未记录过失败时为空。
    next_attempt_at: Option<DateTime<Utc>>,
}

impl RetryMetadata {
    /// 创建从零开始的重试元数据；最大次数和退避必须为正。
    /// 最大次数为零会让消息一失败就死信从而失去重试意义，退避非正则会使重试立即到期退化为忙等，
    /// 两者都在构造阶段直接拒绝而不做静默纠正，避免非法策略被带进运行期。
    /// 初始失败次数为零、下次执行时刻为空，只有记录过失败之后才会有排期。
    /// 纯状态构造，不读系统时钟也不产生任何副作用。
    pub fn new(max_attempts: u32, backoff: TimeDelta) -> Result<Self, RetryMetadataError> {
        if max_attempts == 0 {
            return Err(RetryMetadataError::InvalidMaxAttempts);
        }
        if backoff <= TimeDelta::zero() {
            return Err(RetryMetadataError::InvalidBackoff);
        }

        Ok(Self {
            max_attempts,
            attempt_count: 0,
            backoff,
            next_attempt_at: None,
        })
    }

    /// 以给定失败时间派生下一版重试状态，不修改原值。
    /// 计数溢出失败；调用方负责持久化，新状态自身不触发调度或重放。
    pub fn record_failure(&self, failed_at: DateTime<Utc>) -> Result<Self, RetryMetadataError> {
        let attempt_count = self
            .attempt_count
            .checked_add(1)
            .ok_or(RetryMetadataError::AttemptOverflow)?;

        Ok(Self {
            max_attempts: self.max_attempts,
            attempt_count,
            backoff: self.backoff,
            next_attempt_at: Some(failed_at + self.backoff),
        })
    }

    /// 返回累计失败次数，用于持久化或与最大次数比较。
    /// 只读访问，不推进重试也不产生任何副作用；新建实例返回零。
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// 返回最近失败派生的下次时间；未先记录失败属于调用方违约并会 panic。
    /// 只读访问，不读取系统时钟、不调度任务。
    pub fn next_attempt_at(&self) -> DateTime<Utc> {
        self.next_attempt_at
            .expect("next_attempt_at is set after a recorded failure")
    }

    /// 判断累计失败次数是否已达最大次数，为真表示应转入死信而不再排下一次重试。
    /// 用不小于而非等于比较，使持久化数据异常偏大时仍能正确收敛到死信。
    /// 纯判断，不写状态也不移动消息。
    pub fn should_dead_letter(&self) -> bool {
        self.attempt_count >= self.max_attempts
    }
}

/// 重试策略的构造与推进错误，前两项属于配置非法，最后一项属于运行期越界。
/// 上层通常把它们统一转成内部错误，因为三者都不该由外部输入触发。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetryMetadataError {
    /// 最大次数为零，会导致消息一失败就死信且永无重试机会。
    #[error("retry max attempts must be positive")]
    InvalidMaxAttempts,
    /// 退避量非正，会让重试立即到期从而退化为忙等重放。
    #[error("retry backoff must be positive")]
    InvalidBackoff,
    /// 失败计数自增溢出，通常意味着持久化的次数已被异常写坏。
    #[error("retry attempt counter overflowed")]
    AttemptOverflow,
}

/// 无状态的重试策略，只保存阈值与退避量，每次判定都从外部接收当前失败次数。
/// outbox 与 inbox 共用同一实现，保证两侧的退避曲线与死信阈值完全一致。
/// 由于不持有计数，同一实例可被并发共享用于任意多条消息的判定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRetryPolicy {
    /// 允许的最大失败次数，达到即判死信。
    max_attempts: u32,
    /// 每次失败后的固定退避量。
    backoff: TimeDelta,
}

impl InboxRetryPolicy {
    /// 创建 inbox/outbox 统一重试策略；最大次数和固定退避必须为正。
    /// 策略本身无 I/O，失败时返回配置错误，调用方不得静默使用非法默认值。
    pub fn new(max_attempts: u32, backoff: TimeDelta) -> Result<Self, RetryMetadataError> {
        if max_attempts == 0 {
            return Err(RetryMetadataError::InvalidMaxAttempts);
        }
        if backoff <= TimeDelta::zero() {
            return Err(RetryMetadataError::InvalidBackoff);
        }

        Ok(Self {
            max_attempts,
            backoff,
        })
    }

    /// 根据当前持久化尝试次数决定下次重试或死信；达到阈值即进入 DeadLetter。
    /// 失败次数由调用方从数据库读入而非本对象持有，因此同一策略实例可并发服务任意多条消息。
    /// 先把传入次数加一得到本次失败后的累计值，再与最大次数比较，用不小于而非等于比较，
    /// 使库中次数异常偏大时仍能正确收敛到死信而不是继续无限重试。
    /// 下次执行时刻为传入失败时刻加固定退避量，因此退避是等距的而非指数增长。
    /// 该纯函数不写仓储、不确认消息，只返回决定；计数溢出返回错误。
    pub fn record_failure(
        &self,
        current_attempt_count: u32,
        failed_at: DateTime<Utc>,
    ) -> Result<InboxRetryDecision, RetryMetadataError> {
        let attempt_count = current_attempt_count
            .checked_add(1)
            .ok_or(RetryMetadataError::AttemptOverflow)?;

        if attempt_count >= self.max_attempts {
            Ok(InboxRetryDecision::DeadLetter { attempt_count })
        } else {
            Ok(InboxRetryDecision::Retry {
                attempt_count,
                next_retry_at: failed_at + self.backoff,
            })
        }
    }
}

/// 一次失败后的处置决定，两个分支互斥且都携带自增后的失败次数供落库。
/// 决定本身不产生任何效果，必须由调用方写入 outbox 或 inbox 才真正生效。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxRetryDecision {
    /// 仍有重试预算，按固定退避排定下次执行时刻。
    Retry {
        /// 自增后的累计失败次数。
        attempt_count: u32,
        /// 下次可执行时刻，等于本次失败时刻加固定退避量。
        next_retry_at: DateTime<Utc>,
    },
    /// 预算耗尽，转入死信且不再排定下次执行时刻。
    DeadLetter {
        /// 最终失败次数。
        attempt_count: u32,
    },
}
