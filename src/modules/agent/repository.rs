//! agent bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//! 当前文件先作为 DDD 迁移锚点，后续把对应职责的仓储契约逐步迁入。
//! 这里只放代理分销的行记录与写入入参结构，不含任何 SQL 与业务判断，
//! 供基础设施层做查询映射、应用层做参数传递，读写两侧因此共享同一套字段口径。

use bigdecimal::BigDecimal;

/// 代理可见范围，由服务端按登录管理员查出，是所有子树查询的边界依据。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AgentAccessScope {
    pub(crate) agent_id: u64,
    /// 所属顶级代理，自身即顶级时回落为自己的主键。
    pub(crate) root_agent_id: u64,
    /// 物化路径，参与等值比较与带分隔符的 LIKE 前缀匹配。
    pub(crate) path: String,
}

/// 改密流程中锁定读出的代理管理员凭证快照，仅含比对与状态判定所需的两列。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AgentAdminCredentialRecord {
    pub(crate) password_hash: String,
    pub(crate) status: String,
}

/// 看板的两项子查询计数，团队人数覆盖整棵子树，邀请码数只统计本级启用中的码。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AgentDashboardCountsRecord {
    pub(crate) team_user_count: i64,
    pub(crate) active_invite_code_count: i64,
}

/// 代理列表查询的分页参数，取值已由服务层按各用例上限收敛后才允许拼入 SQL。
#[derive(Debug, Clone, Copy)]
pub(crate) struct AgentListPage {
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

/// 子树闪兑聚合结果，状态计数由 SUM 得出因而是十进制类型，需由服务层转回整数。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AgentConvertStatsRecord {
    pub(crate) agent_id: u64,
    pub(crate) total_orders: i64,
    pub(crate) pending_orders: BigDecimal,
    pub(crate) completed_orders: BigDecimal,
    /// 转出金额合计，跨币种直接相加，仅供量级观察不可当作对账口径。
    pub(crate) total_from_amount: BigDecimal,
    pub(crate) total_to_amount: BigDecimal,
}

/// 新建代理邀请码的写入入参，码文本由服务端生成，使用上限为空表示不限次数。
#[derive(Debug, Clone)]
pub(crate) struct AgentInviteCodeWrite {
    pub(crate) agent_id: u64,
    pub(crate) code: String,
    pub(crate) usage_limit: Option<i32>,
}

/// 代理链上某一级当前生效的返佣规则行，比例语义是从成交用户向上累计的总分成。
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct AgentCommissionRuleRecord {
    pub(crate) agent_id: u64,
    pub(crate) commission_rate: BigDecimal,
}

/// 各业务线生成分层返佣时的统一入参，把归属查找、规则匹配和幂等写入收敛到一处实现。
#[derive(Debug)]
pub(crate) struct AgentBusinessCommissionWrite<'a> {
    // 统一的返佣写入契约，使各业务不再各自复制归属、规则和幂等 SQL。
    /// 产生业务行为的终端用户，归属代理由其邀请关系反查得出。
    pub(crate) user_id: u64,
    /// 返佣规则的产品维度，取值必须是服务层归一化后的五类之一。
    pub(crate) product_type: &'a str,
    /// 业务来源类别，与来源单号共同构成返佣记录的幂等键。
    pub(crate) source_type: &'a str,
    pub(crate) source_id: &'a str,
    /// 计佣基数，非正数时整个写入被跳过。
    pub(crate) source_amount: &'a BigDecimal,
    /// 返佣发放资产，其精度决定各层级金额的截断位数。
    pub(crate) payout_asset_id: u64,
}
