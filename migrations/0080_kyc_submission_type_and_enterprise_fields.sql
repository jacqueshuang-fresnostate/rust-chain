-- Extend KYC submissions with enterprise-submission metadata.
ALTER TABLE user_kyc_submissions
  ADD COLUMN submission_type VARCHAR(16) NOT NULL DEFAULT 'personal' COMMENT '认证类型：personal(个人) / enterprise(企业)',
  ADD COLUMN enterprise_name VARCHAR(128) NULL COMMENT '企业认证时的企业名称',
  ADD COLUMN business_registration_number VARCHAR(128) NULL COMMENT '企业认证时的统一社会信用代码';
