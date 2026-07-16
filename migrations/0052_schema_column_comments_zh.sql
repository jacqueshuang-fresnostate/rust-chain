SET @schema_comment_old_group_concat_max_len := @@SESSION.group_concat_max_len;
SET @schema_comment_old_foreign_key_checks := @@SESSION.foreign_key_checks;
SET SESSION group_concat_max_len = 1048576;
SET FOREIGN_KEY_CHECKS = 0;

CREATE TEMPORARY TABLE schema_comment_targets (
    table_name VARCHAR(128) PRIMARY KEY,
    table_comment VARCHAR(128) NOT NULL
);

INSERT INTO schema_comment_targets (table_name, table_comment)
VALUES
    ('users', '用户账号'),
    ('user_security', '用户安全配置'),
    ('refresh_tokens', '刷新令牌'),
    ('admin_roles', '管理员角色'),
    ('admin_users', '管理员账号'),
    ('admin_audit_logs', '管理员审计日志'),
    ('agents', '代理商'),
    ('agent_admin_users', '代理后台账号'),
    ('invite_codes', '邀请码'),
    ('user_referrals', '用户邀请关系'),
    ('agent_audit_logs', '代理审计日志'),
    ('agent_commission_rules', '代理佣金规则'),
    ('agent_commission_records', '代理佣金记录'),
    ('assets', '资产币种'),
    ('wallet_accounts', '用户钱包账户'),
    ('wallet_ledger', '钱包流水'),
    ('asset_lock_positions', '资产锁仓仓位'),
    ('asset_lock_position_sources', '锁仓来源'),
    ('asset_unlock_records', '解禁记录'),
    ('deposit_records', '充值记录'),
    ('withdraw_records', '提现记录'),
    ('trading_pairs', '交易对配置'),
    ('market_sources', '行情源'),
    ('market_strategies', '行情策略'),
    ('strategy_runs', '策略运行状态'),
    ('strategy_versions', '策略版本'),
    ('strategy_events', '策略事件'),
    ('spot_orders', '现货订单'),
    ('spot_trades', '现货成交'),
    ('order_events', '订单事件'),
    ('new_coin_projects', '新币项目'),
    ('new_coin_subscriptions', '新币申购'),
    ('new_coin_distributions', '新币派发'),
    ('new_coin_purchase_orders', '新币上市认购订单'),
    ('new_coin_lifecycle_events', '新币生命周期事件'),
    ('convert_pairs', '闪兑交易对'),
    ('new_coin_convert_rules', '新币闪兑规则'),
    ('convert_quotes', '闪兑报价'),
    ('convert_orders', '闪兑订单'),
    ('convert_events', '闪兑事件'),
    ('event_outbox', '事件发件箱'),
    ('event_inbox', '事件收件箱'),
    ('risk_rules', '风控规则'),
    ('risk_events', '风控事件'),
    ('sensitive_operation_confirmations', '敏感操作确认'),
    ('audit_events', '全局审计事件'),
    ('seconds_contract_products', '秒合约产品'),
    ('seconds_contract_orders', '秒合约订单'),
    ('margin_products', '杠杆产品'),
    ('margin_positions', '杠杆仓位'),
    ('earn_products', '理财产品'),
    ('earn_subscriptions', '理财申购'),
    ('margin_liquidation_records', '杠杆强平记录'),
    ('market_feed_configs', '行情订阅配置'),
    ('market_source_credentials', '行情源凭证'),
    ('user_email_verifications', '用户邮箱验证码'),
    ('smtp_configs', '发信配置'),
    ('upload_storage_configs', '上传存储配置'),
    ('upload_objects', '上传文件对象'),
    ('admin_news_items', '新闻内容'),
    ('country_configs', '国家地区配置'),
    ('user_two_factor_settings', '用户双重验证配置'),
    ('security_policy_configs', '安全策略配置'),
    ('login_two_factor_challenges', '登录双重验证挑战'),
    ('wallet_withdrawal_requests', '钱包提现申请'),
    ('kyc_configs', 'KYC 配置'),
    ('user_kyc_submissions', '用户 KYC 提交'),
    ('platform_brand_configs', 'PC 品牌配置'),
    ('smtp_delivery_settings', '发信策略配置');

