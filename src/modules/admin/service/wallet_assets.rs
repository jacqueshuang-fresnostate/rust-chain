use super::*;

/// 校验新资产符号、名称、精度、类型、状态、充提限额费率及阶梯提现费配置。
/// 不查询 symbol 唯一性或关联交易对；这些并发约束由创建事务与数据库唯一键保证。
pub(crate) fn validate_create_asset_request(request: &CreateAssetRequest) -> AppResult<()> {
    normalize_asset_symbol(&request.symbol)?;
    validate_asset_name(&request.name)?;
    validate_asset_precision(request.precision_scale)?;
    validate_optional_asset_amount(request.min_deposit_amount.as_ref(), "min_deposit_amount")?;
    validate_optional_asset_amount(request.deposit_fee.as_ref(), "deposit_fee")?;
    validate_optional_asset_amount(request.withdraw_fee.as_ref(), "withdraw_fee")?;
    validate_optional_withdraw_fee_tiers(request.withdraw_fee_tiers.as_deref())?;
    if let Some(asset_type) = request.asset_type.as_deref() {
        validate_asset_type(asset_type)?;
    }
    if let Some(status) = request.status.as_deref() {
        validate_asset_status(status)?;
    }
    Ok(())
}

/// 校验资产更新中的名称、精度、类型、状态、充提限额费率、阶梯费及审计原因。
/// 这里只校验目标配置快照；变更精度或删除资产的引用安全由应用事务锁定后确认。
pub(crate) fn validate_update_asset_request(request: &UpdateAssetRequest) -> AppResult<()> {
    validate_asset_name(&request.name)?;
    validate_asset_precision(request.precision_scale)?;
    validate_asset_type(&request.asset_type)?;
    validate_asset_status(&request.status)?;
    validate_optional_asset_amount(request.min_deposit_amount.as_ref(), "min_deposit_amount")?;
    validate_optional_asset_amount(request.deposit_fee.as_ref(), "deposit_fee")?;
    validate_optional_asset_amount(request.withdraw_fee.as_ref(), "withdraw_fee")?;
    validate_optional_withdraw_fee_tiers(request.withdraw_fee_tiers.as_deref())?;
    required_admin_audit_reason(request.reason.clone())?;
    Ok(())
}

/// 校验最小充值额、充值费和固定提现费均为非负值；不执行资产精度截断或阶梯费匹配。
pub(crate) fn validate_asset_fee_settings(
    min_deposit_amount: &BigDecimal,
    deposit_fee: &BigDecimal,
    withdraw_fee: &BigDecimal,
) -> AppResult<()> {
    validate_asset_amount(min_deposit_amount, "min_deposit_amount")?;
    validate_asset_amount(deposit_fee, "deposit_fee")?;
    validate_asset_amount(withdraw_fee, "withdraw_fee")
}

/// 规范化资产提现阶梯费配置并返回按共享钱包规则校验后的阶梯集合。
/// 金额边界、费率形状或阶梯重叠等错误转换为后台校验错误；函数不读取资产精度或保存配置。
pub(crate) fn normalize_asset_withdraw_fee_tiers(
    tiers: Vec<WithdrawFeeTier>,
) -> AppResult<Vec<WithdrawFeeTier>> {
    normalize_withdraw_fee_tiers(tiers).map_err(AppError::Validation)
}

/// 去除资产名称首尾空白并限制数据库列长度；空名称返回校验错误。
pub(crate) fn validate_asset_name(value: &str) -> AppResult<String> {
    let Some(name) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("asset name is required".to_owned()));
    };
    if name.len() > 128 {
        return Err(AppError::Validation(
            "asset name must be at most 128 characters".to_owned(),
        ));
    }
    Ok(name)
}

/// 限制资产小数精度为 0..=18；本函数不重算已有余额或流水。
pub(crate) fn validate_asset_precision(value: i32) -> AppResult<()> {
    if !(0..=18).contains(&value) {
        return Err(AppError::Validation(
            "asset precision_scale must be between 0 and 18".to_owned(),
        ));
    }
    Ok(())
}

/// 将资产符号去除空白并转为大写，生成最多 32 字节的稳定数据库代码。
/// 符号只允许 ASCII 字母数字；空白或非法格式返回校验错误，唯一性由资产写事务确认。
pub(crate) fn normalize_asset_symbol(value: &str) -> AppResult<String> {
    let Some(symbol) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("asset symbol is required".to_owned()));
    };
    if symbol.len() > 32 || !symbol.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(AppError::Validation(
            "asset symbol format is invalid".to_owned(),
        ));
    }
    Ok(symbol.to_ascii_uppercase())
}

