//! 借贷产品配置与本金敞口文档；金额都是十进制文本，容量不是托管余额或可放款资金证明。
#![allow(dead_code)]

use utoipa::{OpenApi, ToSchema};

/// 产品准入字段；用户限额对申请产品的币种跨全部借贷产品累计，产品容量只累计本产品同币种。
#[derive(ToSchema)]
struct LoanPrincipalExposurePolicy {
    /// null 关闭，零禁止新增；含 pending、disbursed、overdue 本金，不含利息和费用。
    user_principal_limit: Option<String>,
    /// null 关闭，零禁止新增；待审核即预留，终态释放。
    product_principal_capacity: Option<String>,
    /// 默认 false；开启时任何未结逾期借贷阻止申请及审批，包括已过 due_at 尚未扫描的订单。
    #[schema(default = false)]
    deny_borrowing_while_overdue: bool,
}

#[derive(ToSchema)]
struct LoanCollateralAssetConfig {
    collateral_asset_id: u64,
    oracle_symbol: String,
    oracle_source: String,
    oracle_max_age_seconds: u64,
}

/// 与既有产品表单保持一致的完整配置；利率、期限和抵押基础不随敞口策略变化。
#[derive(ToSchema)]
struct LoanProductConfiguration {
    loan_type: String,
    asset_id: u64,
    name: String,
    name_json: Option<serde_json::Value>,
    term_days: u32,
    interest_rate: String,
    interest_calculation_mode: String,
    min_kyc_level: i32,
    min_amount: String,
    max_amount: Option<String>,
    initial_ltv: Option<String>,
    maintenance_ltv: Option<String>,
    liquidation_ltv: Option<String>,
    collateral_assets: Vec<LoanCollateralAssetConfig>,
    #[serde(flatten)]
    exposure: LoanPrincipalExposurePolicy,
}

#[derive(ToSchema)]
struct CreateLoanProductRequest {
    #[serde(flatten)]
    configuration: LoanProductConfiguration,
    /// 省略时为 active。
    status: Option<String>,
    reason: String,
}

#[derive(ToSchema)]
struct UpdateLoanProductRequest {
    #[serde(flatten)]
    configuration: LoanProductConfiguration,
    status: String,
    revision: u64,
    reason: String,
}

#[derive(ToSchema)]
struct LoanProductResponse {
    #[serde(flatten)]
    configuration: LoanProductConfiguration,
    id: u64,
    asset_symbol: String,
    status: String,
    revision: u64,
    /// 当前产品币种待审预留本金，加载时快照，不是外部流动性。
    reserved_principal: String,
    /// 当前产品币种已放款及逾期本金，加载时快照。
    outstanding_principal: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(ToSchema)]
struct AdminLoanProductsResponse {
    products: Vec<LoanProductResponse>,
    total: i64,
}

#[utoipa::path(
    get, path = "/admin/api/v1/loan/products", tag = "loan",
    security(("bearerAuth" = [])),
    params(
        ("loan_type" = Option<String>, Query),
        ("status" = Option<String>, Query),
        ("limit" = Option<u32>, Query),
        ("offset" = Option<u32>, Query)
    ),
    responses((status = 200, body = AdminLoanProductsResponse))
)]
fn list_admin_loan_products() {}

#[utoipa::path(
    get, path = "/admin/api/v1/loan/products/{id}", tag = "loan",
    security(("bearerAuth" = [])), params(("id" = u64, Path)),
    responses((status = 200, body = LoanProductResponse))
)]
fn get_admin_loan_product() {}

#[utoipa::path(
    post, path = "/admin/api/v1/loan/products", tag = "loan",
    security(("bearerAuth" = [])), request_body = CreateLoanProductRequest,
    responses((status = 200, body = LoanProductResponse),
        (status = 400, description = "限额必须非负且满足资产及 DECIMAL(38,18) 精度；原因必填"))
)]
fn create_admin_loan_product() {}

#[utoipa::path(
    patch, path = "/admin/api/v1/loan/products/{id}", tag = "loan",
    security(("bearerAuth" = [])), params(("id" = u64, Path)),
    request_body = UpdateLoanProductRequest,
    responses((status = 200, body = LoanProductResponse),
        (status = 400, description = "限额或完整配置无效"),
        (status = 409, description = "revision 已过期，或仍有未结敞口时尝试切换币种"))
)]
fn update_admin_loan_product() {}

/// 主文档聚合者通过 merge 注册此借贷子文档，不在借贷切片改动共享聚合入口。
#[derive(OpenApi)]
#[openapi(
    paths(
        list_admin_loan_products,
        get_admin_loan_product,
        create_admin_loan_product,
        update_admin_loan_product
    ),
    components(schemas(
        LoanPrincipalExposurePolicy,
        LoanCollateralAssetConfig,
        LoanProductConfiguration,
        CreateLoanProductRequest,
        UpdateLoanProductRequest,
        LoanProductResponse,
        AdminLoanProductsResponse
    ))
)]
pub(super) struct LoanApiDoc;