CREATE TEMPORARY TABLE schema_column_comment_targets AS
SELECT
    c.TABLE_NAME AS table_name,
    c.COLUMN_NAME AS column_name,
    CONCAT(
        t.table_comment,
        '：',
        CASE
            WHEN c.COLUMN_NAME = 'id' THEN '记录主键 ID'
            WHEN c.COLUMN_NAME = 'user_id' THEN '关联用户 ID'
            WHEN c.COLUMN_NAME = 'admin_id' THEN '关联管理员 ID'
            WHEN c.COLUMN_NAME = 'agent_id' THEN '关联代理商 ID'
            WHEN c.COLUMN_NAME = 'agent_admin_id' THEN '关联代理后台账号 ID'
            WHEN c.COLUMN_NAME = 'asset_id' THEN '关联资产币种 ID'
            WHEN c.COLUMN_NAME = 'pair_id' THEN '关联交易对 ID'
            WHEN c.COLUMN_NAME = 'product_id' THEN '关联产品 ID'
            WHEN c.COLUMN_NAME = 'project_id' THEN '关联新币项目 ID'
            WHEN c.COLUMN_NAME = 'position_id' THEN '关联杠杆仓位 ID'
            WHEN c.COLUMN_NAME = 'lock_position_id' THEN '关联锁仓仓位 ID'
            WHEN c.COLUMN_NAME = 'subscription_id' THEN '关联申购记录 ID'
            WHEN c.COLUMN_NAME = 'order_id' THEN '关联订单 ID'
            WHEN c.COLUMN_NAME = 'convert_pair_id' THEN '关联闪兑交易对 ID'
            WHEN c.COLUMN_NAME = 'convert_order_id' THEN '关联闪兑订单 ID'
            WHEN c.COLUMN_NAME = 'created_by' THEN '创建该记录的管理员 ID'
            WHEN c.COLUMN_NAME = 'created_by_admin_id' THEN '创建内容的管理员 ID'
            WHEN c.COLUMN_NAME = 'updated_by' THEN '最后更新该配置的管理员 ID'
            WHEN c.COLUMN_NAME = 'updated_by_admin_id' THEN '最后更新内容的管理员 ID'
            WHEN c.COLUMN_NAME = 'reviewed_by' THEN '审核该记录的管理员 ID'
            WHEN c.COLUMN_NAME = 'uploaded_by' THEN '上传文件的管理员 ID'
            WHEN c.COLUMN_NAME = 'role_id' THEN '管理员所属角色 ID'
            WHEN c.COLUMN_NAME = 'root_agent_id' THEN '归属的顶级代理商 ID'
            WHEN c.COLUMN_NAME = 'direct_inviter_id' THEN '直接邀请人 ID'
            WHEN c.COLUMN_NAME = 'owner_id' THEN '邀请码归属主体 ID'
            WHEN c.COLUMN_NAME = 'actor_id' THEN '操作或登录主体 ID'
            WHEN c.COLUMN_NAME = 'target_id' THEN '被操作目标 ID'
            WHEN c.COLUMN_NAME = 'aggregate_id' THEN '事件聚合根 ID'
            WHEN c.COLUMN_NAME = 'strategy_id' THEN '关联行情策略 ID'
            WHEN c.COLUMN_NAME = 'status' THEN '当前业务状态'
            WHEN c.COLUMN_NAME = 'enabled' THEN '是否启用'
            WHEN c.COLUMN_NAME = 'created_at' THEN '记录创建时间'
            WHEN c.COLUMN_NAME = 'updated_at' THEN '记录最后更新时间'
            WHEN c.COLUMN_NAME = 'expires_at' THEN '过期时间'
            WHEN c.COLUMN_NAME = 'confirmed_at' THEN '确认完成时间'
            WHEN c.COLUMN_NAME = 'consumed_at' THEN '消费完成时间'
            WHEN c.COLUMN_NAME = 'published_at' THEN '发布时间'
            WHEN c.COLUMN_NAME = 'submitted_at' THEN '提交时间'
            WHEN c.COLUMN_NAME = 'reviewed_at' THEN '审核时间'
            WHEN c.COLUMN_NAME = 'verified_at' THEN '验证完成时间'
            WHEN c.COLUMN_NAME = 'sent_at' THEN '发送时间'
            WHEN c.COLUMN_NAME = 'last_login_at' THEN '最后登录时间'
            WHEN c.COLUMN_NAME = 'last_verified_at' THEN '最后一次验证时间'
            WHEN c.COLUMN_NAME = 'last_reloaded_at' THEN '最后重载时间'
            WHEN c.COLUMN_NAME = 'name' THEN '显示名称'
            WHEN c.COLUMN_NAME = 'title' THEN '内容标题'
            WHEN c.COLUMN_NAME = 'symbol' THEN '业务标识或交易符号'
            WHEN c.COLUMN_NAME = 'email' THEN '邮箱地址'
            WHEN c.COLUMN_NAME = 'phone' THEN '手机号'
            WHEN c.COLUMN_NAME = 'username' THEN '登录用户名'
            WHEN c.COLUMN_NAME = 'password_hash' THEN '登录密码哈希'
            WHEN c.COLUMN_NAME = 'password_ciphertext' THEN '发信密码加密密文'
            WHEN c.COLUMN_NAME = 'fund_password_hash' THEN '资金密码哈希'
            WHEN c.COLUMN_NAME = 'token_hash' THEN '刷新令牌哈希'
            WHEN c.COLUMN_NAME = 'code_hash' THEN '验证码哈希'
            WHEN c.COLUMN_NAME = 'payload_hash' THEN '事件载荷哈希'
            WHEN c.COLUMN_NAME = 'idempotency_key' THEN '幂等键，用于防止重复处理'
            WHEN c.COLUMN_NAME = 'request_reference_price' THEN '下单时参考价格'
            WHEN c.COLUMN_NAME = 'request_price' THEN '用户请求的下单价格'
            WHEN c.COLUMN_NAME = 'price' THEN '成交或下单价格'
            WHEN c.COLUMN_NAME = 'entry_price' THEN '开仓或入场价格'
            WHEN c.COLUMN_NAME = 'exit_price' THEN '平仓价格'
            WHEN c.COLUMN_NAME = 'mark_price' THEN '强平时标记价格'
            WHEN c.COLUMN_NAME = 'issue_price' THEN '新币发行价格'
            WHEN c.COLUMN_NAME = 'target_price' THEN '策略目标价格'
            WHEN c.COLUMN_NAME = 'start_price' THEN '策略起始价格'
            WHEN c.COLUMN_NAME = 'current_price' THEN '当前策略价格'
            WHEN c.COLUMN_NAME = 'fixed_rate' THEN '固定兑换汇率'
            WHEN c.COLUMN_NAME = 'rate' THEN '实际兑换汇率'
            WHEN c.COLUMN_NAME = 'spread_rate' THEN '报价价差比例'
            WHEN c.COLUMN_NAME = 'commission_rate' THEN '佣金比例'
            WHEN c.COLUMN_NAME = 'apr_rate' THEN '年化收益率'
            WHEN c.COLUMN_NAME = 'payout_rate' THEN '赔付赔率'
            WHEN c.COLUMN_NAME = 'hourly_interest_rate' THEN '小时借款利率'
            WHEN c.COLUMN_NAME = 'maintenance_margin_rate' THEN '维持保证金率'
            WHEN c.COLUMN_NAME = 'unlock_fee_rate' THEN '解禁手续费率'
            WHEN c.COLUMN_NAME = 'amount' THEN '业务金额或数量'
            WHEN c.COLUMN_NAME = 'quantity' THEN '交易数量'
            WHEN c.COLUMN_NAME = 'filled_quantity' THEN '已成交数量'
            WHEN c.COLUMN_NAME = 'requested_quantity' THEN '用户请求申购数量'
            WHEN c.COLUMN_NAME = 'allocated_quantity' THEN '最终分配数量'
            WHEN c.COLUMN_NAME = 'quote_amount' THEN '计价资产金额'
            WHEN c.COLUMN_NAME = 'from_amount' THEN '闪兑支付数量'
            WHEN c.COLUMN_NAME = 'to_amount' THEN '闪兑获得数量'
            WHEN c.COLUMN_NAME = 'stake_amount' THEN '秒合约押注金额'
            WHEN c.COLUMN_NAME = 'margin_amount' THEN '保证金金额'
            WHEN c.COLUMN_NAME = 'notional_amount' THEN '名义仓位价值'
            WHEN c.COLUMN_NAME = 'borrowed_amount' THEN '借款本金金额'
            WHEN c.COLUMN_NAME = 'interest_amount' THEN '已计提利息金额'
            WHEN c.COLUMN_NAME = 'commission_amount' THEN '佣金金额'
            WHEN c.COLUMN_NAME = 'source_amount' THEN '来源业务金额'
            WHEN c.COLUMN_NAME = 'locked_amount' THEN '锁定总数量'
            WHEN c.COLUMN_NAME = 'released_amount' THEN '已释放数量'
            WHEN c.COLUMN_NAME = 'remaining_amount' THEN '剩余锁定数量'
            WHEN c.COLUMN_NAME = 'reserved_amount' THEN '订单预占金额'
            WHEN c.COLUMN_NAME = 'payout_amount' THEN '强平后返还或赔付金额'
            WHEN c.COLUMN_NAME = 'unlock_quantity' THEN '本次解禁数量'
            WHEN c.COLUMN_NAME = 'unlock_fee_amount' THEN '解禁手续费金额'
            WHEN c.COLUMN_NAME = 'fee' THEN '手续费金额'
            WHEN c.COLUMN_NAME = 'equity' THEN '强平时账户权益'
            WHEN c.COLUMN_NAME = 'maintenance_margin' THEN '强平时维持保证金要求'
            WHEN c.COLUMN_NAME = 'available' THEN '可用余额'
            WHEN c.COLUMN_NAME = 'frozen' THEN '冻结余额'
            WHEN c.COLUMN_NAME = 'locked' THEN '锁定余额'
            WHEN c.COLUMN_NAME = 'balance_after' THEN '变动后的目标余额'
            WHEN c.COLUMN_NAME = 'available_after' THEN '变动后的可用余额'
            WHEN c.COLUMN_NAME = 'frozen_after' THEN '变动后的冻结余额'
            WHEN c.COLUMN_NAME = 'locked_after' THEN '变动后的锁定余额'
            WHEN c.COLUMN_NAME = 'base_asset' THEN '基础资产 ID'
            WHEN c.COLUMN_NAME = 'quote_asset' THEN '计价资产 ID'
            WHEN c.COLUMN_NAME = 'from_asset' THEN '闪兑支付资产 ID'
            WHEN c.COLUMN_NAME = 'to_asset' THEN '闪兑获得资产 ID'
            WHEN c.COLUMN_NAME = 'stake_asset' THEN '秒合约押注资产 ID'
            WHEN c.COLUMN_NAME = 'margin_asset' THEN '杠杆保证金资产 ID'
            WHEN c.COLUMN_NAME = 'reserved_asset' THEN '订单预占资产 ID'
            WHEN c.COLUMN_NAME = 'unlock_fee_asset' THEN '解禁手续费资产 ID'
            WHEN c.COLUMN_NAME = 'asset_symbol' THEN '提现资产符号'
            WHEN c.COLUMN_NAME = 'asset_type' THEN '资产类型'
            WHEN c.COLUMN_NAME = 'precision_scale' THEN '资产数量精度'
            WHEN c.COLUMN_NAME = 'price_precision' THEN '价格小数精度'
            WHEN c.COLUMN_NAME = 'qty_precision' THEN '数量小数精度'
            WHEN c.COLUMN_NAME = 'min_order_value' THEN '最小下单金额'
            WHEN c.COLUMN_NAME = 'market_type' THEN '交易市场类型'
            WHEN c.COLUMN_NAME = 'side' THEN '买卖方向'
            WHEN c.COLUMN_NAME = 'order_type' THEN '订单类型'
            WHEN c.COLUMN_NAME = 'direction' THEN '交易方向'
            WHEN c.COLUMN_NAME = 'result' THEN '结算结果'
            WHEN c.COLUMN_NAME = 'duration_seconds' THEN '秒合约周期秒数'
            WHEN c.COLUMN_NAME = 'min_stake' THEN '最小押注金额'
            WHEN c.COLUMN_NAME = 'max_stake' THEN '最大押注金额；为空表示无上限'
            WHEN c.COLUMN_NAME = 'max_leverage' THEN '最大杠杆倍数'
            WHEN c.COLUMN_NAME = 'leverage' THEN '实际杠杆倍数'
            WHEN c.COLUMN_NAME = 'min_margin' THEN '最小保证金'
            WHEN c.COLUMN_NAME = 'max_margin' THEN '最大保证金'
            WHEN c.COLUMN_NAME = 'margin_mode' THEN '杠杆模式'
            WHEN c.COLUMN_NAME = 'leverage_levels' THEN '可选杠杆档位 JSON'
            WHEN c.COLUMN_NAME = 'term_days' THEN '理财期限天数'
            WHEN c.COLUMN_NAME = 'min_subscribe' THEN '最小申购金额'
            WHEN c.COLUMN_NAME = 'max_subscribe' THEN '最大申购金额'
            WHEN c.COLUMN_NAME = 'subscribed_at' THEN '理财申购时间'
            WHEN c.COLUMN_NAME = 'matures_at' THEN '理财到期时间'
            WHEN c.COLUMN_NAME = 'redeemed_at' THEN '理财赎回时间'
            WHEN c.COLUMN_NAME = 'category' THEN '业务分类'
            WHEN c.COLUMN_NAME = 'introduction_json' THEN '理财产品多语言介绍 JSON'
            WHEN c.COLUMN_NAME = 'banner_url' THEN '横幅图片 URL'
            WHEN c.COLUMN_NAME = 'small_logo_url' THEN '小图标 URL'
            WHEN c.COLUMN_NAME = 'logo_url' THEN 'Logo 图片 URL'
            WHEN c.COLUMN_NAME = 'platform_name' THEN 'PC 端展示的平台名称'
            WHEN c.COLUMN_NAME = 'content_json' THEN '新闻多语言内容 JSON'
            WHEN c.COLUMN_NAME = 'country_code' THEN '国家或地区代码'
            WHEN c.COLUMN_NAME = 'country_name' THEN '国家或地区名称'
            WHEN c.COLUMN_NAME = 'country' THEN '用户提交的国家或地区'
            WHEN c.COLUMN_NAME = 'default_locale' THEN '默认语言区域'
            WHEN c.COLUMN_NAME = 'preferred_locale' THEN '用户偏好的语言区域'
            WHEN c.COLUMN_NAME = 'supported_locales' THEN '支持的语言区域列表'
            WHEN c.COLUMN_NAME = 'registration_enabled' THEN '是否开放注册'
            WHEN c.COLUMN_NAME = 'sort_order' THEN '后台展示排序'
            WHEN c.COLUMN_NAME = 'kyc_level' THEN '用户当前 KYC 等级'
            WHEN c.COLUMN_NAME = 'target_kyc_level' THEN '审核通过后授予的 KYC 等级'
            WHEN c.COLUMN_NAME = 'required_documents_json' THEN '要求上传的证件项 JSON'
            WHEN c.COLUMN_NAME = 'allowed_countries_json' THEN '允许提交 KYC 的国家 JSON'
            WHEN c.COLUMN_NAME = 'country_document_types_json' THEN '各国家允许的证件类型规则 JSON'
            WHEN c.COLUMN_NAME = 'max_document_size_bytes' THEN '单个证件文件最大字节数'
            WHEN c.COLUMN_NAME = 'real_name' THEN '用户实名姓名'
            WHEN c.COLUMN_NAME = 'id_number' THEN '证件号码'
            WHEN c.COLUMN_NAME = 'document_type' THEN '证件类型'
            WHEN c.COLUMN_NAME = 'document_front_image' THEN '证件正面图片'
            WHEN c.COLUMN_NAME = 'document_back_image' THEN '证件反面图片'
            WHEN c.COLUMN_NAME = 'document_handheld_image' THEN '本人手持证件图片'
            WHEN c.COLUMN_NAME = 'review_reason' THEN '审核拒绝或备注原因'
            WHEN c.COLUMN_NAME = 'email_verified_at' THEN '邮箱验证完成时间'
            WHEN c.COLUMN_NAME = 'purpose' THEN '验证码用途'
            WHEN c.COLUMN_NAME = 'attempt_count' THEN '验证码尝试次数'
            WHEN c.COLUMN_NAME = 'totp_secret_encrypted' THEN 'TOTP 密钥加密密文'
            WHEN c.COLUMN_NAME = 'totp_enabled' THEN '是否启用 TOTP'
            WHEN c.COLUMN_NAME = 'login_2fa_enabled' THEN '是否启用登录二次验证'
            WHEN c.COLUMN_NAME = 'challenge_id' THEN '二次验证挑战 ID'
            WHEN c.COLUMN_NAME = 'challenge_type' THEN '二次验证挑战类型'
            WHEN c.COLUMN_NAME = 'security_method' THEN '提现安全验证方式'
            WHEN c.COLUMN_NAME = 'policy_key' THEN '安全策略键'
            WHEN c.COLUMN_NAME = 'policy_value' THEN '安全策略值'
            WHEN c.COLUMN_NAME = 'anti_phishing_code' THEN '用户防钓鱼码'
            WHEN c.COLUMN_NAME = 'actor_type' THEN '操作主体类型'
            WHEN c.COLUMN_NAME = 'target_type' THEN '被操作目标类型'
            WHEN c.COLUMN_NAME = 'action' THEN '操作动作'
            WHEN c.COLUMN_NAME = 'reason' THEN '操作原因或系统判定原因'
            WHEN c.COLUMN_NAME = 'before_json' THEN '变更前数据快照 JSON'
            WHEN c.COLUMN_NAME = 'after_json' THEN '变更后数据快照 JSON'
            WHEN c.COLUMN_NAME = 'ip' THEN '操作来源 IP'
            WHEN c.COLUMN_NAME = 'permissions' THEN '角色权限集合'
            WHEN c.COLUMN_NAME = 'agent_code' THEN '代理商编码'
            WHEN c.COLUMN_NAME = 'level' THEN '代理层级'
            WHEN c.COLUMN_NAME = 'owner_type' THEN '邀请码归属主体类型'
            WHEN c.COLUMN_NAME = 'code' THEN '邀请码或业务编码'
            WHEN c.COLUMN_NAME = 'usage_limit' THEN '邀请码最大可使用次数'
            WHEN c.COLUMN_NAME = 'used_count' THEN '邀请码已使用次数'
            WHEN c.COLUMN_NAME = 'direct_inviter_type' THEN '直接邀请人类型'
            WHEN c.COLUMN_NAME = 'depth' THEN '邀请层级深度'
            WHEN c.COLUMN_NAME = 'path' THEN '邀请链路路径'
            WHEN c.COLUMN_NAME = 'product_type' THEN '佣金适用产品类型'
            WHEN c.COLUMN_NAME = 'source_type' THEN '业务来源类型'
            WHEN c.COLUMN_NAME = 'source_id' THEN '业务来源 ID'
            WHEN c.COLUMN_NAME = 'source_time' THEN '业务来源发生时间'
            WHEN c.COLUMN_NAME = 'change_type' THEN '钱包变动类型'
            WHEN c.COLUMN_NAME = 'balance_type' THEN '变动的余额类型'
            WHEN c.COLUMN_NAME = 'ref_type' THEN '关联业务类型'
            WHEN c.COLUMN_NAME = 'ref_id' THEN '关联业务 ID'
            WHEN c.COLUMN_NAME = 'unlock_type' THEN '解禁方式'
            WHEN c.COLUMN_NAME = 'unlock_at' THEN '计划解禁时间'
            WHEN c.COLUMN_NAME = 'merge_key' THEN '锁仓合并唯一键'
            WHEN c.COLUMN_NAME = 'unlock_price' THEN '解禁计价价格'
            WHEN c.COLUMN_NAME = 'unlock_fee_enabled' THEN '是否收取解禁手续费'
            WHEN c.COLUMN_NAME = 'unlock_fee_basis' THEN '解禁手续费计费基准'
            WHEN c.COLUMN_NAME = 'fee_paid_status' THEN '手续费支付状态'
            WHEN c.COLUMN_NAME = 'tx_hash' THEN '链上交易哈希'
            WHEN c.COLUMN_NAME = 'address' THEN '提现或链上地址'
            WHEN c.COLUMN_NAME = 'network' THEN '提现网络'
            WHEN c.COLUMN_NAME = 'rest_base_url' THEN '行情源 REST 基础地址'
            WHEN c.COLUMN_NAME = 'ws_url' THEN '行情源 WebSocket 地址'
            WHEN c.COLUMN_NAME = 'priority' THEN '调用优先级'
            WHEN c.COLUMN_NAME = 'strategy_type' THEN '行情策略类型'
            WHEN c.COLUMN_NAME = 'start_time' THEN '策略开始时间'
            WHEN c.COLUMN_NAME = 'end_time' THEN '策略结束时间'
            WHEN c.COLUMN_NAME = 'volatility' THEN '策略波动率参数'
            WHEN c.COLUMN_NAME = 'volume_min' THEN '策略最小成交量'
            WHEN c.COLUMN_NAME = 'volume_max' THEN '策略最大成交量'
            WHEN c.COLUMN_NAME = 'run_status' THEN '策略运行状态'
            WHEN c.COLUMN_NAME = 'last_tick_at' THEN '最后行情 tick 时间'
            WHEN c.COLUMN_NAME = 'last_generated_at' THEN '最后生成行情时间'
            WHEN c.COLUMN_NAME = 'last_kline_open_time' THEN '最后 K 线开盘时间'
            WHEN c.COLUMN_NAME = 'recovery_status' THEN '策略恢复状态'
            WHEN c.COLUMN_NAME = 'error_message' THEN '错误信息'
            WHEN c.COLUMN_NAME = 'version' THEN '配置或策略版本号'
            WHEN c.COLUMN_NAME = 'effective_time' THEN '版本生效时间'
            WHEN c.COLUMN_NAME = 'seed' THEN '策略随机种子'
            WHEN c.COLUMN_NAME = 'event_type' THEN '事件类型'
            WHEN c.COLUMN_NAME = 'payload_json' THEN '事件或业务载荷 JSON'
            WHEN c.COLUMN_NAME = 'lifecycle_status' THEN '新币生命周期状态'
            WHEN c.COLUMN_NAME = 'total_supply' THEN '新币总发行量'
            WHEN c.COLUMN_NAME = 'listed_at' THEN '计划或实际上市时间'
            WHEN c.COLUMN_NAME = 'fixed_unlock_at' THEN '固定解禁时间'
            WHEN c.COLUMN_NAME = 'relative_unlock_seconds' THEN '相对解禁延迟秒数'
            WHEN c.COLUMN_NAME = 'post_listing_purchase_enabled' THEN '是否允许上市后认购'
            WHEN c.COLUMN_NAME = 'post_listing_pair_id' THEN '上市后认购使用的交易对 ID'
            WHEN c.COLUMN_NAME = 'pricing_mode' THEN '闪兑定价模式'
            WHEN c.COLUMN_NAME = 'min_amount' THEN '最小兑换数量'
            WHEN c.COLUMN_NAME = 'max_amount' THEN '最大兑换数量；为空表示无上限'
            WHEN c.COLUMN_NAME = 'rate_source' THEN '汇率来源'
            WHEN c.COLUMN_NAME = 'floating_rate_json' THEN '浮动汇率配置 JSON'
            WHEN c.COLUMN_NAME = 'quote_id' THEN '闪兑报价 ID'
            WHEN c.COLUMN_NAME = 'aggregate_type' THEN '事件聚合类型'
            WHEN c.COLUMN_NAME = 'routing_key' THEN '事件路由键'
            WHEN c.COLUMN_NAME = 'retry_count' THEN '重试次数'
            WHEN c.COLUMN_NAME = 'next_retry_at' THEN '下次重试时间'
            WHEN c.COLUMN_NAME = 'consumer_name' THEN '事件消费者名称'
            WHEN c.COLUMN_NAME = 'message_id' THEN '外部消息 ID'
            WHEN c.COLUMN_NAME = 'rule_type' THEN '风控规则类型'
            WHEN c.COLUMN_NAME = 'config_json' THEN '规则或版本配置 JSON'
            WHEN c.COLUMN_NAME = 'risk_level' THEN '风险等级'
            WHEN c.COLUMN_NAME = 'decision' THEN '风控处置结果'
            WHEN c.COLUMN_NAME = 'operation_type' THEN '敏感操作类型'
            WHEN c.COLUMN_NAME = 'operation_id' THEN '敏感操作 ID'
            WHEN c.COLUMN_NAME = 'confirmation_type' THEN '确认方式'
            WHEN c.COLUMN_NAME = 'opened_at' THEN '开仓或订单开始时间'
            WHEN c.COLUMN_NAME = 'closed_at' THEN '平仓时间'
            WHEN c.COLUMN_NAME = 'settled_at' THEN '结算完成时间'
            WHEN c.COLUMN_NAME = 'next_settlement_attempt_at' THEN '下次结算重试时间'
            WHEN c.COLUMN_NAME = 'realized_pnl' THEN '已实现盈亏'
            WHEN c.COLUMN_NAME = 'liquidated_at' THEN '强平时间'
            WHEN c.COLUMN_NAME = 'liquidation_reason' THEN '强平原因'
            WHEN c.COLUMN_NAME = 'next_liquidation_attempt_at' THEN '下次强平重试时间'
            WHEN c.COLUMN_NAME = 'interest_accrued_at' THEN '利息最后计提时间'
            WHEN c.COLUMN_NAME = 'symbols_json' THEN '订阅交易对列表 JSON'
            WHEN c.COLUMN_NAME = 'intervals_json' THEN '订阅周期列表 JSON'
            WHEN c.COLUMN_NAME = 'providers_json' THEN '行情提供方列表 JSON'
            WHEN c.COLUMN_NAME = 'applied_version' THEN '已应用配置版本号'
            WHEN c.COLUMN_NAME = 'last_reload_status' THEN '最后重载状态'
            WHEN c.COLUMN_NAME = 'last_reload_error' THEN '最后重载错误'
            WHEN c.COLUMN_NAME = 'provider' THEN '服务提供商标识'
            WHEN c.COLUMN_NAME = 'auth_type' THEN '认证方式'
            WHEN c.COLUMN_NAME = 'api_key_ciphertext' THEN 'API Key 加密密文'
            WHEN c.COLUMN_NAME = 'api_secret_ciphertext' THEN 'API Secret 加密密文'
            WHEN c.COLUMN_NAME = 'passphrase_ciphertext' THEN 'API Passphrase 加密密文'
            WHEN c.COLUMN_NAME = 'api_key_mask' THEN 'API Key 脱敏展示值'
            WHEN c.COLUMN_NAME = 'host' THEN 'SMTP 主机'
            WHEN c.COLUMN_NAME = 'port' THEN 'SMTP 端口'
            WHEN c.COLUMN_NAME = 'security' THEN 'SMTP 加密方式'
            WHEN c.COLUMN_NAME = 'username_ciphertext' THEN 'SMTP 用户名加密密文'
            WHEN c.COLUMN_NAME = 'username_mask' THEN 'SMTP 用户名脱敏展示值'
            WHEN c.COLUMN_NAME = 'from_email' THEN '发件邮箱'
            WHEN c.COLUMN_NAME = 'from_name' THEN '发件人名称'
            WHEN c.COLUMN_NAME = 'verification_code_template_html' THEN '默认验证码 HTML 模板'
            WHEN c.COLUMN_NAME = 'verification_code_templates_json' THEN '多验证码模板 JSON'
            WHEN c.COLUMN_NAME = 'strategy' THEN '发信选择策略'
            WHEN c.COLUMN_NAME = 'round_robin_cursor' THEN '轮询发信游标'
            WHEN c.COLUMN_NAME = 'endpoint' THEN '上传服务接口地址'
            WHEN c.COLUMN_NAME = 'file_field' THEN '上传接口文件字段名'
            WHEN c.COLUMN_NAME = 'bearer_token_ciphertext' THEN 'Bearer Token 加密密文'
            WHEN c.COLUMN_NAME = 'bearer_token_mask' THEN 'Bearer Token 脱敏展示值'
            WHEN c.COLUMN_NAME = 'access_key_ciphertext' THEN 'Access Key 加密密文'
            WHEN c.COLUMN_NAME = 'access_key_mask' THEN 'Access Key 脱敏展示值'
            WHEN c.COLUMN_NAME = 'secret_key_ciphertext' THEN 'Secret Key 加密密文'
            WHEN c.COLUMN_NAME = 'bucket' THEN '对象存储桶名称'
            WHEN c.COLUMN_NAME = 'region' THEN '对象存储区域'
            WHEN c.COLUMN_NAME = 'public_base_url' THEN '公开访问基础地址'
            WHEN c.COLUMN_NAME = 'local_root' THEN '本地存储根目录'
            WHEN c.COLUMN_NAME = 'key_prefix' THEN '对象 Key 前缀'
            WHEN c.COLUMN_NAME = 'max_file_size_bytes' THEN '允许上传的最大文件字节数'
            WHEN c.COLUMN_NAME = 'allowed_mime_types_json' THEN '允许上传的 MIME 类型 JSON'
            WHEN c.COLUMN_NAME = 'object_key' THEN '对象存储 Key'
            WHEN c.COLUMN_NAME = 'public_url' THEN '文件公开访问 URL'
            WHEN c.COLUMN_NAME = 'share_url' THEN '文件分享 URL'
            WHEN c.COLUMN_NAME = 'delete_url' THEN '文件删除 URL'
            WHEN c.COLUMN_NAME = 'mime_type' THEN '文件 MIME 类型'
            WHEN c.COLUMN_NAME = 'size_bytes' THEN '文件大小字节数'
            WHEN c.COLUMN_NAME = 'original_filename' THEN '上传原始文件名'
            WHEN RIGHT(c.COLUMN_NAME, 5) = '_json' THEN 'JSON 格式的业务配置或数据快照'
            WHEN RIGHT(c.COLUMN_NAME, 11) = '_ciphertext' THEN '加密保存的敏感信息密文'
            WHEN RIGHT(c.COLUMN_NAME, 5) = '_mask' THEN '敏感信息脱敏展示值'
            WHEN RIGHT(c.COLUMN_NAME, 4) = '_url' THEN '外部访问地址'
            WHEN RIGHT(c.COLUMN_NAME, 5) = '_hash' THEN '哈希摘要值'
            WHEN RIGHT(c.COLUMN_NAME, 8) = '_enabled' THEN '是否启用该能力'
            WHEN RIGHT(c.COLUMN_NAME, 7) = '_status' THEN '对应业务流程状态'
            WHEN RIGHT(c.COLUMN_NAME, 5) = '_type' THEN '业务类型'
            WHEN RIGHT(c.COLUMN_NAME, 3) = '_id' THEN '关联业务记录 ID'
            WHEN RIGHT(c.COLUMN_NAME, 3) = '_at' THEN '业务时间点'
            WHEN RIGHT(c.COLUMN_NAME, 7) = '_amount' THEN '业务金额或数量'
            WHEN RIGHT(c.COLUMN_NAME, 5) = '_rate' THEN '业务比例或费率'
            WHEN RIGHT(c.COLUMN_NAME, 6) = '_price' THEN '业务价格'
            WHEN RIGHT(c.COLUMN_NAME, 9) = '_quantity' THEN '业务数量'
            WHEN RIGHT(c.COLUMN_NAME, 6) = '_asset' THEN '关联资产 ID'
            WHEN RIGHT(c.COLUMN_NAME, 6) = '_count' THEN '统计次数'
            WHEN RIGHT(c.COLUMN_NAME, 4) = '_key' THEN '业务唯一键'
            ELSE '业务字段，用于支撑该表对应功能'
        END
    ) AS column_comment
