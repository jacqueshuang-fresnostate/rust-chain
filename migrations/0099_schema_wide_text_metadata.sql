-- Canonical definitions generated from a fresh MySQL 8.4 schema after migrations 0001-0098.
-- BLOB and other binary payload types are intentionally excluded from this metadata repair.

ALTER DATABASE CHARACTER SET = utf8mb4 COLLATE = utf8mb4_unicode_ci;

ALTER TABLE `admin_audit_logs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `action` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '管理员审计日志：操作动作',
    MODIFY COLUMN `target_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '管理员审计日志：被操作目标类型',
    MODIFY COLUMN `target_id` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '管理员审计日志：被操作目标 ID',
    MODIFY COLUMN `reason` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '管理员审计日志：操作原因或系统判定原因',
    MODIFY COLUMN `ip` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '管理员审计日志：操作来源 IP';

ALTER TABLE `admin_login_two_factor_challenges`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `challenge_id` CHAR(36)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '';

ALTER TABLE `admin_news_items`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `title` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新闻内容：内容标题',
    MODIFY COLUMN `banner_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '新闻内容：横幅图片 URL',
    MODIFY COLUMN `small_logo_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '新闻内容：小图标 URL',
    MODIFY COLUMN `category` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新闻内容：业务分类',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新闻内容：当前业务状态',
    MODIFY COLUMN `country_code` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '新闻内容：国家或地区代码',
    MODIFY COLUMN `default_locale` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新闻内容：默认语言区域';

ALTER TABLE `admin_roles`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `name` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '管理员角色：显示名称';

ALTER TABLE `admin_two_factor_settings`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `totp_secret_encrypted` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '';

ALTER TABLE `admin_users`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `username` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '管理员账号：登录用户名',
    MODIFY COLUMN `password_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '';

ALTER TABLE `agent_admin_users`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `username` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '代理后台账号：登录用户名',
    MODIFY COLUMN `password_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '';

ALTER TABLE `agent_audit_logs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `action` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '代理审计日志：操作动作',
    MODIFY COLUMN `target_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '代理审计日志：被操作目标类型',
    MODIFY COLUMN `target_id` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '代理审计日志：被操作目标 ID',
    MODIFY COLUMN `ip` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '代理审计日志：操作来源 IP';

ALTER TABLE `agent_commission_records`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `source_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '代理佣金记录：业务来源类型',
    MODIFY COLUMN `source_id` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '代理佣金记录：业务来源 ID',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '代理佣金记录：当前业务状态';

ALTER TABLE `agent_commission_rules`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `product_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '代理佣金规则：佣金适用产品类型',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '代理佣金规则：当前业务状态';

ALTER TABLE `agents`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `agent_code` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '代理商：代理商编码',
    MODIFY COLUMN `path` VARCHAR(2048)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '代理商：当前业务状态';

ALTER TABLE `asset_lock_position_sources`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `source_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '锁仓来源：业务来源类型',
    MODIFY COLUMN `source_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '锁仓来源：业务来源 ID';

ALTER TABLE `asset_lock_positions`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `unlock_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '资产锁仓仓位：解禁方式',
    MODIFY COLUMN `merge_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '资产锁仓仓位：锁仓合并唯一键',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '资产锁仓仓位：当前业务状态';

ALTER TABLE `asset_unlock_records`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `unlock_fee_basis` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '解禁记录：解禁手续费计费基准',
    MODIFY COLUMN `fee_paid_status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'not_required'
        COMMENT '解禁记录：手续费支付状态',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '解禁记录：当前业务状态',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '解禁记录：幂等键，用于防止重复处理';

ALTER TABLE `assets`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `symbol` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '资产币种：业务标识或交易符号',
    MODIFY COLUMN `name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '资产币种：显示名称',
    MODIFY COLUMN `logo_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '资产币种：Logo 图片 URL',
    MODIFY COLUMN `asset_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'coin'
        COMMENT '资产币种：资产类型',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '资产币种：当前业务状态';

