-- Temporarily isable triggers on posts, as they make update extremely slow
ALTER TABLE "post" DISABLE TRIGGER USER;

UPDATE "post"
SET "checksum_md5" = '' 
WHERE "checksum_md5" IS NULL;

ALTER TABLE "post" ENABLE TRIGGER USER;

-- Decodes from base64 if it is a valid base64 sequence. Converts to UTF8 binary otherwise.
CREATE FUNCTION try_decode_base64(input TEXT) RETURNS BYTEA AS $$
BEGIN
    -- PostgreSQL expects a padded base64 sequence
    RETURN decode(input || REPEAT('=', 4 - LENGTH("checksum") % 4), 'base64');
EXCEPTION WHEN others THEN
    RETURN convert_to(input, 'UTF8');
END;
$$ LANGUAGE plpgsql IMMUTABLE;

ALTER TABLE "post"
ALTER COLUMN "checksum" TYPE BYTEA
USING try_decode_base64("checksum"); 

ALTER TABLE "post"
ALTER COLUMN "checksum_md5" TYPE BYTEA
USING try_decode_base64("checksum_md5"),
ALTER COLUMN "checksum_md5" SET NOT NULL;

DROP FUNCTION try_decode_base64;