//! quick_recharge bounded context service layer.
//!
//! 服务层：第三方支付快速充值的纯业务规则集合，覆盖渠道配置校验、商户密钥的加解密搬运、
//! 充值金额区间判定、回跳地址选择，以及 GMPay 签名的生成与回调验签。
//!
//! 签名口径是本文件最需要保持一致的部分：下单请求与回调验签共用 `gmpay_signature`，
//! 规则为剔除 `signature` 字段、剔除空白值字段，按键名字典序拼成 `k=v&k=v` 后紧接商户密钥求 MD5。
//! 金额参与签名时必须先经 `decimal_to_gmpay_string` 归一，否则同一金额的不同文本写法会算出不同签名。
//! 回调验签用不区分大小写的普通字符串比较，并非常量时间比较，这一点是已知取舍。
//!
//! 安全约束：商户密钥在配置行中以密文存储，只有 `runtime_config_from_row` 会在需要发起支付请求时解密；
//! 审计快照一律只写掩码与「是否已设置」布尔量，密文与明文都不得进入审计 JSON 或日志。
//! 本文件不访问数据库、不发起 HTTP 请求、不修改订单状态，所有校验都发生在建单与外部调用之前，
//! 因此校验失败不会留下本地订单或外部副作用。

use super::{
    presentation::{
        QuickRechargeReturnTarget, SaveQuickRechargeConfigRequest, TestQuickRechargeConfigResponse,
    },
    repository::QuickRechargeConfigRow,
};
use crate::{
    error::{AppError, AppResult},
    infra::secrets::{decrypt_secret, encrypt_secret_field},
};
use bigdecimal::BigDecimal;
use md5::{Digest, Md5};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, str::FromStr};
use url::Url;

/// 可直接用于发起支付请求的渠道运行时配置，由持久化配置行解密并补齐必填项后得到。
/// 与存储形态的关键差别是必填项已从 `Option` 收敛为确定值，商户密钥已是明文，
/// 因此该结构只应在发起请求或验签的短暂作用域内存在，禁止写入日志、审计或响应体。
#[derive(Debug, Clone)]
pub(crate) struct QuickRechargeRuntimeConfig {
    /// 支付方 API 根地址，只允许 http 或 https。
    pub(crate) api_base_url: String,
    /// 商户号，随请求参数一起参与签名。
    pub(crate) merchant_pid: String,
    /// 商户密钥明文，仅用于拼接签名源串，绝不外发也不入库。
    pub(crate) merchant_secret: String,
    /// 计价法币代码，已归一为小写。
    pub(crate) currency: String,
    /// 收款币种代码，已归一为小写。
    pub(crate) token: String,
    /// 收款链网络标识，已归一为小写。
    pub(crate) network: String,
    /// 异步回调地址，支付方据此推送支付结果通知。
    pub(crate) notify_url: String,
    /// 通用同步回跳地址，在没有匹配到端侧专用地址时兜底使用。
    pub(crate) redirect_url: Option<String>,
    /// PC 客户端专用回跳地址。
    pub(crate) pc_app_redirect_url: Option<String>,
    /// macOS 客户端专用回跳地址。
    pub(crate) mac_app_redirect_url: Option<String>,
    /// iOS 客户端专用回跳地址。
    pub(crate) ios_app_redirect_url: Option<String>,
    /// Android 客户端专用回跳地址。
    pub(crate) android_app_redirect_url: Option<String>,
    /// 移动端网页专用回跳地址。
    pub(crate) mobile_web_redirect_url: Option<String>,
    /// 桌面端网页专用回跳地址。
    pub(crate) desktop_web_redirect_url: Option<String>,
    /// 单笔充值最小法币金额，必为正数。
    pub(crate) min_amount: BigDecimal,
    /// 单笔充值最大法币金额，`None` 表示不设上限。
    pub(crate) max_amount: Option<BigDecimal>,
}