ALTER TABLE `audit_events`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `actor_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '全局审计事件：操作主体类型',
    MODIFY COLUMN `action` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '全局审计事件：操作动作',
    MODIFY COLUMN `target_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '全局审计事件：被操作目标类型',
    MODIFY COLUMN `target_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '全局审计事件：被操作目标 ID',
    MODIFY COLUMN `reason` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '全局审计事件：操作原因或系统判定原因',
    MODIFY COLUMN `ip` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '全局审计事件：操作来源 IP';

ALTER TABLE `convert_events`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `event_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '闪兑事件：事件类型';

ALTER TABLE `convert_orders`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `quote_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '闪兑订单：闪兑报价 ID',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '闪兑订单：当前业务状态',
    MODIFY COLUMN `error_message` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '闪兑订单：错误信息';

ALTER TABLE `convert_pairs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `pricing_mode` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '闪兑交易对：闪兑定价模式';

ALTER TABLE `convert_quotes`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `quote_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '闪兑报价：闪兑报价 ID',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'quoted'
        COMMENT '闪兑报价：当前业务状态';

ALTER TABLE `country_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `country_code` VARCHAR(8)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '国家地区配置：国家或地区代码',
    MODIFY COLUMN `country_name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '国家地区配置：国家或地区名称',
    MODIFY COLUMN `remark` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT ''
        COMMENT '中文国家或地区名称备注',
    MODIFY COLUMN `default_locale` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '国家地区配置：默认语言区域',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '国家地区配置：当前业务状态';

ALTER TABLE `deposit_address_pool`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `network` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '充值地址池：链网络，支持 eth/base/tron/btc/solana',
    MODIFY COLUMN `address_group_code` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '充值地址池：地址集合编号，同一编号可被多个充值网络共用',
    MODIFY COLUMN `address` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '充值地址池：链上充值地址',
    MODIFY COLUMN `asset_symbol` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '充值地址池：限定可使用该地址的资产符号，空表示该网络任意资产可用',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'available'
        COMMENT '充值地址池：地址状态，available 可分配，assigned 已分配，disabled 禁用',
    MODIFY COLUMN `assigned_user_email` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '充值地址池：当前分配用户邮箱快照',
    MODIFY COLUMN `assigned_asset_symbol` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '充值地址池：当前用户申请充值的资产符号',
    MODIFY COLUMN `memo` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '充值地址池：地址备注或 Memo 标签',
    MODIFY COLUMN `remark` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '充值地址池：后台备注';

ALTER TABLE `deposit_network_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `network` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '充值网络配置：网络标识，当前支持 eth/base/tron/btc/solana',
    MODIFY COLUMN `display_name` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '充值网络配置：前后台显示名称',
    MODIFY COLUMN `address_group_code` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '充值网络配置：地址集合编号，同一编号共用一类地址',
    MODIFY COLUMN `address_group_name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '充值网络配置：地址集合名称，例如 EVM、Bitcoin、Tron',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '充值网络配置：状态，active 启用，disabled 停用';

ALTER TABLE `deposit_records`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `tx_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '充值记录：链上交易哈希',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '充值记录：当前业务状态';

ALTER TABLE `earn_product_categories`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `code` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '分类代码，理财产品通过该代码关联分类',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '状态：active启用，disabled停用';

ALTER TABLE `earn_products`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '理财产品：显示名称',
    MODIFY COLUMN `banner_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '理财产品：横幅图片 URL',
    MODIFY COLUMN `small_logo_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '理财产品：小图标 URL',
    MODIFY COLUMN `category` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'fixed_term'
        COMMENT '理财产品：业务分类',
    MODIFY COLUMN `early_redeem_fee_basis` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'none'
        COMMENT '提前赎回扣费基准：none不扣费，principal按本金，profit按收益',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '理财产品：当前业务状态';

