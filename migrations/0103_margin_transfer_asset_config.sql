ALTER TABLE assets
    ADD COLUMN margin_transfer_enabled BOOLEAN NOT NULL DEFAULT FALSE
        COMMENT '是否允许用户从现货账户转入杠杆账户'
        AFTER withdraw_enabled;

UPDATE assets AS asset
SET margin_transfer_enabled = TRUE
WHERE EXISTS (
          SELECT 1
          FROM margin_products AS product
          WHERE product.margin_asset = asset.id
      )
   OR EXISTS (
          SELECT 1
          FROM margin_wallet_accounts AS wallet
          WHERE wallet.asset_id = asset.id
      );