/// 通过校验、等待写库的渠道配置，字段已完成裁剪、大小写归一和格式检查。
/// 这里不含商户密钥的任何形态：密钥走 `prepare_secret_field` 单独处理，
/// 使「校验配置」与「更换密钥」两条路径互不牵连，未填新密钥时旧密钥保持生效。
/// 启用状态下必填的三项仍为 `Option`，因为字段本身允许为空，只是启用时会被额外断言非空。
#[derive(Debug)]
pub(crate) struct ValidatedQuickRechargeConfig {
    /// 渠道是否启用；为真时会额外要求 API 地址、商户号与回调地址齐备。
    pub(crate) enabled: bool,
    /// 支付方 API 根地址，必须是 http 或 https。
    pub(crate) api_base_url: Option<String>,
    /// 商户号，只允许字母数字与下划线，长度不超过 128。
    pub(crate) merchant_pid: Option<String>,
    /// 计价法币代码，已归一为小写，必填。
    pub(crate) currency: String,
    /// 收款币种代码，已归一为小写，必填。
    pub(crate) token: String,
    /// 收款链网络标识，已归一为小写，允许包含短横线。
    pub(crate) network: String,
    /// 异步回调地址，必须是 http 或 https。
    pub(crate) notify_url: Option<String>,
    /// 通用同步回跳地址。
    pub(crate) redirect_url: Option<String>,
    /// PC 客户端回跳地址，允许自定义 URL Scheme 以便唤起本地应用。
    pub(crate) pc_app_redirect_url: Option<String>,
    /// macOS 客户端回跳地址，同样允许自定义 Scheme。
    pub(crate) mac_app_redirect_url: Option<String>,
    /// iOS 客户端回跳地址，同样允许自定义 Scheme。
    pub(crate) ios_app_redirect_url: Option<String>,
    /// Android 客户端回跳地址，同样允许自定义 Scheme。
    pub(crate) android_app_redirect_url: Option<String>,
    /// 移动端网页回跳地址，只允许 http 或 https。
    pub(crate) mobile_web_redirect_url: Option<String>,
    /// 桌面端网页回跳地址，只允许 http 或 https。
    pub(crate) desktop_web_redirect_url: Option<String>,
    /// 单笔最小充值金额，必须为正数。
    pub(crate) min_amount: BigDecimal,
    /// 单笔最大充值金额，给出时不得小于最小金额。
    pub(crate) max_amount: Option<BigDecimal>,
}

/// 把持久化配置行解密并收敛成可直接调用支付方的运行时配置，是商户密钥唯一的解密入口。
/// `require_enabled` 为真时先断言渠道已启用，用于用户发起充值的路径；后台连通性测试传假，
/// 使未上线的渠道也能被管理员先行验证。
/// 随后逐项要求 API 地址、商户号、密钥密文与回调地址齐备，任一缺失返回 `AppError::Validation`；
/// 加密主密钥未配置则返回 `AppError::Internal`，因为那属于部署问题而非渠道配置问题。
/// 解密在最后一步进行，因此前置校验失败时密钥根本不会被解开。
/// 全部检查都发生在任何外部请求之前，失败不会产生本地订单，也不会向支付方发出任何调用。
pub(crate) fn runtime_config_from_row(
    row: QuickRechargeConfigRow,
    key: Option<&str>,
    require_enabled: bool,
) -> AppResult<QuickRechargeRuntimeConfig> {
    if require_enabled && !row.enabled {
        return Err(AppError::Validation(
            "quick recharge is not enabled".to_owned(),
        ));
    }
    let api_base_url = row.api_base_url.ok_or_else(|| {
        AppError::Validation("quick recharge api_base_url is not configured".to_owned())
    })?;
    let merchant_pid = row.merchant_pid.ok_or_else(|| {
        AppError::Validation("quick recharge merchant_pid is not configured".to_owned())
    })?;
    let secret_ciphertext = row.merchant_secret_ciphertext.ok_or_else(|| {
        AppError::Validation("quick recharge merchant_secret is not configured".to_owned())
    })?;
    let key = key.ok_or_else(|| {
        AppError::Internal("credential encryption key is not configured".to_owned())
    })?;
    let notify_url = row.notify_url.ok_or_else(|| {
        AppError::Validation("quick recharge notify_url is not configured".to_owned())
    })?;
    Ok(QuickRechargeRuntimeConfig {
        api_base_url,
        merchant_pid,
        merchant_secret: decrypt_secret(&secret_ciphertext, key)?,
        currency: row.currency,
        token: row.token,
        network: row.network,
        notify_url,
        redirect_url: row.redirect_url,
        pc_app_redirect_url: row.pc_app_redirect_url,
        mac_app_redirect_url: row.mac_app_redirect_url,
        ios_app_redirect_url: row.ios_app_redirect_url,
        android_app_redirect_url: row.android_app_redirect_url,
        mobile_web_redirect_url: row.mobile_web_redirect_url,
        desktop_web_redirect_url: row.desktop_web_redirect_url,
        min_amount: row.min_amount,
        max_amount: row.max_amount,
    })
}

