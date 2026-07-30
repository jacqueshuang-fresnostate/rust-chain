# 修复 1Panel 共享环境变量引用

## 背景

用户将连接值直接填写到 `x-api-environment` 后，`migrate` 服务仍通过
`${DATABASE_URL}` 读取 1Panel 编排环境或 `.env`，导致 Compose 报数据库环境变量
不存在。API 锚点中的硬编码值不会自动成为 Compose 插值变量。

## 目标

- `DATABASE_URL` 和 `RUST_LOG` 在一个共享 YAML 环境锚点中定义。
- `api` 通过合并共享锚点获得数据库地址和日志级别。
- `migrate` 直接复用同一个共享锚点，不再单独重复 `${DATABASE_URL}`。
- 保持镜像、端口、外部网络、上传目录和迁移完成门禁不变。
- 中文注释说明 1Panel 环境变量模式与直接填写模式的区别。
- 不把用户粘贴的真实密码或密钥写入仓库。

## 验收标准

- 使用示例环境文件执行 `docker compose config --quiet` 成功。
- 展开后 `api` 与 `migrate` 的 `DATABASE_URL` 和 `RUST_LOG` 完全相同。
- Compose 仍只包含 `api` 和 `migrate`。
- 外部网络仍为可配置的 `1panel-network`，API 默认端口映射和启动门禁不变。
- 配置文件不包含用户粘贴的真实凭据。
