//! 平台对账只读传输合同；单币种金额保留十进制文本，历史完整性永不推断。

use serde::{Deserialize, Serialize};

pub(crate) mod snapshots;

/// 资产目录分页与所选资产独立；未选择时只返回目录，不默认替管理员选择币种。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FinancialReconciliationQuery {
    pub asset_id: Option<u64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// 资产身份和精度来自数据库，包括已停用但仍可能存在负债的资产。
#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub(crate) struct ReconciliationAsset {
    pub asset_id: u64,
    pub symbol: String,
    pub precision_scale: i32,
}

/// 三个桶分别比对，不把冻结和锁定重复叠加成额外负债。
#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub(crate) struct WalletEvidence {
    pub account_type: String,
    pub wallet_count: i64,
    pub missing_ledger_count: i64,
    pub missing_wallet_count: i64,
    pub mismatch_count: i64,
    pub available: String,
    pub frozen: String,
    pub locked: String,
    pub comparable_available_delta: String,
    pub comparable_frozen_delta: String,
    pub comparable_locked_delta: String,
}

/// 缺少任一证据侧时对应金额为 null；不将缺失快照替换成零。
#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub(crate) struct WalletDifference {
    pub account_type: String,
    pub user_id: u64,
    pub issue: String,
    pub ledger_id: Option<u64>,
    pub available: Option<String>,
    pub frozen: Option<String>,
    pub locked: Option<String>,
    pub available_after: Option<String>,
    pub frozen_after: Option<String>,
    pub locked_after: Option<String>,
}

/// 每个 transaction_key 在所选资产内独立求和；相反差额不能互相抵消。
#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub(crate) struct JournalDifference {
    pub transaction_key: String,
    pub entry_count: i64,
    pub net_amount: String,
}

/// 原始科目净变动仅描述已有分录，不表示期初余额或实际托管库存。
#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub(crate) struct JournalMovement {
    pub context: String,
    pub account_code: String,
    pub entry_count: i64,
    pub net_movement: String,
}

/// 观察到的分录起止时间不是业务覆盖起点；零分录也不能证明零活动。
#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub(crate) struct JournalEvidence {
    pub entry_count: i64,
    pub transaction_count: i64,
    pub imbalanced_transaction_count: i64,
    pub first_entry_at: Option<i64>,
    pub last_entry_at: Option<i64>,
}

/// 来自当前业务行的独立指标；kind 明确本金、冻结、条件赔付等不同语义，禁止总计。
#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub(crate) struct OpenObligation {
    pub kind: String,
    pub record_count: i64,
    pub amount: String,
}

/// 差异明细有界返回且带全量计数；coverage 固定 partial，不提供偿付能力结论。
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct FinancialReconciliationReport {
    pub asset: ReconciliationAsset,
    pub coverage: &'static str,
    pub limitations: Vec<&'static str>,
    pub journal: JournalEvidence,
    pub journal_differences: Vec<JournalDifference>,
    pub journal_movements: Vec<JournalMovement>,
    pub journal_movement_count: i64,
    pub wallets: Vec<WalletEvidence>,
    pub wallet_differences: Vec<WalletDifference>,
    pub wallet_difference_count: i64,
    pub obligations: Vec<OpenObligation>,
    pub detail_limit: u32,
}

/// 同一只读一致快照包含目录、选定资产证据与服务端读取时间。
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct FinancialReconciliationResponse {
    pub assets: Vec<ReconciliationAsset>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
    pub report: Option<FinancialReconciliationReport>,
    pub checked_at: i64,
}