/// 规范并校验后台提交的整份渠道配置，产出可直接写库的中间结构。
/// 地址类字段分两档把关：API 地址、回调地址与网页回跳地址只允许 http 或 https，
/// 而四个客户端回跳地址走 `validate_optional_return_url`，额外放行自定义 URL Scheme 以便唤起本地应用，
/// 但显式拒绝 `javascript`、`data`、`file`、`about` 这几种可被用于钓鱼或本地文件访问的协议。
/// 商户号只允许字母数字与下划线；币种、收款币种、网络按 ASCII 记号校验后统一转小写，其中网络额外允许短横线。
/// 金额规则为最小值必须为正、最大值给出时不得小于最小值；当前不限制法币金额的小数位与整数位。
/// 最后在 `enabled` 为真时补一轮必填断言，要求 API 地址、商户号与回调地址齐备，
/// 因此一个启用中的渠道不可能缺少发起支付所需的基础字段。
/// 本函数是纯校验，不加密密钥、不访问支付方、不读写数据库，也不会改动当前生效的配置。
pub(crate) fn validate_save_config_request(
    request: &SaveQuickRechargeConfigRequest,
) -> AppResult<ValidatedQuickRechargeConfig> {
    let api_base_url = validate_optional_url(request.api_base_url.clone(), "api_base_url")?;
    let notify_url = validate_optional_url(request.notify_url.clone(), "notify_url")?;
    let redirect_url = validate_optional_url(request.redirect_url.clone(), "redirect_url")?;
    let pc_app_redirect_url =
        validate_optional_return_url(request.pc_app_redirect_url.clone(), "pc_app_redirect_url")?;
    let mac_app_redirect_url =
        validate_optional_return_url(request.mac_app_redirect_url.clone(), "mac_app_redirect_url")?;
    let ios_app_redirect_url =
        validate_optional_return_url(request.ios_app_redirect_url.clone(), "ios_app_redirect_url")?;
    let android_app_redirect_url = validate_optional_return_url(
        request.android_app_redirect_url.clone(),
        "android_app_redirect_url",
    )?;
    let mobile_web_redirect_url = validate_optional_url(
        request.mobile_web_redirect_url.clone(),
        "mobile_web_redirect_url",
    )?;
    let desktop_web_redirect_url = validate_optional_url(
        request.desktop_web_redirect_url.clone(),
        "desktop_web_redirect_url",
    )?;
    let merchant_pid = validate_optional_ascii(
        request.merchant_pid.clone(),
        "merchant_pid",
        128,
        false,
        true,
    )?;
    let currency = validate_symbol_like(&request.currency, "currency", 16, false)?;
    let token = validate_symbol_like(&request.token, "token", 32, false)?;
    let network = validate_symbol_like(&request.network, "network", 32, true)?;
    let min_amount = request.min_amount.clone();
    if min_amount <= 0 {
        return Err(AppError::Validation(
            "quick recharge min_amount must be positive".to_owned(),
        ));
    }
    let max_amount = request.max_amount.clone();
    if let Some(max_amount) = max_amount.as_ref()
        && max_amount < &min_amount
    {
        return Err(AppError::Validation(
            "quick recharge max_amount must be greater than or equal to min_amount".to_owned(),
        ));
    }
    let config = ValidatedQuickRechargeConfig {
        enabled: request.enabled,
        api_base_url,
        merchant_pid,
        currency,
        token,
        network,
        notify_url,
        redirect_url,
        pc_app_redirect_url,
        mac_app_redirect_url,
        ios_app_redirect_url,
        android_app_redirect_url,
        mobile_web_redirect_url,
        desktop_web_redirect_url,
        min_amount,
        max_amount,
    };
    if config.enabled {
        require_config_field(config.api_base_url.as_deref(), "api_base_url")?;
        require_config_field(config.merchant_pid.as_deref(), "merchant_pid")?;
        require_config_field(config.notify_url.as_deref(), "notify_url")?;
    }
    Ok(config)
}

