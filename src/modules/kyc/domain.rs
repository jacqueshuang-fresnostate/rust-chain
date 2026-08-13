//! kyc bounded context domain layer.
//!
//! 领域层：放置业务实体、值对象和不依赖 I/O 的业务规则。
//! 本文件定义实名认证的两套校验口径：运营侧配置的合法性，以及用户提交材料对该配置的符合性。
//! 支持个人与企业两种申请主体，企业申请额外要求企业名称与工商注册号。
//! 证件类型限定在身份证、护照、驾照、居留许可四种，并可按国家进一步收窄，
//! 部分国家与证件类型组合还会强制要求手持证件照。
//! 状态机取值在此收敛为 `pending`、`approved`、`rejected` 三种，
//! 且审核动作只允许写入后两者，`pending` 是提交时的初始态而非可选的审核结论。
//! 证件图片以 Base64 文本随请求体传入，长度上限由配置的原始字节上限换算得出，
//! 换算已计入编码膨胀与信封开销，详见 `encoded_payload_limit`。
//! 隐私边界：本层会处理姓名、证件号与证件图片原文，但全部为纯函数式的内存校验，
//! 不落库、不写日志、不产生审计，任何错误消息都只含字段名而不回显字段值。

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

const MAX_DOCUMENT_SIZE_BYTES: u64 = 10 * 1024 * 1024;
const DOCUMENT_PAYLOAD_PADDING_BYTES: u64 = 2048;
const IDENTITY_FRONT_DOCUMENT: &str = "identity_front";
const IDENTITY_BACK_DOCUMENT: &str = "identity_back";
const DEFAULT_DOCUMENT_TYPE: &str = "identity_card";
const DEFAULT_SUBMISSION_TYPE: &str = "personal";
const ENTERPRISE_SUBMISSION_TYPE: &str = "enterprise";
const SUPPORTED_DOCUMENT_TYPES: &[&str] = &[
    "identity_card",
    "passport",
    "driver_license",
    "residence_permit",
];

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct KycCountryDocumentTypeRule {
    pub country: String,
    pub document_types: Vec<String>,
    #[serde(default)]
    pub handheld_document_types: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct KycConfigValidationInput {
    pub(crate) enabled: bool,
    pub(crate) target_kyc_level: i32,
    pub(crate) required_documents: Vec<String>,
    pub(crate) allowed_countries: Vec<String>,
    pub(crate) country_document_types: Vec<KycCountryDocumentTypeRule>,
    pub(crate) max_document_size_bytes: u64,
}

#[derive(Debug)]
pub(crate) struct ValidatedKycConfig {
    pub(crate) enabled: bool,
    pub(crate) target_kyc_level: i32,
    pub(crate) required_documents: Vec<String>,
    pub(crate) allowed_countries: Vec<String>,
    pub(crate) country_document_types: Vec<KycCountryDocumentTypeRule>,
    pub(crate) max_document_size_bytes: u64,
}

#[derive(Debug)]
pub(crate) struct KycSubmissionConfigRules {
    pub(crate) required_documents: Vec<String>,
    pub(crate) allowed_countries: Vec<String>,
    pub(crate) country_document_types: Vec<KycCountryDocumentTypeRule>,
    pub(crate) max_document_size_bytes: u64,
}

#[derive(Debug)]
pub(crate) struct KycSubmissionValidationInput {
    pub(crate) real_name: String,
    pub(crate) country: String,
    pub(crate) id_number: String,
    pub(crate) submission_type: Option<String>,
    pub(crate) enterprise_name: Option<String>,
    pub(crate) business_registration_number: Option<String>,
    pub(crate) document_type: Option<String>,
    pub(crate) document_front_image: String,
    pub(crate) document_back_image: String,
    pub(crate) document_handheld_image: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ValidatedKycSubmission {
    pub(crate) real_name: String,
    pub(crate) country: String,
    pub(crate) id_number: String,
    pub(crate) submission_type: String,
    pub(crate) enterprise_name: Option<String>,
    pub(crate) business_registration_number: Option<String>,
    pub(crate) document_type: String,
    pub(crate) document_front_image: String,
    pub(crate) document_back_image: String,
    pub(crate) document_handheld_image: Option<String>,
}

/// 校验并规范化运营侧提交的实名认证配置，返回可安全落库的形态。
/// 目标 KYC 等级必须为正数，等级零或负数没有业务含义。
/// 单份材料的原始字节上限被夹在一千字节到十兆之间：下限挡住把上限配到近乎为零而使所有提交失败，
/// 上限则是系统硬边界，防止运营配出会撑爆请求体的数值。
/// 必填材料清单去重后不得为空，且目前只接受 `identity_front` 与 `identity_back` 两项，
/// 手持照不通过这里配置，而是由按国家的证件类型规则决定。
/// 允许国家清单去重后可以为空，空表示不限制国家。
/// 按国家的证件类型规则交由专门函数校验国家唯一性与手持类型的包含关系。
/// 纯函数，任何一步失败都直接返回校验错误，不产生持久化或审计副作用。
pub(crate) fn validate_kyc_config(
    input: KycConfigValidationInput,
) -> AppResult<ValidatedKycConfig> {
    if input.target_kyc_level <= 0 {
        return Err(AppError::Validation(
            "target_kyc_level must be positive".to_owned(),
        ));
    }
    if input.max_document_size_bytes < 1024
        || input.max_document_size_bytes > MAX_DOCUMENT_SIZE_BYTES
    {
        return Err(AppError::Validation(format!(
            "max_document_size_bytes must be between 1024 and {MAX_DOCUMENT_SIZE_BYTES}"
        )));
    }
    let required_documents =
        normalize_unique_values(&input.required_documents, "required_documents", 64)?;
    if required_documents.is_empty() {
        return Err(AppError::Validation(
            "required_documents is required".to_owned(),
        ));
    }
    for document in &required_documents {
        if document != IDENTITY_FRONT_DOCUMENT && document != IDENTITY_BACK_DOCUMENT {
            return Err(AppError::Validation(
                "required_documents only supports identity_front and identity_back".to_owned(),
            ));
        }
    }

    Ok(ValidatedKycConfig {
        enabled: input.enabled,
        target_kyc_level: input.target_kyc_level,
        required_documents,
        allowed_countries: normalize_unique_values(
            &input.allowed_countries,
            "allowed_countries",
            128,
        )?,
        country_document_types: validate_country_document_types(&input.country_document_types)?,
        max_document_size_bytes: input.max_document_size_bytes,
    })
}

/// 按当前 KYC 配置规范化并校验个人或企业申请，生成可安全持久化的领域输入。
/// 调用方须先确认用户权限、配置启用状态和无待审申请；本函数不读取数据库或判断审核资格。
/// 企业申请必须具备主体信息，国家与证件类型须命中白名单，材料长度不得超过编码后上限。
/// 该纯函数不启事务、不记录审计，也不改变原申请；失败返回具体校验错误且无副作用。
/// 相同输入与配置可安全重放并得到相同结果，但持久化幂等由应用层的待审锁负责。
pub(crate) fn validate_kyc_submission(
    input: KycSubmissionValidationInput,
    config: &KycSubmissionConfigRules,
) -> AppResult<ValidatedKycSubmission> {
    let real_name = required_string(Some(input.real_name), "real_name", 128)?;
    let country = required_string(Some(input.country), "country", 128)?;
    let id_number = required_string(Some(input.id_number), "id_number", 128)?;
    let submission_type = validate_submission_type(
        optional_string(input.submission_type)
            .unwrap_or_else(|| DEFAULT_SUBMISSION_TYPE.to_owned()),
    )?;
    let enterprise_name = optional_string(input.enterprise_name);
    let business_registration_number = optional_string(input.business_registration_number);
    if submission_type == ENTERPRISE_SUBMISSION_TYPE {
        // 企业认证时，企业名称和统一社会信用代码均为必填项，方便后台识别主体。
        let _ = required_string(enterprise_name.clone(), "enterprise_name", 128)?;
        let _ = required_string(
            business_registration_number.clone(),
            "business_registration_number",
            128,
        )?;
    }

    let document_type = validate_document_type(
        optional_string(input.document_type).unwrap_or_else(|| DEFAULT_DOCUMENT_TYPE.to_owned()),
        "document_type",
    )?;

    if !config.allowed_countries.is_empty()
        && !config
            .allowed_countries
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(country.as_str()))
    {
        return Err(AppError::Validation("country is not allowed".to_owned()));
    }
    validate_document_type_allowed_for_country(&country, &document_type, config)?;

    let document_front_image = required_string(
        Some(input.document_front_image),
        "document_front_image",
        encoded_payload_limit(config.max_document_size_bytes) as usize,
    )?;
    let document_back_image = required_string(
        Some(input.document_back_image),
        "document_back_image",
        encoded_payload_limit(config.max_document_size_bytes) as usize,
    )?;
    let document_handheld_image = optional_string(input.document_handheld_image)
        .map(|value| {
            required_string(
                Some(value),
                "document_handheld_image",
                encoded_payload_limit(config.max_document_size_bytes) as usize,
            )
        })
        .transpose()?;

    if config
        .required_documents
        .iter()
        .any(|document| document == IDENTITY_FRONT_DOCUMENT)
        && document_front_image.is_empty()
    {
        return Err(AppError::Validation(
            "document_front_image is required".to_owned(),
        ));
    }
    if config
        .required_documents
        .iter()
        .any(|document| document == IDENTITY_BACK_DOCUMENT)
        && document_back_image.is_empty()
    {
        return Err(AppError::Validation(
            "document_back_image is required".to_owned(),
        ));
    }
    if requires_handheld_document_image(&country, &document_type, config)
        && document_handheld_image
            .as_deref()
            .unwrap_or_default()
            .is_empty()
    {
        return Err(AppError::Validation(
            "document_handheld_image is required".to_owned(),
        ));
    }

    Ok(ValidatedKycSubmission {
        real_name,
        country,
        id_number,
        submission_type,
        enterprise_name,
        business_registration_number,
        document_type,
        document_front_image,
        document_back_image,
        document_handheld_image,
    })
}

/// 校验并规范化「按国家限定证件类型」的规则列表，输出顺序与输入保持一致。
/// 国家名做非空与长度校验，并按忽略大小写比较拒绝重复条目：
/// 同一国家配两条规则会让后续按国家查找的结果取决于顺序，属于必须在配置阶段拦下的歧义。
/// 每条规则的可用证件类型去重后不得为空，空清单等于禁止该国家提交任何证件，应通过移除条目表达。
/// 关键约束是手持照类型必须是可用类型的子集：
/// 若某类型只出现在手持清单而不在可用清单，它永远不会被选中，这条规则将成为无法触发的死配置。
/// 任一条目不合法即整体失败，不做部分接受。
fn validate_country_document_types(
    rules: &[KycCountryDocumentTypeRule],
) -> AppResult<Vec<KycCountryDocumentTypeRule>> {
    let mut result = Vec::new();
    for rule in rules {
        let country = required_string(
            Some(rule.country.clone()),
            "country_document_types.country",
            128,
        )?;
        if result.iter().any(|current: &KycCountryDocumentTypeRule| {
            current.country.eq_ignore_ascii_case(&country)
        }) {
            return Err(AppError::Validation(
                "country_document_types has duplicated country".to_owned(),
            ));
        }

        let document_types = normalize_document_types(
            &rule.document_types,
            "country_document_types.document_types",
        )?;
        if document_types.is_empty() {
            return Err(AppError::Validation(
                "country_document_types.document_types is required".to_owned(),
            ));
        }
        let handheld_document_types = normalize_document_types(
            &rule.handheld_document_types,
            "country_document_types.handheld_document_types",
        )?;
        if handheld_document_types
            .iter()
            .any(|document_type| !document_types.contains(document_type))
        {
            return Err(AppError::Validation(
                "country_document_types.handheld_document_types must be included in document_types"
                    .to_owned(),
            ));
        }
        result.push(KycCountryDocumentTypeRule {
            country,
            document_types,
            handheld_document_types,
        });
    }
    Ok(result)
}

/// 逐项校验一组证件类型并去重，保留首次出现的顺序，供国家规则的两个清单共用。
/// 去重发生在规范化之后，因此大小写不同但含义相同的写法会被折叠为一项。
/// `field` 只用于拼装错误消息，让调用方能分辨出错的是可用类型清单还是手持类型清单。
/// 空输入返回空向量而非报错，是否允许为空由各调用点按语义自行判断。
fn normalize_document_types(values: &[String], field: &str) -> AppResult<Vec<String>> {
    let mut result = Vec::new();
    for value in values {
        let document_type = validate_document_type(value.clone(), field)?;
        if !result.contains(&document_type) {
            result.push(document_type);
        }
    }
    Ok(result)
}

/// 规范化并校验单个证件类型，输出统一转为小写以消除大小写写法差异。
/// 三道检查依次施加：非空且不超过 64 字符、字符集限于 ASCII 字母数字与下划线连字符、
/// 最终取值必须命中平台支持的四种类型之一。
/// 字符集检查看似被白名单覆盖而多余，但它让格式错误与类型不支持返回不同消息，
/// 便于运营区分「填错格式」和「填了平台尚未支持的证件」。
/// `field` 参与错误消息拼装，使同一函数可服务申请提交与配置校验两个场景。
fn validate_document_type(value: String, field: &str) -> AppResult<String> {
    let document_type = required_string(Some(value), field, 64)?.to_ascii_lowercase();
    if !document_type
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        return Err(AppError::Validation(format!("{field} format is invalid")));
    }
    if !SUPPORTED_DOCUMENT_TYPES.contains(&document_type.as_str()) {
        return Err(AppError::Validation(format!("{field} is not supported")));
    }
    Ok(document_type)
}

