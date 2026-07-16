# Deposit Address Contracts

## Scenario: Configurable Deposit Network Address Groups

### 1. Scope / Trigger

- Trigger: wallet deposit address allocation, admin deposit network configuration, or PC recharge network selection.
- Applies when a network can share an address class with another network, such as Ethereum and Base sharing EVM addresses.

### 2. Signatures

- DB: `deposit_network_configs(network, display_name, address_group_code, address_group_name, asset_symbols_json, status, sort_order)`.
- DB: `deposit_address_pool.address_group_code` is the allocation key for address inventory.
- User API: `GET /api/v1/wallet/deposit-networks?asset_symbol=USDT`.
- User API: `POST /api/v1/wallet/deposit-address` with `{ asset_symbol, network }`.
- Admin API: `/admin/api/v1/deposit-network-configs` list/create/update.
- Admin API: `/admin/api/v1/deposit-address-pool` and `/batch` accept `address_group_code`.

### 3. Contracts

- `deposit_network_configs.network` remains the user-selected chain network key.
- `address_group_code` is the address set/class key. Networks with the same group code share address inventory.
- `asset_symbols_json` on network config limits which active deposit assets can use that network; `NULL` means no network-level asset restriction.
- Address allocation must first load an active network config, validate the requested asset against it, then query `deposit_address_pool` by `address_group_code`.
- API responses may return the requested network even when the physical address row was imported under another network in the same group.

### 4. Validation & Error Matrix

- Missing active network config -> `VALIDATION_ERROR`.
- Asset not included in network config `asset_symbols_json` -> `VALIDATION_ERROR`.
- Disabled asset deposit switch -> `VALIDATION_ERROR: asset does not support deposit`.
- Admin address pool `address_group_code` differs from the selected network config -> `VALIDATION_ERROR`.
- Duplicate configured network -> `CONFLICT`.

### 5. Good/Base/Bad Cases

- Good: ETH and Base both point to group `A`; a user selecting Base can receive an EVM address imported under ETH.
- Base: BTC points to group `B`; Tron points to group `C`; they do not share inventory.
- Bad: hard-coding Base -> ETH fallback in application code instead of using `deposit_network_configs`.

### 6. Tests Required

- Backend route tests should seed or upsert the relevant `deposit_network_configs` row before inserting address pool rows.
- Address pool rows in tests must include `address_group_code`.
- PC tests should assert `/wallet/deposit-networks` is used for recharge network selection.
- Admin resource tests should assert network config columns and `address_group_code` request payloads.

### 7. Wrong vs Correct

#### Wrong

```rust
network IN ('base', 'eth')
```

#### Correct

```rust
WHERE address_group_code = network_config.address_group_code
```
