ALTER TABLE "post"
ALTER COLUMN "checksum" TYPE VARCHAR(64)
USING encode("checksum", 'base64');

ALTER TABLE "post"
ALTER COLUMN "checksum_md5" TYPE VARCHAR(32)
USING encode("checksum_md5", 'base64'),
ALTER COLUMN "checksum_md5" DROP NOT NULL;