/// 规范化申请主体类型，只接受个人与企业两种，输出统一小写。
/// 长度上限设为 16 字符，远大于两个合法取值，仅用于挡住超长垃圾输入而非表达业务约束。
/// 该取值决定后续是否强制要求企业名称与工商注册号，因此不能容错降级：
/// 未知取值一律返回校验错误，若默默回落到个人类型会让企业申请绕过主体信息校验。
fn validate_submission_type(value: String) -> AppResult<String> {
    let submission_type = required_string(Some(value), "submission_type", 16)?.to_ascii_lowercase();
    if !matches!(
        submission_type.as_str(),
        DEFAULT_SUBMISSION_TYPE | ENTERPRISE_SUBMISSION_TYPE
    ) {
        return Err(AppError::Validation(
            "submission_type must be one of personal or enterprise".to_owned(),
        ));
    }
    Ok(submission_type)
}

/// 判断某国家是否允许使用指定证件类型，规则来自运营配置的按国家清单。
/// 三段语义需要分清：整个按国家清单为空表示该机制未启用，直接放行；
/// 清单非空但找不到该国家条目，返回「该国家未配置证件类型」而非放行，
/// 这是有意从严——机制一旦启用，未显式配置的国家就不该被接受；
/// 找到条目则要求证件类型精确命中其可用清单，否则拒绝。
/// 国家匹配忽略大小写，证件类型此时已被规范化为小写故用精确比较。
/// 只读判定，不修改任何输入。
fn validate_document_type_allowed_for_country(
    country: &str,
    document_type: &str,
    config: &KycSubmissionConfigRules,
) -> AppResult<()> {
    if config.country_document_types.is_empty() {
        return Ok(());
    }
    let Some(rule) = config
        .country_document_types
        .iter()
        .find(|rule| rule.country.eq_ignore_ascii_case(country))
    else {
        return Err(AppError::Validation(
            "country document types are not configured".to_owned(),
        ));
    };
    if rule
        .document_types
        .iter()
        .any(|allowed| allowed == document_type)
    {
        Ok(())
    } else {
        Err(AppError::Validation(
            "document_type is not allowed for country".to_owned(),
        ))
    }
}

