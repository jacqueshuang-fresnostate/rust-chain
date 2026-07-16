ALTER TABLE users
    ADD COLUMN avatar_url TEXT NULL COMMENT '用户头像 URL' AFTER phone;

ALTER TABLE upload_objects
    ADD COLUMN uploaded_by_user BIGINT UNSIGNED NULL COMMENT '上传文件的用户 ID' AFTER uploaded_by,
    ADD INDEX idx_upload_objects_uploaded_by_user (uploaded_by_user),
    ADD CONSTRAINT fk_upload_objects_uploaded_by_user FOREIGN KEY (uploaded_by_user) REFERENCES users(id);