ALTER TABLE `earn_subscriptions`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `early_redeem_fee_basis` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'none'
        COMMENT '申购时快照的提前赎回扣费基准：none不扣费，principal按本金，profit按收益',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'subscribed'
        COMMENT '理财申购：当前业务状态',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '理财申购：幂等键，用于防止重复处理';

ALTER TABLE `event_inbox`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `consumer_name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '事件收件箱：事件消费者名称',
    MODIFY COLUMN `message_id` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '事件收件箱：外部消息 ID',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '事件收件箱：幂等键，用于防止重复处理',
    MODIFY COLUMN `payload_hash` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '事件收件箱：事件载荷哈希',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'processing'
        COMMENT '事件收件箱：当前业务状态',
    MODIFY COLUMN `error_message` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '事件收件箱：错误信息';

ALTER TABLE `event_outbox`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `aggregate_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '事件发件箱：事件聚合类型',
    MODIFY COLUMN `aggregate_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '事件发件箱：事件聚合根 ID',
    MODIFY COLUMN `event_type` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '事件发件箱：事件类型',
    MODIFY COLUMN `routing_key` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '事件发件箱：事件路由键',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '事件发件箱：幂等键，用于防止重复处理',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '事件发件箱：当前业务状态';

ALTER TABLE `invite_codes`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `owner_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '邀请码：邀请码归属主体类型',
    MODIFY COLUMN `code` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '邀请码：邀请码或业务编码',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '邀请码：当前业务状态';

ALTER TABLE `kyc_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `name` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT 'KYC 配置：显示名称';

ALTER TABLE `loan_orders`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `loan_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '订单贷款类型快照：credit信用贷，collateralized抵押贷',
    MODIFY COLUMN `interest_calculation_mode` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '订单计息方式快照',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '订单状态：pending待审核，disbursed已放款，rejected已拒绝，cancelled已取消，repaid已还款，overdue已逾期',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户提交幂等键',
    MODIFY COLUMN `rejected_reason` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '审核拒绝原因';

ALTER TABLE `loan_products`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `loan_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '贷款类型：credit信用贷，collateralized抵押贷',
    MODIFY COLUMN `name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '贷款产品名称',
    MODIFY COLUMN `interest_calculation_mode` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'full_term'
        COMMENT '提前还款计息方式：full_term完整周期，actual_days实际天数',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '产品状态：active启用，disabled禁用';

ALTER TABLE `login_failure_counters`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `actor_type` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `identifier` VARCHAR(191)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '';

ALTER TABLE `login_two_factor_challenges`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `challenge_id` CHAR(36)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '登录双重验证挑战：二次验证挑战 ID',
    MODIFY COLUMN `challenge_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '登录双重验证挑战：二次验证挑战类型';

ALTER TABLE `margin_cross_accounts`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `status` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '';

ALTER TABLE `margin_liquidation_records`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `direction` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '杠杆强平记录：交易方向',
    MODIFY COLUMN `reason` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '杠杆强平记录：操作原因或系统判定原因';

ALTER TABLE `margin_positions`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `wallet_scope` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'spot'
        COMMENT '',
    MODIFY COLUMN `margin_mode` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'isolated'
        COMMENT '杠杆仓位：杠杆模式',
    MODIFY COLUMN `direction` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '杠杆仓位：交易方向',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'opened'
        COMMENT '杠杆仓位：当前业务状态',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '杠杆仓位：幂等键，用于防止重复处理',
    MODIFY COLUMN `liquidation_reason` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '杠杆仓位：强平原因';

ALTER TABLE `margin_products`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `logo_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '杠杆产品：Logo 图片 URL',
    MODIFY COLUMN `margin_mode` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'isolated'
        COMMENT '杠杆产品：杠杆模式',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '杠杆产品：当前业务状态';

