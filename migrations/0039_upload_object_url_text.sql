ALTER TABLE upload_objects
    MODIFY public_url TEXT NOT NULL,
    MODIFY share_url TEXT NULL,
    MODIFY delete_url TEXT NULL;