/// 断言渠道在启用状态下必定已配置商户密钥，未启用时不做要求以便草稿配置可以先行保存。
/// 只检查密文是否存在，既不解密也不验证该密钥能否被支付方接受，真实可用性需靠连通性测试验证。
/// 检查发生在配置写库之前，避免出现启用中却无法签名的渠道配置。
pub(crate) fn validate_enabled_config_secrets(
    config: &ValidatedQuickRechargeConfig,
    secret_ciphertext: &Option<String>,
) -> AppResult<()> {
    if config.enabled && secret_ciphertext.is_none() {
        return Err(AppError::Validation(
            "quick recharge merchant_secret is required when enabled".to_owned(),
        ));
    }
    Ok(())
}

/// 校验快充金额为正且处于配置的最小值和可选最大值之间。
/// 由于配置 min_amount 必须为正，低于最小值即涵盖非正输入；当前规则不按法币或到账资产精度截断。
/// 校验在创建本地订单和支付方请求前完成，失败时不产生本地订单或外部副作用。
pub(crate) fn validate_recharge_amount(
    amount: &BigDecimal,
    config: &QuickRechargeRuntimeConfig,
) -> AppResult<()> {
    if amount < &config.min_amount {
        return Err(AppError::Validation(
            "quick recharge amount is below min_amount".to_owned(),
        ));
    }
    if let Some(max_amount) = config.max_amount.as_ref()
        && amount > max_amount
    {
        return Err(AppError::Validation(
            "quick recharge amount is above max_amount".to_owned(),
        ));
    }
    Ok(())
}

/// 按客户端目标选择专用回跳地址，缺失时回退到默认地址。
/// 选择只基于已验证配置，不拼接用户输入，也不修改订单或支付方状态。
pub(crate) fn redirect_url_for_target(
    config: &QuickRechargeRuntimeConfig,
    target: Option<QuickRechargeReturnTarget>,
) -> Option<String> {
    let target_url = target.and_then(|target| match target {
        QuickRechargeReturnTarget::PcApp => config.pc_app_redirect_url.clone(),
        QuickRechargeReturnTarget::MacApp => config.mac_app_redirect_url.clone(),
        QuickRechargeReturnTarget::IosApp => config.ios_app_redirect_url.clone(),
        QuickRechargeReturnTarget::AndroidApp => config.android_app_redirect_url.clone(),
        QuickRechargeReturnTarget::MobileWeb => config.mobile_web_redirect_url.clone(),
        QuickRechargeReturnTarget::DesktopWeb => config.desktop_web_redirect_url.clone(),
    });
    target_url.or_else(|| config.redirect_url.clone())
}

/// 新密钥非空时加密保存，空白时沿用旧密文；缺少加密键时返回配置错误。
/// 函数只返回待保存密文，不写数据库；配置事务失败时旧密钥仍保持生效。
pub(crate) fn prepare_secret_field(
    new_value: Option<&str>,
    existing_ciphertext: Option<String>,
    key: Option<&str>,
) -> AppResult<Option<String>> {
    if new_value.and_then(optional_str).is_some() {
        let key = key.ok_or_else(|| {
            AppError::Internal("credential encryption key is not configured".to_owned())
        })?;
        return encrypt_secret_field(key, new_value, existing_ciphertext);
    }
    Ok(existing_ciphertext)
}