ALTER TABLE `margin_transfers`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `transfer_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `from_account` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `to_account` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `idempotency_key` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '';

ALTER TABLE `margin_user_settings`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `margin_mode` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '';

ALTER TABLE `margin_wallet_accounts`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

ALTER TABLE `margin_wallet_ledger`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `change_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `balance_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `ref_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `ref_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '';

ALTER TABLE `market_feed_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `name` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '行情订阅配置：显示名称',
    MODIFY COLUMN `last_reload_status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '行情订阅配置：最后重载状态',
    MODIFY COLUMN `last_reload_error` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '行情订阅配置：最后重载错误';

ALTER TABLE `market_source_credentials`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `provider` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '行情源凭证：服务提供商标识',
    MODIFY COLUMN `auth_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'none'
        COMMENT '行情源凭证：认证方式',
    MODIFY COLUMN `api_key_ciphertext` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '行情源凭证：API Key 加密密文',
    MODIFY COLUMN `api_secret_ciphertext` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '行情源凭证：API Secret 加密密文',
    MODIFY COLUMN `passphrase_ciphertext` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '行情源凭证：API Passphrase 加密密文',
    MODIFY COLUMN `api_key_mask` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '行情源凭证：API Key 脱敏展示值';

ALTER TABLE `market_sources`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `name` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '行情源：显示名称',
    MODIFY COLUMN `rest_base_url` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '行情源：行情源 REST 基础地址',
    MODIFY COLUMN `ws_url` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '行情源：行情源 WebSocket 地址';

ALTER TABLE `market_strategies`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `strategy_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '行情策略：行情策略类型',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'draft'
        COMMENT '行情策略：当前业务状态';

ALTER TABLE `new_coin_convert_rules`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `rate_source` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新币闪兑规则：汇率来源',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '新币闪兑规则：当前业务状态';

ALTER TABLE `new_coin_distributions`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '新币派发：当前业务状态',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新币派发：幂等键，用于防止重复处理';

ALTER TABLE `new_coin_lifecycle_events`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `event_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新币生命周期事件：事件类型';

ALTER TABLE `new_coin_projects`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `symbol` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新币项目：业务标识或交易符号',
    MODIFY COLUMN `lifecycle_status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'preheat'
        COMMENT '新币项目：新币生命周期状态',
    MODIFY COLUMN `unlock_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新币项目：解禁方式',
    MODIFY COLUMN `unlock_fee_basis` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '新币项目：解禁手续费计费基准',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '新币项目：当前业务状态';

ALTER TABLE `new_coin_purchase_orders`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '新币上市认购订单：当前业务状态',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新币上市认购订单：幂等键，用于防止重复处理';

ALTER TABLE `new_coin_subscriptions`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '新币申购：当前业务状态',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '新币申购：幂等键，用于防止重复处理';

ALTER TABLE `order_events`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `event_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '订单事件：事件类型';

ALTER TABLE `platform_brand_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `name` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT 'PC 品牌配置：显示名称',
    MODIFY COLUMN `platform_name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT 'PC 品牌配置：PC 端展示的平台名称',
    MODIFY COLUMN `logo_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT 'PC 品牌配置：Logo 图片 URL',
    MODIFY COLUMN `chart_provider` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'klinecharts'
        COMMENT 'PC K线图引擎：klinecharts 或 tradingview';

ALTER TABLE `prediction_asset_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

ALTER TABLE `prediction_markets`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `source` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'polymarket'
        COMMENT '',
    MODIFY COLUMN `external_event_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `external_market_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `slug` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `title` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `description` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `image_url` VARCHAR(1024)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `category` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `outcome_yes_label` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'Yes'
        COMMENT '',
    MODIFY COLUMN `outcome_no_label` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'No'
        COMMENT '',
    MODIFY COLUMN `source_status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '',
    MODIFY COLUMN `display_status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '',
    MODIFY COLUMN `external_resolution` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `local_resolution` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `invalid_refund_policy_used` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `settlement_status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'open'
        COMMENT '',
    MODIFY COLUMN `settlement_mode_override` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '';