/// 判断当前国家与证件类型的组合是否强制要求上传手持证件照。
/// 要求粒度是「国家加证件类型」而非仅按国家：同一国家可以只对护照要求手持照而对身份证不要求。
/// 采取默认不要求的口径：找不到国家条目，或该条目的手持清单里没有这个证件类型，都返回 `false`。
/// 与允许性校验的从严取向不同，这里从宽是因为漏配手持要求只是少收一张照片，
/// 而误判为必填会直接阻断用户提交。
/// 国家匹配忽略大小写；返回布尔值不报错，是否必填由调用方结合实际图片内容判定。
fn requires_handheld_document_image(
    country: &str,
    document_type: &str,
    config: &KycSubmissionConfigRules,
) -> bool {
    config
        .country_document_types
        .iter()
        .find(|rule| rule.country.eq_ignore_ascii_case(country))
        .is_some_and(|rule| {
            rule.handheld_document_types
                .iter()
                .any(|required| required == document_type)
        })
}

/// 规范化一组自由文本并按原顺序去重，用于必填材料清单与允许国家清单两处配置。
/// 每项都要求非空且不超过 `max_chars` 个字符，任一项不合法即整体失败。
/// 去重使用精确相等比较而不忽略大小写，因此大小写不同的国家写法会被保留为两项；
/// 这与按国家查找时忽略大小写的口径不同，配置侧应保持书写一致以免产生冗余条目。
/// 空输入返回空向量，是否允许为空由调用方按各自语义判断。
fn normalize_unique_values(
    values: &[String],
    field: &str,
    max_chars: usize,
) -> AppResult<Vec<String>> {
    let mut result = Vec::new();
    for value in values {
        let item = required_string(Some(value.clone()), field, max_chars)?;
        if !result.iter().any(|current: &String| current == &item) {
            result.push(item);
        }
    }
    Ok(result)
}

