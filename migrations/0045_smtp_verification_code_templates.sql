ALTER TABLE smtp_configs
    ADD COLUMN verification_code_templates_json JSON NULL AFTER verification_code_template_html;