ALTER TABLE `prediction_orders`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `order_no` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `quote_id` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `idempotency_key` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `outcome` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'open'
        COMMENT '',
    MODIFY COLUMN `result` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `invalid_refund_policy_used` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '';

ALTER TABLE `prediction_quotes`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `quote_id` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `outcome` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '';

ALTER TABLE `prediction_settings`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `default_settlement_mode` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'manual_confirm'
        COMMENT '',
    MODIFY COLUMN `default_invalid_refund_policy` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'refund_stake_and_fee'
        COMMENT '',
    MODIFY COLUMN `last_sync_status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `last_sync_error` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '';

ALTER TABLE `prediction_sync_logs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `trigger_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `error_message` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '';

ALTER TABLE `quick_recharge_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `name` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'default'
        COMMENT '快速充值配置：配置名称，默认配置为 default',
    MODIFY COLUMN `provider` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'gmpay'
        COMMENT '快速充值配置：支付服务商，目前支持 GMPay/Epusdt',
    MODIFY COLUMN `api_base_url` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：Epusdt/GMPay API 基础地址',
    MODIFY COLUMN `merchant_pid` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：GMPay 商户 PID',
    MODIFY COLUMN `merchant_secret_ciphertext` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：GMPay 商户 Secret Key 加密密文',
    MODIFY COLUMN `merchant_secret_mask` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：GMPay 商户 Secret Key 脱敏显示值',
    MODIFY COLUMN `currency` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'cny'
        COMMENT '快速充值配置：用户输入充值金额的法币币种，如 cny/usd',
    MODIFY COLUMN `token` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'usdt'
        COMMENT '快速充值配置：到账资产符号，如 USDT',
    MODIFY COLUMN `network` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'tron'
        COMMENT '快速充值配置：GMPay 收款网络，如 tron/ethereum/solana',
    MODIFY COLUMN `notify_url` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：GMPay 支付成功异步回调地址',
    MODIFY COLUMN `redirect_url` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：支付完成后的同步跳转地址',
    MODIFY COLUMN `pc_app_redirect_url` VARCHAR(1024)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：PC 应用端支付完成回跳地址',
    MODIFY COLUMN `mac_app_redirect_url` VARCHAR(1024)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：Mac 应用端支付完成回跳地址',
    MODIFY COLUMN `ios_app_redirect_url` VARCHAR(1024)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：iOS 端支付完成回跳地址',
    MODIFY COLUMN `android_app_redirect_url` VARCHAR(1024)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：Android 端支付完成回跳地址',
    MODIFY COLUMN `mobile_web_redirect_url` VARCHAR(1024)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：手机网页端支付完成回跳地址',
    MODIFY COLUMN `desktop_web_redirect_url` VARCHAR(1024)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值配置：电脑网页端支付完成回跳地址';

