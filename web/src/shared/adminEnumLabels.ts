export const ADMIN_FIELD_VALUE_LABELS: Record<string, Record<string, string>> = {
  environment: { production: '生产环境', staging: '预发布环境', development: '开发环境', test: '测试环境' },
  runtime_status: { error: '运行异常', healthy: '健康', running: '运行中', stopped: '已停止', unknown: '状态未知', not_applicable: '无需运行态' },
  config_status: { normal: '正常', pending_apply: '待应用', runtime_error: '运行异常', unconfigured: '未配置' },
  security: { none: '不加密', starttls: 'STARTTLS 加密', tls: 'TLS/SSL 加密' },
  rate_source: { fixed: '固定汇率', market: '市场汇率', floating: '浮动汇率' },
  strategy_type: { price_path: '价格路径（OHLCV）' },
  scenario: { custom_path: '自定义路径', trend_up: '稳步上涨', trend_down: '缓慢下行', range: '区间震荡', high_volatility: '高波动', crash_recovery: '急跌修复', pump_then_dump: '拉升回落' },
  seed_mode: { auto: '自动', fixed: '固定' },
  volume_shape: { uniform: '均匀分布', trend: '随时间递增', bell: '中段放量', end_spike: '尾段放量' },
  execution_mode: { hard: '硬命中', soft: '软命中', range: '范围命中' },
  target_type: { absolute_price: '绝对价格', percent_from_start: '相对起始价百分比', percent_from_previous: '相对上一节点百分比' },
  settlement_mode: { manual_distribution: '后台派发', legacy_instant: '历史即时成交', manual_confirm: '人工确认', auto: '自动结算' },
  refund_mode: { refund_stake_and_fee: '退本金及手续费', refund_stake_only: '只退本金', manual: '人工选择' },
  actor_type: { admin: '管理员', agent: '代理', user: '用户', system: '系统', worker: '后台任务' },
  role_name: { super_admin: '超级管理员', admin: '管理员', operator: '运营人员', auditor: '审计员', agent: '代理' },
  scope: { global: '全局范围', user: '指定用户', pair: '指定交易对', asset: '指定资产' },
  doc_type: { identity_card: '身份证', passport: '护照', driver_license: '驾驶证', residence_permit: '居住证' },
  liquidity_role: { maker: '挂单方', taker: '吃单方' },
  outcome: { yes: '是（YES）', no: '否（NO）', invalid: '无效' },
  fee_basis: { market_value: '按解禁市值计费', profit: '按解禁收益计费' },
  auth_mode: { none: '无需鉴权', api_key: '接口密钥鉴权' },
  recovery_status: { live: '实时运行', idle: '空闲', pending: '待恢复', recovering: '恢复中', failed: '恢复失败', completed: '恢复完成' },

  settlement_mode_override: {
    manual_confirm: '人工确认',
    auto: '自动结算'
  },
  display_status: {
    active: '显示',
    hidden: '隐藏'
  },
  settlement_status: {
    open: '开放下注',
    pending_confirmation: '待确认',
    settled: '已结算',
    refunded: '已退款'
  },
  interest_calculation_mode: {
    full_term: '完整周期计息',
    actual_days: '按实际天数计息'
  },
  loan_type: {
    credit: '信用贷',
    collateralized: '抵押贷'
  },
  return_target: {
    pc_app: 'PC 应用端',
    mac_app: 'Mac 应用端',
    ios_app: 'iOS 端',
    android_app: 'Android 端',
    mobile_web: '手机网页端',
    desktop_web: '电脑网页端'
  },
  ref_type: {
    manual: '人工记录',
    deposit_record: '充值记录',
    admin_recharge: '后台充值',
    quick_recharge: '快速充值',
    convert_order: '闪兑订单',
    spot_order: '现货委托',
    spot_trade: '现货成交',
    seconds_contract_order: '秒合约订单',
    margin_position: '杠杆仓位',
    earn_subscription: '理财订单',
    loan_order: '贷款订单',
    prediction_order: '竞猜订单',
    new_coin_subscription: '新币申购',
    new_coin_purchase: '新币购买',
    new_coin_distribution: '新币派发',
    new_coin_unlock: '新币解禁',
    agent_commission: '代理佣金'
  },
  change_type: {
    deposit: '充值',
    admin_recharge: '后台充值',
    quick_recharge: '快速充值',
    convert_settlement: '闪兑结算',
    spot_freeze: '现货委托冻结',
    spot_unfreeze: '现货委托解冻',
    spot_fill: '现货成交',
    spot_trade_settlement: '现货成交结算',
    spot_price_improvement_release: '现货差价释放',
    seconds_contract_open: '秒合约开仓',
    seconds_contract_settle_win: '秒合约盈利结算',
    margin_position_open: '杠杆开仓',
    margin_position_close: '杠杆平仓',
    margin_position_liquidate: '杠杆强平',
    earn_subscribe: '理财申购',
    earn_redeem: '理财赎回',
    loan_collateral_freeze: '贷款抵押冻结',
    loan_collateral_release: '贷款抵押释放',
    loan_disbursement: '贷款放款',
    loan_repayment: '贷款还款',
    prediction_stake_freeze: '竞猜下注冻结',
    prediction_fee: '竞猜手续费',
    prediction_settle_win: '竞猜盈利结算',
    prediction_settle_loss: '竞猜亏损结算',
    prediction_payout: '竞猜派彩',
    prediction_stake_refund: '竞猜本金退款',
    prediction_fee_refund: '竞猜手续费退款',
    new_coin_subscription_payment: '新币申购支付',
    new_coin_subscription_freeze: '新币申购冻结',
    new_coin_subscription_refund: '新币申购退款',
    new_coin_subscription_lock: '新币申购锁仓',
    new_coin_purchase_payment: '新币购买支付',
    new_coin_purchase_lock: '新币购买锁仓',
    new_coin_distribution_lock: '新币派发锁仓',
    new_coin_unlock_release: '新币解禁释放',
    asset_lock: '资产锁定',
    agent_commission_payout: '代理佣金发放'
  },
  asset_type: {
    coin: '数字货币',
    stablecoin: '稳定币',
    fiat: '法币',
    platform: '平台币'
  },
  balance_type: {
    available: '可用',
    frozen: '冻结',
    locked: '锁定'
  },
  category: {
    general: '通用资讯',
    market: '市场资讯',
    product: '产品资讯',
    system: '系统公告',
    promotion: '活动推广',
    fixed_term: '定期',
    flexible: '活期',
    structured: '结构化',
    staking: '质押'
  },
  decision: {
    allow: '放行',
    deny: '拒绝',
    review: '人工复核'
  },
  default_locale: {
    zh: '中文',
    'zh-cn': '简体中文',
    'zh-tw': '繁体中文',
    en: '英文',
    'en-us': '英文'
  },
  direction: {
    up: '看涨',
    down: '看跌',
    long: '做多',
    short: '做空'
  },
  fee_paid_status: {
    not_required: '无需支付',
    unpaid: '未支付',
    paid: '已支付'
  },
  early_redeem_fee_basis: {
    none: '不扣费',
    principal: '按本金比例扣除',
    profit: '按收益比例扣除'
  },
  lifecycle_status: {
    preheat: '预热中',
    subscription: '申购中',
    distribution: '派发中',
    listed: '已上市'
  },
  margin_mode: {
    isolated: '逐仓',
    cross: '全仓'
  },
  market_type: {
    external: '外部行情',
    internal: '内部撮合',
    strategy: '策略行情'
  },
  order_type: {
    limit: '限价',
    market: '市价'
  },
  pricing_mode: {
    fixed: '固定汇率',
    market: '市场汇率'
  },
  product_type: {
    convert: '闪兑',
    prediction: '竞猜',
    spot: '现货',
    margin: '杠杆',
    seconds_contract: '秒合约'
  },
  result: {
    pending: '待处理',
    win: '盈利',
    loss: '亏损'
  },
  risk_level: {
    low: '低风险',
    medium: '中风险',
    high: '高风险',
    critical: '严重风险'
  },
  run_status: {
    pending: '待处理',
    running: '运行中',
    completed: '已完成',
    failed: '失败',
    needs_reload: '待重载'
  },
  side: {
    buy: '买入',
    sell: '卖出'
  },
  source_type: {
    convert_order: '闪兑订单',
    prediction_order: '竞猜订单',
    spot_trade_buy: '现货成交(买)',
    spot_trade_sell: '现货成交(卖)',
    margin_position: '杠杆仓位',
    seconds_contract_order: '秒合约订单'
  },
  supported_locales: {
    zh: '中文',
    'zh-cn': '简体中文',
    'zh-tw': '繁体中文',
    en: '英文',
    'en-us': '英文'
  },
  unlock_type: {
    immediate_on_listing: '上市立即解禁',
    fixed_time: '固定时间解禁',
    relative_period: '相对周期解禁'
  }
};
