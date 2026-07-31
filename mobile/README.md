# Hippo Mobile

独立的移动端客户端，前端使用 Vue 3 + Vite，原生壳使用 Tauri v2。H5、Android 和 iOS 共用 `src/` 中的界面与 API 适配层。

## 本地开发

```bash
npm install
npm run dev
```

H5 开发地址默认为 `http://127.0.0.1:1611/`。浏览器始终向 Vite 同源的
`/api/v1` 和 `/api/v1/ws/public` 发起业务请求，再由
`VITE_BACKEND_DEV_PROXY_TARGET` 转发到后端；该代理目标默认是
`https://hipoex.cllbmz.kdns.fr`。例如本地后端运行在 18080 时可设置：

```bash
VITE_BACKEND_DEV_PROXY_TARGET=http://127.0.0.1:18080 npm run dev
```

设置代理目标不会让浏览器改为跨域直连。PWA 和 Tauri 发布包默认使用
`https://hipoex.cllbmz.kdns.fr`，HTTP API 为
`https://hipoex.cllbmz.kdns.fr/api/v1`，公共行情 WebSocket 为
`wss://hipoex.cllbmz.kdns.fr/api/v1/ws/public`。非空的
`VITE_BACKEND_API_DOMAIN` 会优先覆盖产品默认域名；生产配置仍会拒绝明文 HTTP、
`localhost`、`127.0.0.1` 和其他设备 loopback 地址。

客户端启动和业务页面展示不以 `/health` 为门禁。该地址在产品域名上可能被
Cloudflare Managed Challenge 返回 HTTP 403，而市场、认证配置、Tauri CORS 和公共
WebSocket 业务入口仍可正常使用。

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
- 公开 PWA 必须通过 HTTPS 提供。未提供非空 `VITE_BACKEND_API_DOMAIN` 时，产品默认 HTTP API 使用 `https://hipoex.cllbmz.kdns.fr/api/v1`，行情 WebSocket 使用 `wss://hipoex.cllbmz.kdns.fr/api/v1/ws/public`。
- 跨域部署可显式设置其他 HTTPS `VITE_BACKEND_API_DOMAIN`；非空环境值优先，生产配置会拒绝明文 HTTP、`localhost` 和 loopback 地址。
- `VITE_BACKEND_DEV_PROXY_TARGET` 只在 Vite 开发服务器中生效，不会进入生产客户端的 API origin 选择。
- `/health` 只保留为独立诊断地址，不参与客户端启动、路由进入或业务 API 可用性判断。
- `index.html`、`sw.js` 和 `manifest.webmanifest` 应使用 `no-cache` 或短期重新验证；带哈希的静态资源可使用长期 `immutable` 缓存。
- 身份认证、账户、钱包、订单、KYC 和其他金融接口必须返回 `Cache-Control: private, no-store`，且 CDN/反向代理不得缓存。

### `PwaStatus` 集成

PWA 注册由 `src/main.ts` 完成，`App.vue` 已在根 `.app-frame` 内唯一挂载
`<PwaStatus />`。该组件负责安装、iOS 主屏说明、离线、更新与注册失败提示；
安装和更新提示只会出现在经过允许的安全路由，交易、提币、KYC、安全设置和
二次验证等敏感流程不会被打断。不要在子页面重复挂载或自行注册 Service
Worker。
