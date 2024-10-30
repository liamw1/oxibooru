ALTER TABLE "post_signature" 
DROP COLUMN "signature";

ALTER TABLE "post_signature"
ADD COLUMN "signature" BIGINT[];

UPDATE "post_signature"
SET "signature" = '{}';

ALTER TABLE "post_signature"
ALTER COLUMN "signature" SET NOT NULL;