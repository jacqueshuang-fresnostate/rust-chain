# ETH Deposit Addresses Support Base

## Goal

充值地址池中录入为 ETH 网络的 EVM 地址，需要同时可用于用户选择 Base 网络充值。

## Scope

- 用户申请 Base 充值地址时，优先使用 Base 地址池；若没有可用 Base 地址，则可使用 ETH 地址池。
- 用户已分配到 ETH 地址池地址后，再选择 Base 网络充值应复用同一地址，并在响应中返回请求网络 `base`。
- 现有 ETH、Tron、BTC、Solana 分配逻辑保持不变。

## Acceptance Criteria

- Base 请求可以从 ETH 网络地址池分配地址。
- Base 请求返回的 `network` 仍是 `base`，不会导致 PC 端显示成 ETH。
- 同一用户在 Base/ETH 之间请求同一 ETH 池地址时复用同一地址，不会给其他用户重复分配。
- 回归测试覆盖 ETH 地址池支持 Base 的行为。