FROM information_schema.COLUMNS c
JOIN schema_comment_targets t ON t.table_name = c.TABLE_NAME
WHERE c.TABLE_SCHEMA = DATABASE();

CREATE TEMPORARY TABLE schema_column_comment_sqls AS
SELECT
    c.TABLE_NAME AS table_name,
    CONCAT(
        'ALTER TABLE `',
        c.TABLE_NAME,
        '` ',
        GROUP_CONCAT(
            CONCAT(
                'MODIFY COLUMN `',
                c.COLUMN_NAME,
                '` ',
                c.COLUMN_TYPE,
                IF(
                    c.CHARACTER_SET_NAME IS NOT NULL,
                    CONCAT(' CHARACTER SET ', c.CHARACTER_SET_NAME),
                    ''
                ),
                IF(
                    c.COLLATION_NAME IS NOT NULL,
                    CONCAT(' COLLATE ', c.COLLATION_NAME),
                    ''
                ),
                IF(c.IS_NULLABLE = 'NO', ' NOT NULL', ' NULL'),
                CASE
                    WHEN c.COLUMN_DEFAULT IS NULL THEN ''
                    WHEN UPPER(c.COLUMN_DEFAULT) LIKE 'CURRENT_TIMESTAMP%' THEN CONCAT(' DEFAULT ', c.COLUMN_DEFAULT)
                    WHEN c.DATA_TYPE IN (
                        'bit',
                        'tinyint',
                        'smallint',
                        'mediumint',
                        'int',
                        'integer',
                        'bigint',
                        'decimal',
                        'float',
                        'double',
                        'real'
                    ) THEN CONCAT(' DEFAULT ', c.COLUMN_DEFAULT)
                    ELSE CONCAT(' DEFAULT ', QUOTE(c.COLUMN_DEFAULT))
                END,
                IF(
                    TRIM(REPLACE(c.EXTRA, 'DEFAULT_GENERATED', '')) <> '',
                    CONCAT(' ', TRIM(REPLACE(c.EXTRA, 'DEFAULT_GENERATED', ''))),
                    ''
                ),
                ' COMMENT ',
                QUOTE(m.column_comment)
            )
            ORDER BY c.ORDINAL_POSITION
            SEPARATOR ', '
        )
    ) AS statement_sql
