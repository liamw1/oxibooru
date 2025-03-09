-- Temporarily isable triggers on posts, as they make update extremely slow
ALTER TABLE "post" DISABLE TRIGGER USER;

UPDATE "post"
SET "source" = '' 
WHERE "source" IS NULL;

ALTER TABLE "post" ENABLE TRIGGER USER;

ALTER TABLE "post"
ALTER COLUMN "source" TYPE TEXT,
ALTER COLUMN "source" SET NOT NULL;