ALTER TABLE `quick_recharge_orders`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `order_id` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '快速充值订单：平台侧商户订单号，传给 GMPay',
    MODIFY COLUMN `user_email` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值订单：用户邮箱快照，便于后台查账',
    MODIFY COLUMN `asset_symbol` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '快速充值订单：到账资产符号',
    MODIFY COLUMN `currency` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '快速充值订单：用户提交的法币币种',
    MODIFY COLUMN `token` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '快速充值订单：GMPay 实际收款币种',
    MODIFY COLUMN `network` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '快速充值订单：GMPay 实际收款网络',
    MODIFY COLUMN `provider_trade_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值订单：GMPay 交易号',
    MODIFY COLUMN `receive_address` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值订单：GMPay 分配的链上收款地址',
    MODIFY COLUMN `payment_url` VARCHAR(1024)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值订单：GMPay 收银台支付链接',
    MODIFY COLUMN `return_target` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值订单：创建订单时选择的回跳终端类型',
    MODIFY COLUMN `redirect_url` VARCHAR(1024)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值订单：创建订单时传给服务商的支付完成回跳地址',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'created'
        COMMENT '快速充值订单：状态，created 已创建，pending 待支付，paid 已支付，failed 失败，expired 已过期',
    MODIFY COLUMN `block_transaction_id` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '快速充值订单：GMPay 回调的链上交易哈希';

ALTER TABLE `refresh_tokens`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `actor_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '刷新令牌：操作主体类型',
    MODIFY COLUMN `token_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '刷新令牌：刷新令牌哈希';

ALTER TABLE `risk_events`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `actor_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '风控事件：操作主体类型',
    MODIFY COLUMN `event_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '风控事件：事件类型',
    MODIFY COLUMN `risk_level` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '风控事件：风险等级',
    MODIFY COLUMN `decision` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '风控事件：风控处置结果',
    MODIFY COLUMN `reason` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '风控事件：操作原因或系统判定原因';

ALTER TABLE `risk_rules`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `rule_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '风控规则：风控规则类型',
    MODIFY COLUMN `target_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '风控规则：被操作目标类型',
    MODIFY COLUMN `target_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '风控规则：被操作目标 ID';

ALTER TABLE `seconds_contract_orders`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `direction` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '秒合约订单：交易方向',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'opened'
        COMMENT '秒合约订单：当前业务状态',
    MODIFY COLUMN `result` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '秒合约订单：结算结果',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '秒合约订单：幂等键，用于防止重复处理';

ALTER TABLE `seconds_contract_product_cycles`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

ALTER TABLE `seconds_contract_products`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `logo_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '秒合约产品：Logo 图片 URL',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '秒合约产品：当前业务状态';

ALTER TABLE `security_policy_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `policy_key` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '安全策略配置：安全策略键';

ALTER TABLE `sensitive_operation_confirmations`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `actor_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '敏感操作确认：操作主体类型',
    MODIFY COLUMN `operation_type` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '敏感操作确认：敏感操作类型',
    MODIFY COLUMN `operation_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '敏感操作确认：敏感操作 ID',
    MODIFY COLUMN `confirmation_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '敏感操作确认：确认方式',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '敏感操作确认：当前业务状态';

ALTER TABLE `smtp_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `name` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '发信配置：显示名称',
    MODIFY COLUMN `host` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '发信配置：SMTP 主机',
    MODIFY COLUMN `security` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '发信配置：SMTP 加密方式',
    MODIFY COLUMN `username_ciphertext` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '发信配置：SMTP 用户名加密密文',
    MODIFY COLUMN `password_ciphertext` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '发信配置：发信密码加密密文',
    MODIFY COLUMN `username_mask` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '发信配置：SMTP 用户名脱敏展示值',
    MODIFY COLUMN `from_email` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '发信配置：发件邮箱',
    MODIFY COLUMN `from_name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '发信配置：发件人名称',
    MODIFY COLUMN `verification_code_template_html` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '发信配置：默认验证码 HTML 模板';

ALTER TABLE `smtp_delivery_settings`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `strategy` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'priority'
        COMMENT '发信策略配置：发信选择策略';

ALTER TABLE `spot_orders`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `side` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '现货订单：买卖方向',
    MODIFY COLUMN `order_type` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '现货订单：订单类型',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '现货订单：当前业务状态',
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '现货订单：幂等键，用于防止重复处理';

ALTER TABLE `spot_trades`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `idempotency_key` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '现货成交：幂等键，用于防止重复处理';

ALTER TABLE `strategy_events`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `event_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '策略事件：事件类型';

