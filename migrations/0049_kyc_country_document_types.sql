ALTER TABLE kyc_configs
    ADD COLUMN country_document_types_json JSON NULL AFTER allowed_countries_json;

UPDATE kyc_configs
SET country_document_types_json = JSON_ARRAY()
WHERE country_document_types_json IS NULL;

ALTER TABLE kyc_configs
    MODIFY COLUMN country_document_types_json JSON NOT NULL;