/// 规范化一个 KYC 必填文本字段：裁剪首尾空白、要求非空、并限制最大字符数。
/// 与 user 上下文的同名函数的差别在于这里多带长度上限，因为 KYC 字段既包含姓名、证件号这类短文本，
/// 也包含 Base64 证件图片这类超长载荷，二者共用同一入口而只在上限取值上区分。
/// 长度按 Unicode 字符数而非字节数统计，中文姓名不会因多字节被误判超长。
/// 缺失与超长返回不同消息但都只含字段名，绝不回显字段内容，避免证件号进入错误响应或日志。
pub(crate) fn required_string(
    value: Option<String>,
    field: &str,
    max_chars: usize,
) -> AppResult<String> {
    let Some(value) = optional_string(value) else {
        return Err(AppError::Validation(format!("{field} is required")));
    };
    if value.chars().count() > max_chars {
        return Err(AppError::Validation(format!("{field} is too long")));
    }
    Ok(value)
}

/// 规范化 KYC 可选文本：裁剪首尾空白，把缺省与纯空白统一折叠为 `None`。
/// 在本模块中它还承担默认值判定的职责：申请类型与证件类型都靠它判断用户是否真的填了值，
/// 折叠为 `None` 时调用方才会套用个人认证与身份证这两个默认取值。
/// 不做长度校验，需要限长的字段应在其后再过一次 `required_string`。
pub(crate) fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// 把配置中的证件原始字节上限换算为 Base64 编码后的文本长度上限。
/// 系数 4/3 对应 Base64 每三字节膨胀为四字符的固定比率；
/// 再加 `DOCUMENT_PAYLOAD_PADDING_BYTES` 的余量用于覆盖填充字符和 data URI 前缀等信封开销，
/// 避免恰好等于上限的图片因为多出几十字节的头部而被拒绝。
/// 全程使用饱和运算，配置被填成极大值时结果封顶而不会溢出回绕成一个极小的上限。
/// 换算结果用作字符数上限，对纯 ASCII 的 Base64 文本而言与字节数等价。
fn encoded_payload_limit(size_bytes: u64) -> u64 {
    size_bytes
        .saturating_mul(4)
        .saturating_div(3)
        .saturating_add(DOCUMENT_PAYLOAD_PADDING_BYTES)
}

