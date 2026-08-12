//! inbox/outbox 共用的纯重试与死信决策。

use chrono::{DateTime, TimeDelta, Utc};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryMetadata {
    max_attempts: u32,
    attempt_count: u32,
    backoff: TimeDelta,
    next_attempt_at: Option<DateTime<Utc>>,
}

impl RetryMetadata {
    /// 创建从零开始的重试元数据；最大次数和退避必须为正。
    /// 纯状态构造，无时钟读取和副作用；非法策略立即返回配置错误。
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

    /// 返回已记录失败次数；只读访问，不推进重试或产生副作用。
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// 返回最近失败派生的下次时间；未先记录失败属于调用方违约并会 panic。
    /// 只读访问，不读取系统时钟、不调度任务。
    pub fn next_attempt_at(&self) -> DateTime<Utc> {
        self.next_attempt_at
            .expect("next_attempt_at is set after a recorded failure")
    }

    /// 判断累计失败是否达到死信阈值；该纯判断不写状态、不移动消息。
    pub fn should_dead_letter(&self) -> bool {
        self.attempt_count >= self.max_attempts
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetryMetadataError {
    #[error("retry max attempts must be positive")]
    InvalidMaxAttempts,
    #[error("retry backoff must be positive")]
    InvalidBackoff,
    #[error("retry attempt counter overflowed")]
    AttemptOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRetryPolicy {
    max_attempts: u32,
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
    /// 该纯函数不写仓储、不 ACK 消息；计数溢出返回错误，时间只用于计算下次执行点。
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxRetryDecision {
    Retry {
        attempt_count: u32,
        next_retry_at: DateTime<Utc>,
    },
    DeadLetter {
        attempt_count: u32,
    },
}
