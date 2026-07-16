ALTER TABLE seconds_contract_orders
    ADD COLUMN settlement_price DECIMAL(38,18) NULL COMMENT '秒合约结算价格' AFTER entry_price;