/// 生成渠道配置的审计快照，供配置变更事务记录 before/after 双镜像。
/// 商户密钥在快照中只以两种脱敏形态出现：`merchant_secret_mask` 是入库时算好的掩码，
/// `merchant_secret_set` 只表明密钥是否已设置；密文与解密后的明文都绝不进入审计 JSON。
/// 金额字段经 `decimal_to_gmpay_string` 归一成无多余零的字符串，使前后镜像的文本表示可直接比对，
/// 不会因 `BigDecimal` 的标度差异把「未改动」显示成「已改动」。
/// 本函数只读入参构造 JSON，不落库也不脱敏其他字段，地址与商户号按原值记录。
pub(crate) fn config_audit_json(row: &QuickRechargeConfigRow) -> Value {
    json!({
        "id": row.id,
        "name": row.name,
        "provider": row.provider,
        "enabled": row.enabled,
        "api_base_url": row.api_base_url,
        "merchant_pid": row.merchant_pid,
        "merchant_secret_mask": row.merchant_secret_mask,
        "merchant_secret_set": row.merchant_secret_ciphertext.is_some(),
        "currency": row.currency,
        "token": row.token,
        "network": row.network,
        "notify_url": row.notify_url,
        "redirect_url": row.redirect_url,
        "pc_app_redirect_url": row.pc_app_redirect_url,
        "mac_app_redirect_url": row.mac_app_redirect_url,
        "ios_app_redirect_url": row.ios_app_redirect_url,
        "android_app_redirect_url": row.android_app_redirect_url,
        "mobile_web_redirect_url": row.mobile_web_redirect_url,
        "desktop_web_redirect_url": row.desktop_web_redirect_url,
        "min_amount": decimal_to_gmpay_string(&row.min_amount),
        "max_amount": row.max_amount.as_ref().map(decimal_to_gmpay_string),
        "updated_by": row.updated_by,
    })
}

/// 生成后台连通性测试结果的审计快照，记录一次真实建单往返的关键返回值。
/// 快照包含本地订单号与支付方交易号的对应关系，这组映射是日后排查掉单与对账的起点。
/// 法币金额与实际收款金额同样经 GMPay 字符串归一，便于与回调中的金额逐字符比对。
/// 快照只写支付方返回的公开信息，不含商户密钥、签名串或任何签名材料。
pub(crate) fn test_config_audit_json(response: &TestQuickRechargeConfigResponse) -> Value {
    json!({
        "order_id": response.order_id,
        "provider_trade_id": response.provider_trade_id,
        "currency": response.currency,
        "token": response.token,
        "network": response.network,
        "fiat_amount": decimal_to_gmpay_string(&response.fiat_amount),
        "actual_amount": decimal_to_gmpay_string(&response.actual_amount),
        "receive_address": response.receive_address,
        "payment_url": response.payment_url,
        "expiration_time": response.expiration_time,
        "tested_at": response.tested_at,
    })
}

/// 排除 `signature` 字段后按 GMPay 规则重算 MD5，并使用不区分 ASCII 大小写的普通字符串比较。
/// 该比较不是常量时间比较；验签失败发生在锁订单之前，不修改钱包、流水或支付状态。
pub(crate) fn verify_gmpay_notify_signature(
    object: &Map<String, Value>,
    secret: &str,
) -> AppResult<()> {
    let signature = required_json_string(object, "signature")?;
    let expected = gmpay_json_signature(object, secret);
    if !signature.eq_ignore_ascii_case(&expected) {
        return Err(AppError::Validation(
            "gmpay notify signature is invalid".to_owned(),
        ));
    }
    Ok(())
}

/// 按键名排序拼接非空字段与商户密钥，计算 GMPay MD5 签名。
/// 字段过滤和文本表示必须与通知验签共用，避免请求与回调规则漂移。
pub fn gmpay_signature(params: &BTreeMap<String, String>, secret: &str) -> String {
    let sign_source = params
        .iter()
        .filter(|(key, value)| key.as_str() != "signature" && !value.trim().is_empty())
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    md5_lower_hex(&format!("{sign_source}{secret}"))
}

