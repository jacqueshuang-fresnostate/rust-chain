//! 管理员当前身份与权限响应 DTO。

use crate::{architecture::PresentationLayer, modules::admin::domain::AdminScope};
use serde::Serialize;

/// 后台前端启动时读取的权限快照，不含密码、令牌或角色原始 JSON。
#[derive(Debug, Serialize)]
pub(crate) struct AdminAccessResponse {
    pub(crate) admin_id: u64,
    pub(crate) username: String,
    pub(crate) must_change_password: bool,
    pub(crate) role_id: u64,
    pub(crate) role_name: String,
    pub(crate) permissions: Vec<String>,
    pub(crate) is_super_admin: bool,
}

impl From<AdminScope> for AdminAccessResponse {
    /// 将领域快照转为稳定线格式；权限数组保持字典序，超级管理标志只由 `*` 推导。
    fn from(scope: AdminScope) -> Self {
        let is_super_admin = scope.permissions.contains("*");
        let permissions = scope.permission_list();
        Self {
            admin_id: scope.admin_id,
            username: scope.username,
            must_change_password: scope.must_change_password,
            role_id: scope.role_id,
            role_name: scope.role_name,
            permissions,
            is_super_admin,
        }
    }
}

impl PresentationLayer for AdminAccessResponse {}

/// 角色编辑器使用的权限码目录；权限字符串由后端生成，前端不得自行拼接作为保存依据。
#[derive(Debug, Serialize)]
pub(crate) struct AdminPermissionCatalogResponse {
    pub(crate) permissions: Vec<String>,
}

impl PresentationLayer for AdminPermissionCatalogResponse {}
