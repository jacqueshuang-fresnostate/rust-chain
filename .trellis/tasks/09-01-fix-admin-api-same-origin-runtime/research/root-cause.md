# Admin Docker 构建同源配置根因

## 现象

GitHub 发布镜像中的 Admin bundle 在浏览器执行时抛出：

```text
VITE_API_SAME_ORIGIN 必须显式设置为 true 或 false
```

## 根因链路

1. `web/src/config/backend.ts` 在模块加载时读取 `import.meta.env.VITE_API_SAME_ORIGIN`，并
   按生产安全合同拒绝缺失或非布尔字符串。
2. `VITE_*` 是 Vite 构建期变量，不是容器启动后的运行期变量。
3. 根 Dockerfile 的 `web-builder` 原先直接执行 `npm run build`，未注入该变量。
4. 本地 `web/.env` 被 Git 忽略，且 `.dockerignore` 也排除所有 `.env*`；GitHub Actions
   checkout 与 Docker build context 因而都不会包含它。
5. Vite 可以成功生成 bundle，但缺失值会被编译进产物，直到浏览器加载模块才触发异常。
6. 镜像已通过 Nginx 将 Admin API、WebSocket 与 Rust 放在同一 Origin，因此正确的镜像
   构建值是 `VITE_API_SAME_ORIGIN=true`，且不需要 `VITE_API_BASE_URL`。

## 选择的修复

- 在 Dockerfile 的 Admin 构建阶段提供明确的同源构建参数默认值，并在 `npm run build`
  命令作用域内导出为 Vite 变量。
- 通过静态 Docker 构建合同测试防止该注入被误删。
- 在部署文档中明确 Compose `environment` 无法覆盖 Vite 已编译值。

## 未选择方案

- **缺失时静默回退同源**：会破坏刚建立的 fail-closed 生产配置合同，使真正的独立部署
  错配难以及时暴露。
- **依赖 `web/.env`**：该文件包含部署者本地值、被 Git/Docker 忽略，不适合作为镜像合同。
- **在 Compose 增加 VITE 变量**：静态 JS 已在镜像构建时生成，容器运行期变量不会生效。