/// 规范化并校验 KYC 状态字符串，是状态机取值集合的唯一定义处。
/// 合法值只有三种：`pending` 表示已提交待人工审核，`approved` 表示审核通过，`rejected` 表示驳回。
/// 输出统一小写，输入前后空白被裁剪，其余任何取值返回 `AppError::Validation`。
/// 本函数只判定单个取值是否合法，不涉及状态之间能否迁移；迁移方向的限制见 `validate_review_status`。
pub(crate) fn validate_kyc_status(value: &str) -> AppResult<String> {
    let status = value.trim().to_ascii_lowercase();
    if matches!(status.as_str(), "pending" | "approved" | "rejected") {
        Ok(status)
    } else {
        Err(AppError::Validation("status is invalid".to_owned()))
    }
}

/// 校验管理端审核动作要写入的目标状态，在合法取值之上再收窄一层。
/// 先复用通用状态校验保证取值本身合法，再显式排除 `pending`：
/// 待审是提交时的初始状态，不是审核结论，允许写回会让已审结的申请退回待审队列，
/// 也会破坏「审核是终态迁移」这一前提，因此这里只放行通过与驳回两个方向。
/// 本函数不检查申请当前处于什么状态，`pending` 到终态的前置条件由 application 层在事务内锁行确认。
pub(crate) fn validate_review_status(value: &str) -> AppResult<String> {
    let status = validate_kyc_status(value)?;
    if status == "pending" {
        return Err(AppError::Validation(
            "review status cannot be pending".to_owned(),
        ));
    }
    Ok(status)
}
