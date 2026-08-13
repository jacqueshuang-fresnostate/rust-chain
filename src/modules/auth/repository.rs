//! auth bounded context repository layer.
//!
//! 仓储层：定义持久化边界、仓储接口和面向领域的读写契约。
//!
//! 本文件只声明认证限界上下文向外部存储提出的读写契约，不含 SQL 语句、Redis 命令和连接管理，
//! MySQL 与 Redis 两套实现都落在基础设施层。接口刻意保持窄口径：口令明文从不越过这层边界，
//! 传入的只有已经过 Argon2 散列的凭据；刷新令牌要么以不可逆摘要入库，要么由适配器在内部派生存储键。
//! 读取类方法一律用 `None` 表达未命中，不区分账号不存在与账号被停用，由服务层折叠成同一个未授权响应，
//! 使调用方无法通过响应差异枚举账号。写入类方法各自独立提交，端口不承诺跨方法事务。

use crate::{
    error::AppResult,
    modules::auth::{
        ActiveCountryConfig, ActorType, AuthActor, NewAdminActor, NewAgentActor, NewUserActor,
        RefreshTokenRecord, StoredActorCredential, StoredProjectRefreshToken, StoredRefreshToken,
    },
};
use axum::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};

#[async_trait]
/// 认证主体仓储端口，统一承载用户、管理员和代理账号的凭据及登录状态持久化。
///
/// 各方法可由一个或多个独立存储语句组成，本端口不承诺方法内或跨方法事务；
/// 账号创建、失败计数、登录记录与令牌签发的部分成功边界由应用或服务层显式处理。
pub trait AuthRepository: Clone + Send + Sync + 'static {
    /// 落库一名新普通用户并返回其主体标识，入参中的 `password_hash` 必须已由调用方完成 Argon2 散列。
    /// 实现只写账号本身，不写推荐关系、邀请码或注册事件；邮箱与手机号的唯一冲突须映射为冲突类错误上抛。
    /// 本方法独立提交，与随后的令牌签发不共用事务，因此签发失败时账号已经存在且不会被本层回滚。
    async fn create_user(&self, actor: NewUserActor) -> AppResult<AuthActor>;
    /// 落库一名平台管理员的用户名、口令哈希与角色 ID，并返回可直接用于签发令牌的管理员主体。
    /// 角色 ID 的有效性由数据库外键约束保证，实现不做额外的角色存在性预查，冲突与外键错误一律上抛。
    /// 是否允许创建管理员属于授权判断，由服务层在调用前完成，本方法不校验调用者身份。
    async fn create_admin(&self, actor: NewAdminActor) -> AppResult<AuthActor>;
    /// 落库一套代理后台登录凭据并挂到入参给定的代理节点上，返回代理作用域主体。
    /// 本方法只创建可登录的后台账号，不创建代理公司本身，也不生成或修改代理层级路径。
    /// 代理节点是否存在、是否活跃由外键与后续登录查询把关，用户名唯一冲突须作为冲突类错误上抛。
    async fn create_agent(&self, actor: NewAgentActor) -> AppResult<AuthActor>;
    /// 读取指定国家在注册场景下的可用配置，返回其规范化国家码与默认语言，供新账号初始化本地化字段。
    /// 未开放注册或已停用的国家必须按不存在处理，返回 `None`，不得把配置存在但被禁用的差异透给调用方。
    /// 这是无锁读取，返回后配置仍可能被后台改动；注册事务需要一致性时应改用带行锁的加载路径。
    async fn find_registration_country(
        &self,
        country_code: &str,
    ) -> AppResult<Option<ActiveCountryConfig>>;
    /// 按已转小写的邮箱取出用户口令哈希与账号状态，供服务层在同一处完成密码比对和状态判断。
    /// 返回值携带账号状态而非直接过滤，使停用账号也会走完口令校验分支，从而与密码错误保持一致的响应耗时。
    /// 未命中返回 `None`，实现不得在错误信息里区分邮箱未注册和口令不匹配，避免注册邮箱被逐个探测出来。
    async fn find_user_by_email(&self, email: &str) -> AppResult<Option<StoredActorCredential>>;
    /// 按调用方已规范化的手机号取出同一组用户凭据快照，是邮箱登录之外的第二条标识入口。
    /// 号码格式与国家码前缀的整形在进入本方法前完成，实现按存储中的原样精确匹配，不做模糊或去分隔符处理。
    /// 与邮箱查询一样，未命中只返回 `None`，账号是否存在的差异不允许通过错误类型或耗时暴露。
    async fn find_user_by_phone(&self, phone: &str) -> AppResult<Option<StoredActorCredential>>;
    /// 按已规范化的用户名取出用户凭据快照，仅在安全策略开启用户名登录时才允许调用。
    /// 开关判定属于上层职责，本方法不重复检查；实现层不得把这条查询暴露给策略关闭时的登录路径，
    /// 否则用户名登录会被绕过开关启用。用户名不存在同样只返回 `None`，不区别于口令错误。
    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    /// 按用户名取出平台管理员的口令哈希与账号状态，返回的主体不带 `user_id`，与普通用户身份完全隔离。
    /// 密码比对一律在服务层进行，仓储只负责取数，避免比较逻辑在多个实现中出现不一致的短路行为。
    /// 未命中返回 `None`；后台登录同样共用统一的失败计数与锁定策略，因此这里不做任何尝试次数判断。
    async fn find_admin_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    /// 判断管理员表是否已有任意一行，用于把无需鉴权的引导注册入口限制在系统首次初始化时。
    /// 返回真之后，创建管理员必须由已登录的活跃管理员发起，这是该入口唯一的安全边界，不能被调用方跳过。
    /// 本查询与随后的插入不在同一事务内，并发引导的最终结果由用户名唯一约束决定，实现不承诺互斥。
    async fn has_any_admin(&self) -> AppResult<bool>;
    /// 按用户名取出代理后台账号的凭据快照，并保留代理主体类型，使签发的令牌落在独立的 `agent` 作用域。
    /// 实现须同时要求所属代理及其整条上级链路处于活跃状态，任一祖先被停用时按未命中处理，
    /// 从而在上级代理被冻结时立即切断其下级后台的登录能力，而不必逐个改写下级账号状态。
    async fn find_agent_by_username(
        &self,
        username: &str,
    ) -> AppResult<Option<StoredActorCredential>>;
    /// 按主体类型分发到对应账号表，重新确认该主体此刻仍然有效，并回填权威的主体标识。
    /// 令牌刷新和管理员授权都依赖这次回查：它让密码修改、账号停用或代理层级冻结在旧令牌到期前就能生效，
    /// 使撤销不必等待访问令牌自然过期。停用账号、失效代理链路与记录不存在统一返回 `None`，
    /// 调用方须把三者都折叠为未授权，不得据此判断账号是否存在。
    async fn find_active_actor(&self, actor: &AuthActor) -> AppResult<Option<AuthActor>>;
    /// 在密码验证成功后记录适配器支持的登录元数据；MySQL 实现当前只更新代理管理员时间，
    /// 用户和平台管理员为成功空操作。实际 SQL 失败须使本次登录整体失败。
    async fn record_login(&self, actor: &AuthActor) -> AppResult<()>;
    /// 登记一枚 JWT 模式刷新令牌的摘要及其绑定主体和到期时间，入参只接受摘要，原始令牌严禁落库。
    /// 采用摘要存储意味着数据库被读取也无法还原出可用令牌，同时仍能支持按令牌反查和整户撤销。
    /// 到期时间同时充当查询过滤条件，本方法不负责清理历史记录，过期行由查询条件排除而非物理删除。
    async fn store_refresh_token(&self, token: StoredRefreshToken) -> AppResult<()>;
    /// 按摘要回查刷新令牌绑定的主体，只返回在给定时刻仍未过期且未被标记撤销的记录。
    /// 时间由调用方传入而非实现内部取当前时间，便于测试固定时钟，也使同一次刷新流程共用一致的时间基准。
    /// 过期、已撤销和摘要不存在统一返回 `None`；本方法只读，不消费也不轮换令牌，重放防护须由上层实现。
    async fn find_refresh_token(
        &self,
        token_hash: &str,
        now: NaiveDateTime,
    ) -> AppResult<Option<RefreshTokenRecord>>;
    /// 在比对口令之前查询该主体类型与标识组合当前是否处于锁定期，命中时返回锁定截止时刻。
    /// 已过期的锁定按不存在处理，因此解锁靠时间自然到期，无需后台任务或人工干预清理计数行。
    /// 标识须是领域层规范化后的失败计数键，直接传入原始输入会因大小写或空白差异查不到既有锁定。
    async fn find_login_lockout(
        &self,
        actor_type: ActorType,
        identifier: &str,
    ) -> AppResult<Option<DateTime<Utc>>>;
    /// 为一次失败的口令校验累加计数，并在计数越过阈值时开始锁定，返回新的锁定截止时刻。
    /// 计数落在滑动窗口内，窗口过期后自动从头计数；返回 `None` 表示本次失败尚未触发锁定。
    /// 账号不存在时同样要调用本方法，否则爆破者可以按响应差异区分出哪些标识是真实账号。
    /// 实现须保证并发失败请求都被计入，不得因为读改写竞争而漏计，否则等于放过一整轮尝试。
    async fn record_login_failure(
        &self,
        actor_type: ActorType,
        identifier: &str,
    ) -> AppResult<Option<DateTime<Utc>>>;
    /// 在口令校验通过后清空该主体类型与标识组合的失败计数和锁定状态，使下一轮阈值从零重新开始。
    /// 这也是失败计数行唯一的常规回收路径，因此只有真正登录成功才允许调用，任何失败分支都不得清零。
    /// 目标行不存在时视为已清理并返回成功，重复调用幂等；本方法不校验调用方是否确实完成了认证。
    async fn clear_login_failures(&self, actor_type: ActorType, identifier: &str) -> AppResult<()>;
}