/// 规范化资产类型为后台支持的法币、加密资产等稳定代码；未知类型拒绝持久化。
pub(crate) fn validate_asset_type(value: &str) -> AppResult<String> {
    let Some(asset_type) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("asset_type is required".to_owned()));
    };
    match asset_type.as_str() {
        "coin" | "fiat" | "stablecoin" | "platform" => Ok(asset_type),
        _ => Err(AppError::Validation("unsupported asset_type".to_owned())),
    }
}

/// 规范化资产启停状态；不在此处判断是否仍被交易对、钱包或产品引用。
pub(crate) fn validate_asset_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation("unsupported asset status".to_owned())),
    }
}

/// 将资产符号、精度、充提规则、Logo、阶梯费和状态映射为稳定审计 JSON。
/// 快照还包含资产类型和创建时间，但不含钱包余额；应用层在资产配置事务中保存前后值。
pub(crate) fn asset_audit_json(asset: &AdminAssetResponse) -> Value {
    json!({
        "id": asset.id,
        "symbol": asset.symbol,
        "name": asset.name,
        "logo_url": asset.logo_url,
        "precision_scale": asset.precision_scale,
        "asset_type": asset.asset_type,
        "status": asset.status,
        "deposit_enabled": asset.deposit_enabled,
        "withdraw_enabled": asset.withdraw_enabled,
        "min_deposit_amount": asset.min_deposit_amount,
        "deposit_fee": asset.deposit_fee,
        "withdraw_fee": asset.withdraw_fee,
        "withdraw_fee_tiers": asset.withdraw_fee_tiers.0.clone(),
        "created_at": asset.created_at.timestamp_millis(),
    })
}

/// 将充值网络别名规范为 `eth`、`base`、`tron`、`btc` 或 `solana` 五种稳定小写代码。
/// 支持 Ethereum/ERC20、Tron/TRC20、Bitcoin 和 Solana 常见别名；未知网络返回参数错误。
pub(crate) fn normalize_deposit_network(value: &str) -> AppResult<String> {
    let Some(network) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("network is required".to_owned()));
    };
    match network.to_ascii_lowercase().as_str() {
        "eth" | "ethereum" | "erc20" => Ok("eth".to_owned()),
        "base" => Ok("base".to_owned()),
        "tron" | "trx" | "trc20" => Ok("tron".to_owned()),
        "btc" | "bitcoin" => Ok("btc".to_owned()),
        "sol" | "solana" => Ok("solana".to_owned()),
        _ => Err(AppError::Validation(
            "unsupported deposit network".to_owned(),
        )),
    }
}

/// 去除充值网络展示名首尾空白并限制 64 个字符，供前端网络选择列表显示。
pub(crate) fn validate_deposit_network_display_name(value: &str) -> AppResult<String> {
    let Some(display_name) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("display_name is required".to_owned()));
    };
    if display_name.chars().count() > 64 {
        return Err(AppError::Validation("display_name is too long".to_owned()));
    }
    Ok(display_name)
}

/// 规范化充值地址组代码并限制字符集合和长度，使地址池分配查询可稳定匹配。
pub(crate) fn validate_address_group_code(value: &str) -> AppResult<String> {
    let Some(code) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation(
            "address_group_code is required".to_owned(),
        ));
    };
    if code.chars().count() > 64
        || !code
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(AppError::Validation(
            "address_group_code format is invalid".to_owned(),
        ));
    }
    Ok(code.to_ascii_uppercase())
}

/// 规范化充值网络配置状态，仅允许地址分配流程明确支持的生命周期代码。
pub(crate) fn validate_deposit_network_config_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "active" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported deposit network config status".to_owned(),
        )),
    }
}

