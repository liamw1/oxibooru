-- Temporarily isable triggers on posts, as they make update extremely slow
ALTER TABLE "post" DISABLE TRIGGER USER;

UPDATE "post"
SET "checksum_md5" = '' 
WHERE "checksum_md5" IS NULL;

ALTER TABLE "post" ENABLE TRIGGER USER;

ALTER TABLE "post"
ALTER COLUMN "checksum" TYPE BYTEA
USING decode("checksum" || REPEAT('=', 4 - LENGTH("checksum") % 4), 'base64');

ALTER TABLE "post"
ALTER COLUMN "checksum_md5" TYPE BYTEA
USING decode("checksum_md5" || REPEAT('=', 4 - LENGTH("checksum_md5") % 4), 'base64'),
ALTER COLUMN "checksum_md5" SET NOT NULL;