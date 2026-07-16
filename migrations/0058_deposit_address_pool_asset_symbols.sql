ALTER TABLE deposit_address_pool
    ADD COLUMN asset_symbols_json JSON NULL COMMENT '充值地址池：限定可使用该地址的资产符号列表，空表示该网络任意资产可用' AFTER asset_symbol;

UPDATE deposit_address_pool
SET asset_symbols_json = JSON_ARRAY(asset_symbol)
WHERE asset_symbol IS NOT NULL
  AND asset_symbols_json IS NULL;
