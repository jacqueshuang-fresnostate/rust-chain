# Hippo Mobile

独立的移动端客户端，前端使用 Vue 3 + Vite，原生壳使用 Tauri v2。H5、Android 和 iOS 共用 `src/` 中的界面与 API 适配层。

## 本地开发

```bash
npm install
npm run dev
```

H5 开发地址默认为 `http://127.0.0.1:1611/`。未配置 `VITE_BACKEND_API_DOMAIN` 时，Vite 会将 `/api/v1` 同源代理到本机 `http://127.0.0.1:8080`，方便浏览器、Android 和 iOS 调试共享接口；原生发布与 H5 部署时应注入实际的 `VITE_BACKEND_API_DOMAIN` 和 `VITE_BACKEND_API_PREFIX`。

## 原生目标

```bash
npm run tauri:ios:init
npm run tauri:android:init
npm run tauri:ios:dev
npm run tauri:android:dev
```

Android 命令会自动探测 macOS、Windows 和 Linux 的常见 SDK 路径；若本机路径不同，请在终端设置 `ANDROID_HOME` 或 `ANDROID_SDK_ROOT`。iOS 发布构建仍需在 Xcode 中配置有效的签名团队和 Provisioning Profile。

项目的 iOS 脚本只在自身子进程中允许 Tauri 使用 Swift 依赖的 bare Git repository，不会修改全局 Git 配置。

## 校验

```bash
npm run type-check
npm test
npm run build
```
