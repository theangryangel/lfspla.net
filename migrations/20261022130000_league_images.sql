ALTER TABLE league ADD COLUMN image_attachment_id BIGINT REFERENCES attachment(id);