ALTER TABLE `strategy_runs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `run_status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '策略运行状态：策略运行状态',
    MODIFY COLUMN `recovery_status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '策略运行状态：策略恢复状态',
    MODIFY COLUMN `error_message` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '策略运行状态：错误信息';

ALTER TABLE `strategy_versions`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `seed` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '策略版本：策略随机种子';

ALTER TABLE `trading_pairs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `symbol` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '交易对配置：业务标识或交易符号',
    MODIFY COLUMN `logo_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '交易对配置：Logo 图片 URL',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'disabled'
        COMMENT '交易对配置：当前业务状态',
    MODIFY COLUMN `market_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '交易对配置：交易市场类型';

ALTER TABLE `upload_objects`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `provider` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '上传文件对象：服务提供商标识',
    MODIFY COLUMN `object_key` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '上传文件对象：对象存储 Key',
    MODIFY COLUMN `public_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '上传文件对象：文件公开访问 URL',
    MODIFY COLUMN `share_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传文件对象：文件分享 URL',
    MODIFY COLUMN `delete_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传文件对象：文件删除 URL',
    MODIFY COLUMN `mime_type` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '上传文件对象：文件 MIME 类型',
    MODIFY COLUMN `original_filename` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传文件对象：上传原始文件名';

ALTER TABLE `upload_storage_configs`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `name` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '上传存储配置：显示名称',
    MODIFY COLUMN `provider` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '上传存储配置：服务提供商标识',
    MODIFY COLUMN `endpoint` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：上传服务接口地址',
    MODIFY COLUMN `file_field` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：上传接口文件字段名',
    MODIFY COLUMN `bearer_token_ciphertext` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：Bearer Token 加密密文',
    MODIFY COLUMN `bearer_token_mask` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：Bearer Token 脱敏展示值',
    MODIFY COLUMN `access_key_ciphertext` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：Access Key 加密密文',
    MODIFY COLUMN `access_key_mask` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：Access Key 脱敏展示值',
    MODIFY COLUMN `secret_key_ciphertext` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：Secret Key 加密密文',
    MODIFY COLUMN `bucket` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：对象存储桶名称',
    MODIFY COLUMN `region` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：对象存储区域',
    MODIFY COLUMN `public_base_url` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：公开访问基础地址',
    MODIFY COLUMN `local_root` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：本地存储根目录',
    MODIFY COLUMN `key_prefix` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '上传存储配置：对象 Key 前缀';

ALTER TABLE `user_email_verifications`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `email` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户邮箱验证码：邮箱地址',
    MODIFY COLUMN `purpose` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户邮箱验证码：验证码用途',
    MODIFY COLUMN `code_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户邮箱验证码：验证码哈希',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '用户邮箱验证码：当前业务状态';

ALTER TABLE `user_kyc_submissions`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `real_name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户 KYC 提交：用户实名姓名',
    MODIFY COLUMN `country` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户 KYC 提交：用户提交的国家或地区',
    MODIFY COLUMN `id_number` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户 KYC 提交：证件号码',
    MODIFY COLUMN `document_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'identity_card'
        COMMENT '用户 KYC 提交：证件类型',
    MODIFY COLUMN `document_front_image` MEDIUMTEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户 KYC 提交：证件正面图片',
    MODIFY COLUMN `document_back_image` MEDIUMTEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户 KYC 提交：证件反面图片',
    MODIFY COLUMN `document_handheld_image` MEDIUMTEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户 KYC 提交：本人手持证件图片',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '用户 KYC 提交：当前业务状态',
    MODIFY COLUMN `review_reason` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户 KYC 提交：审核拒绝或备注原因',
    MODIFY COLUMN `submission_type` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'personal'
        COMMENT '认证类型：personal(个人) / enterprise(企业)',
    MODIFY COLUMN `enterprise_name` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '企业认证时的企业名称',
    MODIFY COLUMN `business_registration_number` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '企业认证时的统一社会信用代码';