FROM information_schema.COLUMNS c
JOIN schema_column_comment_targets m
    ON m.table_name = c.TABLE_NAME AND m.column_name = c.COLUMN_NAME
WHERE c.TABLE_SCHEMA = DATABASE()
GROUP BY c.TABLE_NAME;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'users'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'user_security'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'refresh_tokens'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'admin_roles'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'admin_users'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'admin_audit_logs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'agents'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'agent_admin_users'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'invite_codes'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'user_referrals'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'agent_audit_logs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'agent_commission_rules'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'agent_commission_records'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'assets'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'wallet_accounts'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'wallet_ledger'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'asset_lock_positions'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'asset_lock_position_sources'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'asset_unlock_records'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'deposit_records'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'withdraw_records'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'trading_pairs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'market_sources'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'market_strategies'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'strategy_runs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'strategy_versions'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'strategy_events'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'spot_orders'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'spot_trades'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'order_events'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'new_coin_projects'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'new_coin_subscriptions'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'new_coin_distributions'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'new_coin_purchase_orders'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'new_coin_lifecycle_events'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'convert_pairs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'new_coin_convert_rules'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'convert_quotes'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'convert_orders'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'convert_events'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'event_outbox'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'event_inbox'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'risk_rules'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'risk_events'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'sensitive_operation_confirmations'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'audit_events'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'seconds_contract_products'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'seconds_contract_orders'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'margin_products'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'margin_positions'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'earn_products'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'earn_subscriptions'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'margin_liquidation_records'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'market_feed_configs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'market_source_credentials'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'user_email_verifications'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'smtp_configs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'upload_storage_configs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'upload_objects'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'admin_news_items'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'country_configs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'user_two_factor_settings'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'security_policy_configs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'login_two_factor_challenges'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'wallet_withdrawal_requests'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'kyc_configs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'user_kyc_submissions'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'platform_brand_configs'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET @sql := COALESCE((SELECT statement_sql FROM schema_column_comment_sqls WHERE table_name = 'smtp_delivery_settings'), 'SELECT 1');
PREPARE schema_comment_stmt FROM @sql;
EXECUTE schema_comment_stmt;
DEALLOCATE PREPARE schema_comment_stmt;

SET FOREIGN_KEY_CHECKS = @schema_comment_old_foreign_key_checks;
SET SESSION group_concat_max_len = @schema_comment_old_group_concat_max_len;