/// 去除可选文本两端空白，将空串转为空值，并按调用方字段名执行字符长度上限校验。
/// 超长值返回带字段名的校验错误；函数不解释字段业务含义，也不持久化结果。
pub(crate) fn validate_optional_length(
    value: Option<String>,
    field: &str,
    max_len: usize,
) -> AppResult<Option<String>> {
    let Some(value) = optional_string(value) else {
        return Ok(None);
    };
    if value.chars().count() > max_len {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(Some(value))
}

/// 合并充值配置的单资产兼容字段和资产数组，返回去空、转大写且保持首次出现顺序的符号集合。
/// 优先使用非空数组，仅在数组结果为空时回退单值；重复项被忽略，超过 50 项或任一符号非法时返回校验错误。
pub(crate) fn normalize_deposit_asset_symbols(
    asset_symbol: Option<String>,
    asset_symbols: Option<Vec<String>>,
) -> AppResult<Vec<String>> {
    let mut symbols = Vec::new();
    let mut seen = HashSet::new();

    if let Some(values) = asset_symbols {
        for value in values {
            let Some(raw_symbol) = optional_string(Some(value)) else {
                continue;
            };
            let symbol = normalize_asset_symbol(&raw_symbol)?;
            if seen.insert(symbol.clone()) {
                symbols.push(symbol);
            }
        }
    }

    if symbols.is_empty()
        && let Some(raw_symbol) = optional_string(asset_symbol)
    {
        let symbol = normalize_asset_symbol(&raw_symbol)?;
        if seen.insert(symbol.clone()) {
            symbols.push(symbol);
        }
    }

    if symbols.len() > 50 {
        return Err(AppError::Validation(
            "asset_symbols cannot contain more than 50 assets".to_owned(),
        ));
    }

    Ok(symbols)
}

/// 将充值网络、地址组、资产集合、状态、排序和时间映射为配置审计快照。
/// 快照不包含地址池明细；应用层在网络配置写事务中保存前后值。
pub(crate) fn deposit_network_config_audit_json(
    config: &AdminDepositNetworkConfigResponse,
) -> Value {
    json!({
        "id": config.id,
        "network": config.network,
        "display_name": config.display_name,
        "address_group_code": config.address_group_code,
        "address_group_name": config.address_group_name,
        "asset_symbols": config.asset_symbols.0.clone(),
        "status": config.status,
        "sort_order": config.sort_order,
        "created_at": config.created_at.timestamp_millis(),
        "updated_at": config.updated_at.timestamp_millis(),
    })
}

#[derive(Debug)]
pub(crate) struct NormalizedDepositAddressPoolEntry {
    pub(crate) address: String,
    pub(crate) memo: Option<String>,
    pub(crate) remark: Option<String>,
}

/// 校验请求资产符号均受目标充值网络配置允许，并要求该网络配置处于 active。
/// 请求或白名单任一为空时视为不限制；比较不区分大小写，不支持的首个资产返回校验错误，函数不锁配置行。
pub(crate) fn ensure_deposit_asset_symbols_allowed_by_network(
    asset_symbols: &[String],
    network_config: &AdminDepositNetworkConfigResponse,
) -> AppResult<()> {
    if network_config.status != "active" {
        return Err(AppError::Validation(
            "deposit network config is disabled".to_owned(),
        ));
    }
    if asset_symbols.is_empty() || network_config.asset_symbols.0.is_empty() {
        return Ok(());
    }

    let allowed = network_config
        .asset_symbols
        .0
        .iter()
        .map(|symbol| symbol.to_ascii_uppercase())
        .collect::<HashSet<_>>();
    let unsupported = asset_symbols
        .iter()
        .find(|symbol| !allowed.contains(symbol.as_str()));
    if let Some(symbol) = unsupported {
        return Err(AppError::Validation(format!(
            "asset {symbol} does not support deposit network {}",
            network_config.network
        )));
    }
    Ok(())
}

/// 解析充值地址导入使用的地址组代码，缺省时采用网络配置值，显式值必须规范化后与配置一致。
/// 配置值或请求值格式非法、二者不一致均返回校验错误；该纯函数不修改网络配置或地址池。
pub(crate) fn resolve_deposit_address_group_code(
    requested_group_code: Option<String>,
    network_config: &AdminDepositNetworkConfigResponse,
) -> AppResult<String> {
    let configured_group_code = validate_address_group_code(&network_config.address_group_code)?;
    let Some(requested_group_code) = requested_group_code else {
        return Ok(configured_group_code);
    };
    let requested_group_code = validate_address_group_code(&requested_group_code)?;
    if requested_group_code != configured_group_code {
        return Err(AppError::Validation(
            "address_group_code must match deposit network config".to_owned(),
        ));
    }
    Ok(requested_group_code)
}

/// 去除充值地址首尾空白并限制 255 个字符；链格式、校验和及重复地址由具体网络/数据库约束确认。
pub(crate) fn validate_deposit_address(value: &str) -> AppResult<String> {
    let Some(address) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("address is required".to_owned()));
    };
    if address.chars().count() > 255 {
        return Err(AppError::Validation("address is too long".to_owned()));
    }
    Ok(address)
}

