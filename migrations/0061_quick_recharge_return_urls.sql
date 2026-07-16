ALTER TABLE quick_recharge_configs
    ADD COLUMN pc_app_redirect_url VARCHAR(1024) NULL COMMENT '快速充值配置：PC 应用端支付完成回跳地址' AFTER redirect_url,
    ADD COLUMN mac_app_redirect_url VARCHAR(1024) NULL COMMENT '快速充值配置：Mac 应用端支付完成回跳地址' AFTER pc_app_redirect_url,
    ADD COLUMN ios_app_redirect_url VARCHAR(1024) NULL COMMENT '快速充值配置：iOS 端支付完成回跳地址' AFTER mac_app_redirect_url,
    ADD COLUMN android_app_redirect_url VARCHAR(1024) NULL COMMENT '快速充值配置：Android 端支付完成回跳地址' AFTER ios_app_redirect_url,
    ADD COLUMN mobile_web_redirect_url VARCHAR(1024) NULL COMMENT '快速充值配置：手机网页端支付完成回跳地址' AFTER android_app_redirect_url,
    ADD COLUMN desktop_web_redirect_url VARCHAR(1024) NULL COMMENT '快速充值配置：电脑网页端支付完成回跳地址' AFTER mobile_web_redirect_url;

ALTER TABLE quick_recharge_orders
    ADD COLUMN return_target VARCHAR(32) NULL COMMENT '快速充值订单：创建订单时选择的回跳终端类型' AFTER payment_url,
    ADD COLUMN redirect_url VARCHAR(1024) NULL COMMENT '快速充值订单：创建订单时传给服务商的支付完成回跳地址' AFTER return_target;