/// 从回调 JSON 对象读取必填字段并转成签名口径的字符串表示，数字与布尔值也会被接受并转文本。
/// 字段缺失、为 null、为空白串或是数组与对象这类无法参与签名的结构，一律返回 `AppError::Validation`。
/// 取值口径与验签时的字段收集完全一致，保证读到的值就是参与签名计算的那个值。
pub(crate) fn required_json_string(object: &Map<String, Value>, field: &str) -> AppResult<String> {
    object
        .get(field)
        .and_then(json_value_to_sign_string)
        .ok_or_else(|| AppError::Validation(format!("gmpay notify {field} is required")))
}

/// 从回调 JSON 对象读取可选字段，取值口径与必填版本相同，只是缺失时返回 `None` 而非报错。
/// 空白串、null 以及数组与对象都会被归一为 `None`，使调用方无需再区分「未提供」与「提供了空值」。
pub(crate) fn optional_json_string(object: &Map<String, Value>, field: &str) -> Option<String> {
    object.get(field).and_then(json_value_to_sign_string)
}

/// 从回调对象解析必填金额字段，先按签名口径取出文本再转成 `BigDecimal`，全程不经过浮点。
/// 这一点对资金安全是必要的：回调金额要与本地订单金额做精确比对，任何二进制浮点舍入都可能让
/// 金额不一致的回调被误判为一致。
/// 字段缺失或文本无法解析成十进制数时返回 `AppError::Validation`，此处不校验金额正负与区间。
pub(crate) fn required_json_decimal(
    object: &Map<String, Value>,
    field: &str,
) -> AppResult<BigDecimal> {
    let value = required_json_string(object, field)?;
    BigDecimal::from_str(&value)
        .map_err(|_| AppError::Validation(format!("gmpay notify {field} is invalid")))
}

/// 把金额标准化为 GMPay 请求与签名统一使用的十进制文本，是签名一致性的关键前置步骤。
/// 先按固定 18 位小数展开以抹平 `BigDecimal` 内部标度差异，再逐位去掉小数部分的尾随零，
/// 若小数点后被清空则连小数点一并去掉，因此同一数值无论标度如何都会得到唯一文本。
/// 负零被显式规整为 `0`，避免出现 `-0` 这种支付方无法识别的写法。
/// 下单请求与回调验签必须复用本函数，否则相同金额的不同文本形态会算出不同 MD5 而导致验签失败。
/// 注意 18 位展开意味着超过 18 位小数的部分会被截断，法币金额在实际业务中不会触达该边界。
pub(crate) fn decimal_to_gmpay_string(value: &BigDecimal) -> String {
    let mut text = format!("{value:.18}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        return "0".to_owned();
    }
    text
}

/// 规范快充订单状态，只接受本地状态机定义的值。
/// 非法状态在查询执行前拒绝，不会被当作空结果或触发任何订单变更。
pub(crate) fn validate_order_status(value: &str) -> AppResult<String> {
    let status = value.trim();
    match status {
        "created" | "pending" | "paid" | "failed" | "expired" => Ok(status.to_owned()),
        _ => Err(AppError::Validation(
            "quick recharge status is invalid".to_owned(),
        )),
    }
}

/// 把持有所有权的可选文本裁剪空白，裁剪后为空串的降级为 `None`。
/// 用于配置字段与列表筛选项，避免前端提交的空白串被当成有效配置写库或被当成筛选条件。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| optional_str(&value).map(str::to_owned))
}

/// 裁剪借用文本并把纯空白归一为 `None`，返回的是原串的切片而不复制内容。
/// 是本模块「空白等同于未提供」这一约定的底层实现，配置校验与签名字段收集都经由它统一口径。
pub(crate) fn optional_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

/// 取出并校验管理员操作原因，保证每次渠道配置变更或删除都留下可追责的说明。
/// 缺省、空串或纯空白都按缺失处理并返回 `AppError::Validation`；长度按字节数限制在 512 以内，
/// 中文原因的可写字数因此少于 512 字。校验在开启配置事务之前完成，失败不会留下半截写入。
pub(crate) fn required_reason(value: Option<String>) -> AppResult<String> {
    let Some(reason) = optional_string(value) else {
        return Err(AppError::Validation("reason is required".to_owned()));
    };
    if reason.len() > 512 {
        return Err(AppError::Validation("reason is too long".to_owned()));
    }
    Ok(reason)
}