ALTER TABLE `user_referrals`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `direct_inviter_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户邀请关系：直接邀请人类型',
    MODIFY COLUMN `path` VARCHAR(2048)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '用户邀请关系：邀请链路路径';

ALTER TABLE `user_registration_email_verifications`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `email` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '注册邮箱',
    MODIFY COLUMN `purpose` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'register'
        COMMENT '验证码用途',
    MODIFY COLUMN `code_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '验证码哈希',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '验证码状态：pending待验证，verified已验证，superseded已失效';

ALTER TABLE `user_security`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `fund_password_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户安全配置：资金密码哈希',
    MODIFY COLUMN `anti_phishing_code` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户安全配置：用户防钓鱼码';

ALTER TABLE `user_third_party_bindings`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `provider` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '绑定提供方：coinbase_wallet Coinbase钱包，telegram_account TG账号',
    MODIFY COLUMN `account_identifier` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '第三方账号标识，比如钱包地址或TG用户名',
    MODIFY COLUMN `display_name` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '第三方账号显示名称',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'bound'
        COMMENT '绑定状态：bound已绑定，disabled已停用';

ALTER TABLE `user_two_factor_settings`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `totp_secret_encrypted` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户双重验证配置：TOTP 密钥加密密文';

ALTER TABLE `users`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `username` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户登录用户名，唯一，标准化小写',
    MODIFY COLUMN `email` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户账号：邮箱地址',
    MODIFY COLUMN `phone` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户账号：手机号',
    MODIFY COLUMN `avatar_url` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户头像 URL',
    MODIFY COLUMN `country_code` VARCHAR(8)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户账号：国家或地区代码',
    MODIFY COLUMN `preferred_locale` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '用户账号：用户偏好的语言区域',
    MODIFY COLUMN `password_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'active'
        COMMENT '';

ALTER TABLE `wallet_accounts`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;

ALTER TABLE `wallet_chain_event_dead_letters`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `network` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `event_kind` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `dedup_key` VARCHAR(512)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `request_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `tx_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `failure_reason` VARCHAR(500)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '';

ALTER TABLE `wallet_chain_gateways`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `network` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `broadcast_url` VARCHAR(1000)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `event_poll_url` VARCHAR(1000)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `auth_token_encrypted` TEXT
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `status` VARCHAR(16)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'disabled'
        COMMENT '',
    MODIFY COLUMN `last_deposit_cursor` VARCHAR(500)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '';

ALTER TABLE `wallet_deposit_events`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `asset_symbol` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `network` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `address` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `memo` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `tx_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'observed'
        COMMENT '',
    MODIFY COLUMN `failure_reason` VARCHAR(500)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '';

ALTER TABLE `wallet_ledger`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `change_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '钱包流水：钱包变动类型',
    MODIFY COLUMN `balance_type` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '钱包流水：变动的余额类型',
    MODIFY COLUMN `ref_type` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '钱包流水：关联业务类型',
    MODIFY COLUMN `ref_id` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '钱包流水：关联业务 ID';

ALTER TABLE `wallet_withdrawal_requests`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `asset_symbol` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '钱包提现申请：提现资产符号',
    MODIFY COLUMN `network` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '钱包提现申请：提现网络',
    MODIFY COLUMN `address` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '钱包提现申请：提现或链上地址',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '钱包提现申请：当前业务状态',
    MODIFY COLUMN `security_method` VARCHAR(64)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '钱包提现申请：提现安全验证方式',
    MODIFY COLUMN `idempotency_key` VARCHAR(128)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `gateway_request_id` CHAR(36)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL
        COMMENT '',
    MODIFY COLUMN `tx_hash` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `failure_reason` VARCHAR(500)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '',
    MODIFY COLUMN `review_reason` VARCHAR(500)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '';

ALTER TABLE `withdraw_records`
    DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
    MODIFY COLUMN `address` VARCHAR(255)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NULL DEFAULT NULL
        COMMENT '提现记录：提现或链上地址',
    MODIFY COLUMN `status` VARCHAR(32)
        CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci
        NOT NULL DEFAULT 'pending'
        COMMENT '提现记录：当前业务状态';
