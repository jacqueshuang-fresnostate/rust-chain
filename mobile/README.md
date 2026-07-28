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

## PWA 与 Tauri 构建隔离

Web 发布使用独立 PWA 模式；该模式生成 Web App Manifest、`sw.js` 和静态应用壳 precache：

```bash
npm run build:pwa
npm run preview
```

`npm run build` 等价于 `npm run build:pwa`。PWA 只预缓存编译后的 HTML、JavaScript、CSS、字体和品牌图标，不配置 runtime API/Auth/WebSocket/金融数据缓存，也不使用 Background Sync。离线启动仅代表应用外壳可用，不代表行情、账户或交易数据可用。

Tauri 发布使用专用模式，`vite-plugin-pwa` 在该模式下禁用，不生成 manifest 或 service worker；运行时还会通过 `window.__TAURI_INTERNALS__` 阻止注册：

```bash
npm run build:tauri
```

`src-tauri/tauri.conf.json` 已将 `beforeBuildCommand` 指向该命令。Android/iOS 的 Tauri 构建脚本会继续复用它。

### Web 部署配置

- 独立域名部署保持 `VITE_PWA_BASE=/`；固定子路径部署需设置同一个永久前缀，例如 `VITE_PWA_BASE=/mobile/`。
- 公开 PWA 必须通过 HTTPS 提供，后端 API 必须使用 HTTPS，同源 `/ws/` 必须升级为 WSS。不能把生产 `VITE_BACKEND_API_DOMAIN` 留为 `http://127.0.0.1:8080`。
- `index.html`、`sw.js` 和 `manifest.webmanifest` 应使用 `no-cache` 或短期重新验证；带哈希的静态资源可使用长期 `immutable` 缓存。
- 身份认证、账户、钱包、订单、KYC 和其他金融接口必须返回 `Cache-Control: private, no-store`，且 CDN/反向代理不得缓存。

### `PwaStatus` 集成

PWA 注册由 `src/main.ts` 完成，`App.vue` 已在根 `.app-frame` 内唯一挂载
`<PwaStatus />`。该组件负责安装、iOS 主屏说明、离线、更新与注册失败提示；
安装和更新提示只会出现在经过允许的安全路由，交易、提币、KYC、安全设置和
二次验证等敏感流程不会被打断。不要在子页面重复挂载或自行注册 Service
Worker。
