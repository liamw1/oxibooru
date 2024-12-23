-- Drop the existing foreign key constraint
ALTER TABLE "comment"
DROP CONSTRAINT "comment_user_id_fkey";

-- Add the original foreign key constraint with ON DELETE CASCADE
ALTER TABLE "comment"
ADD CONSTRAINT "comment_user_id_fkey"
FOREIGN KEY ("user_id")
REFERENCES "user" ("id")
ON DELETE CASCADE;

-- Make the user_id column NOT NULL
ALTER TABLE "comment"
ALTER COLUMN "user_id" SET NOT NULL;