# 修复 GMPay Cloudflare 403 错误提示

## 背景

后台快速充值联通测试返回 `GMPAY_REQUEST_FAILED`，消息里包含 Cloudflare `Just a moment...` HTML 页面。当前后端把 GMPay 非 2xx 响应体原样拼接进错误消息，导致管理员看到一大段不可读 HTML，也容易误以为是签名或表单字段问题。

## 目标

- GMPay 下单请求带上常规服务端 HTTP 头，减少被服务商网关误判的概率。
- 当服务商返回 Cloudflare 挑战页或 HTML 页面时，返回简短、可操作的中文错误提示。
- 保持错误码 `GMPAY_REQUEST_FAILED` 和 502 HTTP 状态不变，避免破坏前端错误处理。
- 不在错误消息中暴露完整 HTML、签名、密钥或请求参数。
- 覆盖单元测试：Cloudflare 403 响应应转换为简短提示。

## 非目标

- 不绕过 Cloudflare 验证。
- 不实现浏览器挑战、Cookie 维持或第三方反爬流程。
- 不改变 GMPay 签名算法和回调验签逻辑。

## 验收标准

- Cloudflare `Just a moment...`/`challenge-platform` 响应不再原样展示 HTML。
- 错误信息明确提示需要使用服务商提供的后端 API 域名，或让服务商将服务器 IP/API 路径加入放行名单。
- 现有 GMPay 成功下单测试仍通过。
- 快速充值相关 Rust 测试和格式检查通过。