/// 从鉴权令牌 subject 解析发起充值的用户编号，要求形如 `user:{数字}`。
/// 充值订单的归属人只由此确定，请求体中的任何用户字段都不参与，杜绝替他人充值或篡改入账对象。
/// 前缀不符或数字解析失败一律返回 `AppError::Unauthorized`。
pub(crate) fn user_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("user:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 从鉴权令牌 subject 解析后台管理员编号，要求形如 `admin:{数字}`，用于审计日志的操作人字段。
/// 前缀不符或数字解析失败返回 `AppError::Unauthorized`，确保普通用户令牌命中不了渠道配置与密钥变更接口。
pub(crate) fn admin_id_from_subject(subject: &str) -> AppResult<u64> {
    subject
        .strip_prefix("admin:")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AppError::Unauthorized)
}

/// 归一化快充列表接口的分页条数，缺省取 50 并夹在 1 到 200 之间。
/// 上限比其他模块宽，是为了让后台一次翻查更多充值订单；下限 1 用于挡掉零条这类无意义请求。
pub(crate) fn route_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).clamp(1, 200)
}

/// 归一化快充列表接口的分页偏移，缺省取 0 并截断到 100000。
/// 偏移同样设上限：充值订单表随时间持续增长，超大 offset 会退化为全表扫描加文件排序。
/// 超限只做静默截断而不报错，深翻页应改用按时间范围筛选。
pub(crate) fn route_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(100_000)
}

/// 校验客户端专用回跳地址，与网页地址的区别是这里允许自定义 URL Scheme 以便支付完成后唤起本地应用。
/// 空白值归一为 `None`；地址必须能被解析成合法 URL，否则返回 `AppError::Validation`。
/// http 与 https 直接放行；`javascript`、`data`、`file`、`about` 显式拒绝，
/// 因为这几种协议可被用于在回跳时执行脚本或读取本地文件；其余非空 Scheme 视为应用自定义协议放行。
/// Scheme 为空的相对地址同样拒绝，避免拼接出指向本站的意外跳转。
/// 校验在配置写库前完成，不会向支付方注册任何地址。
pub(crate) fn validate_optional_return_url(
    value: Option<String>,
    field: &str,
) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    let parsed = Url::parse(&url)
        .map_err(|_| AppError::Validation(format!("quick recharge {field} is invalid")))?;
    match parsed.scheme() {
        "http" | "https" => Ok(Some(url)),
        "javascript" | "data" | "file" | "about" => Err(AppError::Validation(format!(
            "quick recharge {field} uses an unsupported scheme"
        ))),
        scheme if !scheme.is_empty() => Ok(Some(url)),
        _ => Err(AppError::Validation(format!(
            "quick recharge {field} requires a url scheme"
        ))),
    }
}

/// 校验只允许 http 或 https 的可选地址，用于 API 根地址、异步回调地址和网页回跳地址。
/// 空白值归一为 `None`；无法解析或 Scheme 不在白名单内返回 `AppError::Validation`。
/// 比客户端回跳地址严格，因为这几类地址会被服务端或浏览器直接访问，不存在唤起本地应用的需求。
fn validate_optional_url(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    let Some(url) = optional_string(value) else {
        return Ok(None);
    };
    let parsed = Url::parse(&url)
        .map_err(|_| AppError::Validation(format!("quick recharge {field} is invalid")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::Validation(format!(
            "quick recharge {field} must be http or https"
        )));
    }
    Ok(Some(url))
}

/// 对可选字段执行 ASCII 记号校验，空白值直接归一为 `None` 而不触发格式检查。
/// 与 `validate_symbol_like` 的差别是保留原始大小写，适用于商户号这类大小写敏感的凭据标识。
/// 校验失败时向上返回错误而非静默丢弃该字段。
fn validate_optional_ascii(
    value: Option<String>,
    field: &str,
    max_len: usize,
    allow_dash: bool,
    allow_underscore: bool,
) -> AppResult<Option<String>> {
    optional_string(value)
        .map(|value| validate_ascii_token(&value, field, max_len, allow_dash, allow_underscore))
        .transpose()
}

