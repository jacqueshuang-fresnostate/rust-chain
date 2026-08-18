//! 管理端 HTTP 请求的审计传输上下文。
//!
//! 该模块只保留请求 ID 与来源 IP 两项非业务元数据，并用 Tokio task-local
//! 把它们从 Axum 中间件传到同一请求内的事务审计写入。它们不参与身份识别、
//! 权限判定或限流；反向代理头只能作为运营排查线索，不得被当作可信安全边界。

use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use std::{net::IpAddr, str::FromStr};
use uuid::Uuid;

const REQUEST_ID_HEADER: &str = "x-request-id";
const MAX_REQUEST_ID_LEN: usize = 64;

/// 当前管理端请求的审计元数据快照。
/// 值在进入后台路由树时一次生成，业务事务只读取不修改，以保证同一请求写入的多条审计记录具有相同关联标识。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminRequestContext {
    pub request_id: String,
    pub source_ip: Option<String>,
}

tokio::task_local! {
    static ADMIN_REQUEST_CONTEXT: AdminRequestContext;
}

/// 为整个后台请求 future 绑定审计上下文，并把最终使用的请求 ID 回写到响应头。
/// 合法且不超过 64 字符的入站 `x-request-id` 会被保留，其余情况生成 UUIDv7；
/// 来源 IP 按 Cloudflare、X-Forwarded-For、X-Real-IP 顺序读取并严格解析为 IP 字面量。
/// 中间件不消费请求体，也不改变下游状态码；响应头编码失败时只略过回写。
pub async fn admin_request_context_middleware(request: Request, next: Next) -> Response {
    let context = AdminRequestContext {
        request_id: request_id(request.headers()),
        source_ip: source_ip(request.headers()),
    };
    let response_request_id = context.request_id.clone();

    ADMIN_REQUEST_CONTEXT
        .scope(context, async move {
            let mut response = next.run(request).await;
            if let Ok(value) = HeaderValue::from_str(&response_request_id) {
                response.headers_mut().insert(REQUEST_ID_HEADER, value);
            }
            response
        })
        .await
}

/// 读取当前请求的审计上下文。
/// HTTP 请求之外的 worker、迁移器或独立单元测试没有 task-local 作用域时返回 `None`，
/// 调用方应将审计 IP/request ID 落为 NULL，不得伪造一个网络来源。
pub fn current_admin_request_context() -> Option<AdminRequestContext> {
    ADMIN_REQUEST_CONTEXT.try_with(Clone::clone).ok()
}

fn request_id(headers: &HeaderMap) -> String {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= MAX_REQUEST_ID_LEN
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
                })
        })
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::now_v7().simple().to_string())
}

fn source_ip(headers: &HeaderMap) -> Option<String> {
    ["cf-connecting-ip", "x-forwarded-for", "x-real-ip"]
        .into_iter()
        .find_map(|name| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(',').next())
                .map(str::trim)
                .and_then(|value| IpAddr::from_str(value).ok())
                .map(|value| value.to_string())
        })
}
