ALTER TABLE smtp_configs
    ADD COLUMN verification_code_template_html TEXT NULL AFTER from_name;