/// 校验并归一化币种、收款币种、网络这类符号型必填字段，始终允许下划线，短横线由参数控制。
/// 通过校验后统一转小写，使 `USDT` 与 `usdt` 落到同一存储值，避免同一渠道因大小写差异出现两套配置。
/// 空值或含非法字符时返回 `AppError::Validation`，不做静默替换。
fn validate_symbol_like(
    value: &str,
    field: &str,
    max_len: usize,
    allow_dash: bool,
) -> AppResult<String> {
    let normalized = validate_ascii_token(value, field, max_len, allow_dash, true)?;
    Ok(normalized.to_ascii_lowercase())
}

/// 校验一个 ASCII 记号型字段：裁剪空白后要求非空、长度不超限、字符集受控。
/// 允许的字符固定为 ASCII 字母与数字，短横线和下划线分别由两个开关控制，其余字符一律拒绝。
/// 字符集之所以收得这么紧，是因为这些值会直接进入签名源串与支付方请求参数，
/// 放行 `&`、`=` 等分隔符会让攻击者有机会构造出歧义的签名拼接串。
/// 长度按字节数比较，纯 ASCII 输入下与字符数等价；空值、超长、含非法字符分别返回不同的校验提示。
fn validate_ascii_token(
    value: &str,
    field: &str,
    max_len: usize,
    allow_dash: bool,
    allow_underscore: bool,
) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!(
            "quick recharge {field} is required"
        )));
    }
    if value.len() > max_len {
        return Err(AppError::Validation(format!(
            "quick recharge {field} is too long"
        )));
    }
    let valid = value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || (allow_dash && ch == '-') || (allow_underscore && ch == '_')
    });
    if !valid {
        return Err(AppError::Validation(format!(
            "quick recharge {field} format is invalid"
        )));
    }
    Ok(value.to_owned())
}

/// 断言渠道启用时的必填字段确实有值，空白串同样按缺失处理。
/// 只在 `enabled` 为真的分支调用，使草稿态配置可以留空这些字段而不影响保存。
fn require_config_field(value: Option<&str>, field: &str) -> AppResult<()> {
    if value.and_then(optional_str).is_none() {
        return Err(AppError::Validation(format!(
            "quick recharge {field} is required when enabled"
        )));
    }
    Ok(())
}

/// 把回调 JSON 对象折叠成参与签名的键值表并计算期望签名。
/// `signature` 字段本身被跳过，无法转成签名文本的字段（null、空白串、数组、对象）也被丢弃，
/// 使收集口径与下单请求侧完全一致，这样对同一组业务字段两端才能算出相同的 MD5。
/// 收集使用 `BTreeMap`，键名按字典序排列，因此回调字段的出现顺序不影响签名结果。
fn gmpay_json_signature(object: &Map<String, Value>, secret: &str) -> String {
    let mut params = BTreeMap::new();
    for (key, value) in object {
        if key == "signature" {
            continue;
        }
        if let Some(value) = json_value_to_sign_string(value) {
            params.insert(key.clone(), value);
        }
    }
    gmpay_signature(&params, secret)
}

/// 对 UTF-8 字节求 MD5 并输出小写十六进制文本，是 GMPay 签名算法要求的摘要形式。
/// MD5 由支付方协议规定，仅用于渠道约定的签名比对，不得挪作口令散列或其他安全用途。
fn md5_lower_hex(value: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

/// 把单个 JSON 值转成签名口径的文本，是回调字段读取与验签共用的唯一转换规则。
/// 字符串裁剪空白后为空则返回 `None`；数字按其 JSON 文本原样输出，不做补零或去零；
/// 布尔转成 `true` 或 `false`；null、数组与对象一律返回 `None` 表示不参与签名。
/// 数字保持原文本这一点很关键：签名比对依赖两端对同一数值给出完全相同的字符串。
fn json_value_to_sign_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => optional_str(value).map(str::to_owned),
        Value::Number(value) => Some(value.to_string()).filter(|value| !value.is_empty()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}
