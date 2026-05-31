ALTER TABLE new_coin_projects
    ADD COLUMN post_listing_purchase_enabled BOOLEAN NOT NULL DEFAULT FALSE AFTER status,
    ADD COLUMN post_listing_pair_id BIGINT UNSIGNED NULL AFTER post_listing_purchase_enabled,
    ADD CONSTRAINT fk_new_coin_projects_post_listing_pair
        FOREIGN KEY (post_listing_pair_id) REFERENCES trading_pairs(id);