/// 规范化一批充值地址导入项，返回去空地址及长度受限的 memo/remark。
/// 批次必须为 1..=100 项且地址按原字符串区分重复；发现空地址、超长字段或批内重复即整体返回校验错误，不部分保留结果。
pub(crate) fn normalize_deposit_address_batch_entries(
    entries: Vec<CreateDepositAddressPoolEntryRequest>,
) -> AppResult<Vec<NormalizedDepositAddressPoolEntry>> {
    if entries.is_empty() {
        return Err(AppError::Validation(
            "at least one deposit address is required".to_owned(),
        ));
    }
    if entries.len() > 100 {
        return Err(AppError::Validation(
            "a single batch cannot contain more than 100 deposit addresses".to_owned(),
        ));
    }

    let mut normalized_entries = Vec::with_capacity(entries.len());
    let mut seen = HashSet::new();
    for entry in entries {
        let address = validate_deposit_address(&entry.address)?;
        if !seen.insert(address.clone()) {
            return Err(AppError::Validation(
                "duplicate deposit address in batch".to_owned(),
            ));
        }
        normalized_entries.push(NormalizedDepositAddressPoolEntry {
            address,
            memo: validate_optional_length(entry.memo, "memo", 255)?,
            remark: validate_optional_length(entry.remark, "remark", 512)?,
        });
    }

    Ok(normalized_entries)
}

/// 规范化地址池记录状态，区分可分配、已分配和禁用等既有生命周期代码。
pub(crate) fn validate_deposit_address_status(value: &str) -> AppResult<String> {
    let Some(status) = optional_string(Some(value.to_owned())) else {
        return Err(AppError::Validation("status is required".to_owned()));
    };
    match status.as_str() {
        "available" | "assigned" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "unsupported deposit address status".to_owned(),
        )),
    }
}

/// 在通用地址状态基础上进一步限制后台可手工设置的状态，仅允许 available 或 disabled。
/// 已分配状态由地址分配事务维护，管理员不得通过此入口伪造或释放用户绑定。
pub(crate) fn validate_deposit_address_assignable_status(value: &str) -> AppResult<String> {
    let status = validate_deposit_address_status(value)?;
    match status.as_str() {
        "available" | "disabled" => Ok(status),
        _ => Err(AppError::Validation(
            "assigned status is managed by user allocation".to_owned(),
        )),
    }
}

/// 将充值地址、网络、地址组、允许资产、分配用户、状态、备注和时间映射为地址池审计快照。
/// 地址本身会进入审计但不含私钥；应用层在插入、更新或回收事务中保存对应前后值。
pub(crate) fn deposit_address_pool_audit_json(address: &AdminDepositAddressPoolResponse) -> Value {
    json!({
        "id": address.id,
        "network": address.network,
        "address_group_code": address.address_group_code,
        "address": address.address,
        "asset_symbol": address.asset_symbol,
        "asset_symbols": address.asset_symbols.0.clone(),
        "status": address.status,
        "assigned_user_id": address.assigned_user_id,
        "assigned_user_email": address.assigned_user_email,
        "assigned_asset_symbol": address.assigned_asset_symbol,
        "assigned_at": address.assigned_at.map(|value| value.timestamp_millis()),
        "memo": address.memo,
        "remark": address.remark,
        "created_at": address.created_at.timestamp_millis(),
        "updated_at": address.updated_at.timestamp_millis(),
    })
}

fn validate_optional_withdraw_fee_tiers(value: Option<&[WithdrawFeeTier]>) -> AppResult<()> {
    if let Some(tiers) = value {
        normalize_asset_withdraw_fee_tiers(tiers.to_vec())?;
    }
    Ok(())
}

fn validate_optional_asset_amount(value: Option<&BigDecimal>, field: &str) -> AppResult<()> {
    if let Some(value) = value {
        validate_asset_amount(value, field)?;
    }
    Ok(())
}

fn validate_asset_amount(value: &BigDecimal, field: &str) -> AppResult<()> {
    if value < &BigDecimal::from(0) {
        return Err(AppError::Validation(format!(
            "{field} must be non-negative"
        )));
    }
    Ok(())
}
