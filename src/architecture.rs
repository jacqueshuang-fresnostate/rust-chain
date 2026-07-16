//! Backend architecture markers.
//!
//! The backend is moving toward a DDD-style layout per bounded context:
//! domain, repository, service, application, infrastructure, presentation,
//! and thin routes.
//! These marker traits document intent without forcing a framework.

/// 领域层：保存业务实体、值对象和纯规则。
///
/// 这一层不应该依赖 Axum、SQLx、Redis 或 HTTP DTO。
pub trait DomainLayer {}

/// 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
///
/// 这一层描述“需要保存/读取什么”，不直接绑定 SQLx、Redis 或第三方 SDK。
pub trait RepositoryLayer {}

/// 服务层：封装可复用业务服务和跨实体规则。
///
/// 这一层承载业务动作本身，应用层负责把它编排进具体用例和事务。
pub trait ServiceLayer {}

/// 应用层：编排用例、事务边界和跨仓储协作。
///
/// 这一层可以决定“做什么”，但不应该直接关心 HTTP 请求如何表示。
pub trait ApplicationLayer {}

/// 基础设施层：封装 SQLx、Redis、第三方接口和仓储实现。
///
/// 金融类写操作需要把余额变化和流水审计保持在同一个事务里。
pub trait InfrastructureLayer {}

/// 表现层：负责请求/响应 DTO 与传输层格式转换。
///
/// 这一层只做边界适配，不承载钱包、风控、结算等核心业务决策。
pub trait PresentationLayer {}
