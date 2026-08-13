//! 全局错误类型与 HTTP 响应映射：把各层抛出的异常收敛成 `AppError`，再统一序列化为固定结构的错误响应体。
//! 响应体只含稳定的机器码 `code` 与人读 `message`，前端应按 `code` 分支处理，`message` 文案允许变化。
//! 基础设施类错误一律折叠为 500 并只暴露分类码，但 `Display` 实现会带上底层库原文，因此这类错误只应写日志、不应回显。
//! 客户端可纠正的错误（校验、鉴权、冲突）保留具体语义，`Api` 变体则允许业务自行指定状态码与错误码。

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;
use tracing::error;
use utoipa::ToSchema;

/// 全服务统一错误枚举，前六个变体由外部依赖自动转换而来，其余由业务代码显式构造。
/// 变体的选择直接决定对外 HTTP 状态码与错误码，新增变体时必须同步补齐状态码与码值映射。
#[derive(Debug, Error)]
pub enum AppError {
    /// 配置解析或缺失导致的错误，通常只在启动阶段出现，映射为 500 且不区分具体配置项。
    #[error("configuration error: {0}")]
    Config(#[from] config::ConfigError),
    /// MySQL 查询、事务或连接池错误，涵盖唯一键冲突等约束失败，调用方若要区分冲突需自行匹配底层错误。
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// MongoDB 读写错误，主要来自 K 线等行情历史数据的存取。
    #[error("mongo error: {0}")]
    Mongo(#[from] mongodb::error::Error),
    /// Redis 操作错误，覆盖会话存储、缓存与 worker 协调键的读写失败。
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    /// RabbitMQ 连接或信道错误，事件发布与消费失败会归入这一类。
    #[error("rabbitmq error: {0}")]
    RabbitMq(#[from] lapin::Error),
    /// 未认证：缺少令牌、令牌过期或签名无效，客户端应重新登录或走刷新流程。
    #[error("unauthorized")]
    Unauthorized,
    /// 已认证但无权访问，例如令牌作用域与接口所属端不匹配，重新登录并不能解决。
    #[error("forbidden")]
    Forbidden,
    /// 入参校验失败，`String` 是给调用方看的具体原因，允许直接展示。
    #[error("validation error: {0}")]
    Validation(String),
    /// 目标资源不存在或对当前调用方不可见，不携带任何标识信息。
    #[error("not found")]
    NotFound,
    /// 与现有状态冲突，例如重复注册或状态机不允许的迁移，`String` 说明冲突点。
    #[error("conflict: {0}")]
    Conflict(String),
    /// 兜底的内部错误，`String` 是内部描述，会随 500 响应一并返回，构造时不得写入敏感细节。
    #[error("internal error: {0}")]
    Internal(String),
    /// 业务自定义响应：由调用方直接指定状态码与稳定错误码，用于安全策略等需要细分码值的场景。
    #[error("{message}")]
    Api {
        status: StatusCode,
        code: &'static str,
        message: String,
    },
}

pub type AppResult<T> = Result<T, AppError>;

/// 对外统一的错误响应体，`code` 为稳定机器码供前端分支判断，`message` 为可变的人读描述。
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    /// 把错误变体映射成 HTTP 状态码：鉴权类分别对应 401 与 403，校验 400、缺失 404、冲突 409。
    /// 所有基础设施类错误与内部错误一律折叠为 500，不向外区分是数据库、缓存还是队列出了问题。
    /// `Api` 变体直接采用构造时给定的状态码，因此业务可以表达上述固定映射之外的语义。
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Api { status, .. } => *status,
            Self::Config(_)
            | Self::Database(_)
            | Self::Mongo(_)
            | Self::Redis(_)
            | Self::RabbitMq(_)
            | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// 给出响应体里的稳定错误码，前端据此分支处理，因此这些字面量属于对外契约，不能随意改名。
    /// 基础设施类错误虽然都返回 500，但码值仍按依赖种类区分，便于客户端与监控快速定位故障面。
    /// `Api` 变体透传构造时传入的码值，由各业务模块自行保证命名不与既有码冲突。
    fn code(&self) -> &'static str {
        match self {
            Self::Config(_) => "CONFIG_ERROR",
            Self::Database(_) => "DATABASE_ERROR",
            Self::Mongo(_) => "MONGO_ERROR",
            Self::Redis(_) => "REDIS_ERROR",
            Self::RabbitMq(_) => "RABBITMQ_ERROR",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::Internal(_) => "INTERNAL_ERROR",
            Self::Api { code, .. } => code,
        }
    }

    /// 构造安全策略类的 400 响应，把调用方给定的稳定错误码与提示文案原样带到响应体里。
    /// 用于二次验证、资金密码等需要让前端按细分码值区分处理的校验失败，语义上仍属于客户端可纠正错误。
    /// 传入的 `message` 会直接回显给调用方，因此不得包含验证码原文、密钥或其他内部细节。
    pub fn security_validation(code: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    /// 构造安全策略类的 403 响应，用于身份已确认但当前操作被安全规则拒绝的场景。
    /// 与通用 `Forbidden` 的区别在于携带细分错误码，前端可据此提示补齐二次验证或联系客服等具体动作。
    /// 同样会把 `message` 直接回显，构造时应给出面向用户的原因而非内部判定过程。
    pub fn security_forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status: StatusCode::FORBIDDEN,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    /// 把错误渲染成 JSON 响应：先算出状态码，再以 `code` 与 `Display` 文本组装标准错误体。
    /// 只有 5xx 会额外打一条 error 日志，4xx 视为调用方可自行纠正，不占用错误日志量。
    /// 由于 `message` 直接来自 `Display`，基础设施类错误会把底层库原文透出，构造时必须避免带入敏感数据。
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();

        if status.is_server_error() {
            error!(error = %self, "请求处理失败");
        }

        let body = ErrorResponse {
            code: self.code(),
            message: self.to_string(),
        };

        (status, Json(body)).into_response()
    }
}