#[async_trait]
/// 项目刷新令牌存储端口，隔离 Sa-Token 模式使用的 Redis 会话实现。
///
/// 原始令牌只允许在适配器内部转换为不可逆键；读取必须校验过期时间，撤销范围以
/// 适配器已登记的主体索引为准。端口只定义单次调用结果，不承诺记录键与索引键跨命令原子性。
pub trait ProjectRefreshTokenRepository: Send + Sync + 'static {
    /// 登记一枚 Sa-Token 模式的刷新令牌及其主体快照，并按记录自带的到期时刻设置存储层 TTL。
    /// 快照里保存主体类型、主体 ID、可选用户 ID 与作用域，使刷新时无需回查账号表即可判定预期作用域。
    /// 依赖 TTL 到期自动回收，实现不做定时清扫；同一令牌重复写入按覆盖处理，不产生第二条记录。
    /// 传入的原始令牌只允许在适配器内部用于派生存储键，落盘内容中不得出现可直接使用的令牌串。
    async fn store_project_refresh_token(&self, token: StoredProjectRefreshToken) -> AppResult<()>;

    /// 用原始令牌派生存储键并取回主体快照，仅当记录在传入时刻仍未到期时才返回。
    /// 除了依赖存储层 TTL，实现还须用记录内的到期时间再判一次，避免 TTL 尚未生效的残留记录被继续使用。
    /// 键不存在、记录已过期以及内容无法解析都必须归为未命中或未授权，不得把损坏数据当成有效主体放行，
    /// 也不得把解析细节透出到错误信息里。本方法只读，不消费令牌，重复刷新的语义由服务层决定。
    async fn find_project_refresh_token(
        &self,
        refresh_token: &str,
        now: DateTime<Utc>,
    ) -> AppResult<Option<RefreshTokenRecord>>;

    /// 一次性撤销该主体名下已登记的全部刷新令牌，用于改密、强制下线等需要立即失效所有会话的场景。
    /// 撤销范围以适配器维护的主体索引为准，未被索引登记的令牌不会被清除，因此写入端必须保证索引同步。
    /// 主体没有任何在册令牌时视为已完成并返回成功，重复调用幂等，不会因为索引为空而报错。
    /// 本方法只处理刷新令牌，已签发且未过期的访问令牌需由会话管理器单独登出。
    async fn revoke_actor_refresh_tokens(&self, actor: &AuthActor) -> AppResult<()>;
}
