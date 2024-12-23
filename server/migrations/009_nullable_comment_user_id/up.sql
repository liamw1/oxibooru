-- Make the user_id column nullable
ALTER TABLE "comment"
ALTER COLUMN "user_id" DROP NOT NULL;

-- Drop the existing foreign key constraint
ALTER TABLE "comment"
DROP CONSTRAINT "comment_user_id_fkey";

-- Add the new foreign key constraint with ON DELETE SET NULL
ALTER TABLE "comment"
ADD CONSTRAINT "comment_user_id_fkey"
FOREIGN KEY ("user_id")
REFERENCES "user" ("id")
ON DELETE SET NULL;