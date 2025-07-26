-- ================================== Setup ================================== --
-- Enable PL/Python for python helper functions
CREATE EXTENSION plpython3u;

-- Disable triggers on tables, as they make copying data over extremely slow
ALTER TABLE oxi."user" DISABLE TRIGGER USER;
ALTER TABLE oxi."tag_category" DISABLE TRIGGER USER;
ALTER TABLE oxi."tag" DISABLE TRIGGER USER;
ALTER TABLE oxi."tag_name" DISABLE TRIGGER USER;
ALTER TABLE oxi."tag_implication" DISABLE TRIGGER USER;
ALTER TABLE oxi."tag_suggestion" DISABLE TRIGGER USER;
ALTER TABLE oxi."post" DISABLE TRIGGER USER;
ALTER TABLE oxi."post_relation" DISABLE TRIGGER USER;
ALTER TABLE oxi."post_tag" DISABLE TRIGGER USER;
ALTER TABLE oxi."post_favorite" DISABLE TRIGGER USER;
ALTER TABLE oxi."post_feature" DISABLE TRIGGER USER;
ALTER TABLE oxi."post_note" DISABLE TRIGGER USER;
ALTER TABLE oxi."post_score" DISABLE TRIGGER USER;
ALTER TABLE oxi."comment" DISABLE TRIGGER USER;
ALTER TABLE oxi."comment_score" DISABLE TRIGGER USER;
ALTER TABLE oxi."pool_category" DISABLE TRIGGER USER;
ALTER TABLE oxi."pool" DISABLE TRIGGER USER;
ALTER TABLE oxi."pool_name" DISABLE TRIGGER USER;
ALTER TABLE oxi."pool_post" DISABLE TRIGGER USER;

-- Disable triggers on Szurubooru tables
ALTER TABLE public."user" DISABLE TRIGGER ALL;
ALTER TABLE public."tag_category" DISABLE TRIGGER ALL;
ALTER TABLE public."tag" DISABLE TRIGGER ALL;
ALTER TABLE public."tag_name" DISABLE TRIGGER ALL;
ALTER TABLE public."tag_implication" DISABLE TRIGGER ALL;
ALTER TABLE public."tag_suggestion" DISABLE TRIGGER ALL;
ALTER TABLE public."post" DISABLE TRIGGER ALL;
ALTER TABLE public."post_relation" DISABLE TRIGGER ALL;
ALTER TABLE public."post_tag" DISABLE TRIGGER ALL;
ALTER TABLE public."post_favorite" DISABLE TRIGGER ALL;
ALTER TABLE public."post_feature" DISABLE TRIGGER ALL;
ALTER TABLE public."post_note" DISABLE TRIGGER ALL;
ALTER TABLE public."post_score" DISABLE TRIGGER ALL;
ALTER TABLE public."comment" DISABLE TRIGGER ALL;
ALTER TABLE public."comment_score" DISABLE TRIGGER ALL;
ALTER TABLE public."pool_category" DISABLE TRIGGER ALL;
ALTER TABLE public."pool" DISABLE TRIGGER ALL;
ALTER TABLE public."pool_name" DISABLE TRIGGER ALL;
ALTER TABLE public."pool_post" DISABLE TRIGGER ALL;

-- ================================== Users ================================== --
-- last_login_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."user"
SET "last_login_time" = CURRENT_TIMESTAMP
WHERE "last_login_time" IS NULL;

-- Ranks in Oxibooru are represented by SMALLINT so we have to convert
UPDATE public."user"
SET "rank" = CASE "rank"
    WHEN 'anonymous' THEN 0
    WHEN 'restricted' THEN 1
    WHEN 'regular' THEN 2
    WHEN 'power' THEN 3
    WHEN 'moderator' THEN 4
    WHEN 'administrator' THEN 5
END;
ALTER TABLE public."user"
ALTER COLUMN "rank" TYPE SMALLINT USING "rank"::SMALLINT;

-- Avatar styles in Oxibooru are represented by SMALLINT so we have to convert
UPDATE public."user"
SET "avatar_style" = CASE "avatar_style"
    WHEN 'gravatar' THEN 0
    WHEN 'manual' THEN 1
END;
ALTER TABLE public."user"
ALTER COLUMN "avatar_style" TYPE SMALLINT USING "avatar_style"::SMALLINT;

-- Insert into Oxibooru table
-- Password hash and salt won't be useful in Oxibooru, but there's nothing we can really do about that
INSERT INTO oxi."user" ("id", "name", "password_hash", "password_salt", "email", "rank", "avatar_style", "creation_time", "last_login_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "name", "password_hash", "password_salt", "email", "rank", "avatar_style", "creation_time" AT TIME ZONE 'UTC', "last_login_time" AT TIME ZONE 'UTC', CURRENT_TIMESTAMP FROM public."user";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.user', 'id'), GREATEST((SELECT MAX("id") FROM oxi."user"), 1));

-- ============================= Tag Categories ============================== --
-- First, set category_id of tags in default category to 0 (the id of the default category in Oxibooru)
ALTER TABLE public."tag" DROP CONSTRAINT "tag_category_id_fkey";
UPDATE public."tag"
SET "category_id" = 0
FROM public."tag_category"
WHERE public."tag"."category_id" = public."tag_category"."id" AND public."tag_category"."default" = true;

-- Then, update Oxibooru default category with properties of Szurubooru default category
UPDATE oxi."tag_category"
SET "order" = public."tag_category"."order",
    "name" = public."tag_category"."name",
    "color" = public."tag_category"."color"
FROM public."tag_category"
WHERE public."tag_category"."default" = true;

-- Lastly, remove Szurubooru default category
DELETE FROM public."tag_category"
WHERE "default" = true;

-- Insert into Oxibooru table
INSERT INTO oxi."tag_category" ("id", "order", "name", "color", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "order", "name", "color", CURRENT_TIMESTAMP FROM public."tag_category";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.tag_category', 'id'), GREATEST((SELECT MAX("id") FROM oxi."tag_category"), 1));

-- ================================== Tags =================================== --
-- Move tags that do not have valid category_id to default category
UPDATE public."tag"
SET "category_id" = 0
WHERE NOT EXISTS (
    SELECT 1 FROM public."tag_category" WHERE "tag_category"."id" = "tag"."category_id"
);

-- Descriptions are non-nullable in Oxibooru, so replace NULL values with empty description
UPDATE public."tag"
SET "description" = ''
WHERE "description" IS NULL;

-- last_edit_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."tag"
SET "last_edit_time" = CURRENT_TIMESTAMP
WHERE "last_edit_time" IS NULL;

-- Insert into Oxibooru table
INSERT INTO oxi."tag" ("id", "category_id", "description", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "category_id", "description", "creation_time" AT TIME ZONE 'UTC', "last_edit_time" AT TIME ZONE 'UTC' FROM public."tag";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.tag', 'id'), GREATEST((SELECT MAX("id") FROM oxi."tag"), 1));

-- ================================ Tag Names ================================ --
-- Remove tag names that do not have a valid tag_id
DELETE FROM public."tag_name"
WHERE NOT EXISTS (
    SELECT 1 FROM public."tag" WHERE "tag"."id" = "tag_name"."tag_id"
);

-- Create temporary table to deduplicate case insensitive names
CREATE TABLE "ci_tag_name" (
    "tag_id" INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "dup_count" INTEGER NOT NULL DEFAULT 0
);

-- Add indexes or this part takes forever
CREATE INDEX ON "ci_tag_name" (LOWER("name"));
CREATE INDEX ON public."tag_name" (LOWER("name"));

INSERT INTO "ci_tag_name" ("tag_id", "order", "name")
SELECT "tag_id", ROW_NUMBER() OVER (PARTITION BY "tag_id" ORDER BY "ord") AS new_order, "name" FROM public."tag_name";

UPDATE "ci_tag_name"
SET "dup_count" = (SELECT COUNT(*) FROM public."tag_name" WHERE LOWER(public."tag_name"."name") = LOWER("ci_tag_name"."name"));

UPDATE "ci_tag_name"
SET "name" = CONCAT("name", '_name_modified_', "tag_id", '_', "order")
WHERE "dup_count" > 1;

-- Insert into Oxibooru table
INSERT INTO oxi."tag_name" ("tag_id", "order", "name")
SELECT "tag_id", "order" - 1, "name" FROM "ci_tag_name";

DROP TABLE "ci_tag_name";

-- ============================ Tag Implications ============================= --
-- Remove tag implications that do not have valid parent/child ids
DELETE FROM public."tag_implication"
WHERE NOT EXISTS (
    SELECT 1 FROM public."tag" WHERE "tag"."id" = "tag_implication"."parent_id"
);

DELETE FROM public."tag_implication"
WHERE NOT EXISTS (
    SELECT 1 FROM public."tag" WHERE "tag"."id" = "tag_implication"."child_id"
);

-- Insert into Oxibooru table
INSERT INTO oxi."tag_implication" ("parent_id", "child_id")
SELECT "parent_id", "child_id" FROM public."tag_implication";

-- ============================= Tag Suggestions ============================= --
-- Remove tag suggestions that do not have valid parent/child ids
DELETE FROM public."tag_suggestion"
WHERE NOT EXISTS (
    SELECT 1 FROM public."tag" WHERE "tag"."id" = "tag_suggestion"."parent_id"
);

DELETE FROM public."tag_suggestion"
WHERE NOT EXISTS (
    SELECT 1 FROM public."tag" WHERE "tag"."id" = "tag_suggestion"."child_id"
);

-- Insert into Oxibooru table
INSERT INTO oxi."tag_suggestion" ("parent_id", "child_id")
SELECT "parent_id", "child_id" FROM public."tag_suggestion";

-- ================================== Posts ================================== --
-- File sizes are non-nullable in Oxibooru, so replace NULL values with 0
UPDATE public."post"
SET "file_size" = 0
WHERE "file_size" IS NULL;

-- Post widths are non-nullable in Oxibooru, so replace NULL values with 0
UPDATE public."post"
SET "image_width" = 0
WHERE "image_width" IS NULL;

-- Post heights are non-nullable in Oxibooru, so replace NULL values with 0
UPDATE public."post"
SET "image_height" = 0
WHERE "image_height" IS NULL;

-- Post checksums are UNIQUE BYTEA in Oxibooru, so we have to deduplicate and convert (they will be recalculated anyway)
UPDATE public."post"
SET "checksum" = CONCAT(RANDOM(), "id");
ALTER TABLE public."post"
ALTER COLUMN "checksum" TYPE BYTEA USING "checksum"::BYTEA;

-- Post MD5 checksums are non-nullable BYTEA in Oxibooru, so replace NULL values with '' and convert
UPDATE public."post"
SET "checksum_md5" = ''
WHERE "checksum_md5" IS NULL;
ALTER TABLE public."post"
ALTER COLUMN "checksum_md5" TYPE BYTEA USING "checksum_md5"::BYTEA;

-- Post sources are non-nullable in Oxibooru, so replace NULL values with ''
UPDATE public."post"
SET "source" = ''
WHERE "source" IS NULL;

-- last_edit_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."post"
SET "last_edit_time" = CURRENT_TIMESTAMP
WHERE "last_edit_time" IS NULL;

-- Safety ratings in Oxibooru are represented by SMALLINT so we have to convert
UPDATE public."post"
SET "safety" = CASE "safety"
    WHEN 'safe' THEN 0
    WHEN 'sketchy' THEN 1
    WHEN 'questionable' THEN 1
    WHEN 'unsafe' THEN 2
END;
ALTER TABLE public."post"
ALTER COLUMN "safety" TYPE SMALLINT USING "safety"::SMALLINT;

-- Post types in Oxibooru are represented by SMALLINT so we have to convert
UPDATE public."post"
SET "type" = CASE
    WHEN "type" = 'image' THEN 0
    WHEN "type" = 'animation' THEN 1
    WHEN "type" = 'video' THEN 2
    WHEN "type" = 'flash' THEN 3
END;
ALTER TABLE public."post"
ALTER COLUMN "type" TYPE SMALLINT USING "type"::SMALLINT;

-- MIME types in Oxibooru are represented by SMALLINT so we have to convert
UPDATE public."post"
SET "mime-type" = CASE
    WHEN "mime-type" = 'image/bmp' THEN 0
    WHEN "mime-type" = 'image/gif' THEN 1
    WHEN "mime-type" = 'image/jpeg' THEN 2
    WHEN "mime-type" = 'image/png' THEN 3
    WHEN "mime-type" = 'image/webp' THEN 4
    WHEN "mime-type" = 'video/mp4' THEN 5
    WHEN "mime-type" = 'video/quicktime' THEN 6
    WHEN "mime-type" = 'video/webm' THEN 7
    WHEN "mime-type" = 'application/x-shockwave-flash' THEN 8
END;
ALTER TABLE public."post"
ALTER COLUMN "mime-type" TYPE SMALLINT USING "mime-type"::SMALLINT;

-- Post flags in Oxibooru are represented by SMALLINT so we have to convert
UPDATE public."post"
SET "flags" = CASE
    WHEN "flags" = NULL THEN 0
    WHEN "flags" = '' THEN 0
    WHEN "flags" = 'loop' THEN 1
    WHEN "flags" = 'sound' THEN 2
    WHEN "flags" = 'loop,sound' THEN 3
    WHEN "flags" = 'sound,loop' THEN 3
END;
ALTER TABLE public."post"
ALTER COLUMN "flags" TYPE SMALLINT USING "flags"::SMALLINT;

-- Insert into Oxibooru table
INSERT INTO oxi."post" ("id", "user_id", "file_size", "width", "height", "safety", "type", "mime_type", "checksum", "checksum_md5", "flags", "source", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "user_id", "file_size", "image_width", "image_height", "safety", "type", "mime-type", "checksum", "checksum_md5", "flags", "source", "creation_time" AT TIME ZONE 'UTC', "last_edit_time" AT TIME ZONE 'UTC' FROM public."post";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.post', 'id'), GREATEST((SELECT MAX("id") FROM oxi."post"), 1));

-- ============================= Post Relations ============================== --
-- Remove post relations that do not have valid parent/child ids
DELETE FROM public."post_relation"
WHERE NOT EXISTS (
    SELECT 1 FROM public."post" WHERE "post"."id" = "post_relation"."parent_id"
);

DELETE FROM public."post_relation"
WHERE NOT EXISTS (
    SELECT 1 FROM public."post" WHERE "post"."id" = "post_relation"."child_id"
);

-- Insert into Oxibooru table
INSERT INTO oxi."post_relation" ("parent_id", "child_id")
SELECT "parent_id", "child_id" FROM public."post_relation";

-- ================================ Post Tags ================================ --
-- Remove post relations that do not have valid post/tag ids
DELETE FROM public."post_tag"
WHERE NOT EXISTS (
    SELECT 1 FROM public."post" WHERE "post"."id" = "post_tag"."post_id"
);

DELETE FROM public."post_tag"
WHERE NOT EXISTS (
    SELECT 1 FROM public."tag" WHERE "tag"."id" = "post_tag"."tag_id"
);

-- Insert into Oxibooru table
INSERT INTO oxi."post_tag" ("post_id", "tag_id")
SELECT "post_id", "tag_id" FROM public."post_tag";

-- ============================= Post Favorites ============================== --
-- Remove post favorites that do not have valid post/user ids
DELETE FROM public."post_favorite"
WHERE NOT EXISTS (
    SELECT 1 FROM public."post" WHERE "post"."id" = "post_favorite"."post_id"
);

DELETE FROM public."post_favorite"
WHERE NOT EXISTS (
    SELECT 1 FROM public."user" WHERE "user"."id" = "post_favorite"."user_id"
);

-- Insert into Oxibooru table
INSERT INTO oxi."post_favorite" ("post_id", "user_id", "time")
SELECT "post_id", "user_id", "time" AT TIME ZONE 'UTC' FROM public."post_favorite";

-- ============================== Post Feature =============================== --
-- Remove post features that do not have valid post/user ids
DELETE FROM public."post_feature"
WHERE NOT EXISTS (
    SELECT 1 FROM public."post" WHERE "post"."id" = "post_feature"."post_id"
);

DELETE FROM public."post_feature"
WHERE NOT EXISTS (
    SELECT 1 FROM public."user" WHERE "user"."id" = "post_feature"."user_id"
);

-- Insert into Oxibooru table
INSERT INTO oxi."post_feature" ("id", "post_id", "user_id", "time") OVERRIDING SYSTEM VALUE
SELECT "id", "post_id", "user_id", "time" AT TIME ZONE 'UTC' FROM public."post_feature";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.post_feature', 'id'), GREATEST((SELECT MAX("id") FROM oxi."post_feature"), 1));

-- ================================ Post Note ================================ --
-- Create an unpickle helper
-- Returns 1D array because multi-dimensional arrays are not supported in diesel
CREATE FUNCTION unpickle_to_array(raw BYTEA)
  RETURNS REAL[]
  LANGUAGE plpython3u
AS $$
import pickle

# Extract bytes
if hasattr(raw, 'tobytes'):
    data_bytes = raw.tobytes()
else:
    data_bytes = raw

# Unpickle into Python object
try:
    pts = pickle.loads(data_bytes)
except Exception as e:
    plpy.error(f"Could not unpickle post_note.polygon: {e!s}")

# pts should be a list of [x, y] pairs; flatten into [x0, y0, x1, y1, ...]
try:
    flat = []
    for p in pts:
        if len(p) != 2:
            raise ValueError(f"Expected 2 elements per point, got {len(p)}")
        flat.extend([float(p[0]), float(p[1])])
except Exception as e:
    plpy.error(f"Invalid polygon shape: {e!s}")

return flat
$$;

-- Remove post notes that do not have valid post ids
DELETE FROM public."post_note"
WHERE NOT EXISTS (
    SELECT 1 FROM public."post" WHERE "post"."id" = "post_note"."post_id"
);

-- Polygons are represented as REAL[][2] in Oxibooru, so we need to convert
ALTER TABLE public."post_note" ADD COLUMN "shape" REAL[][2];

UPDATE public."post_note"
SET "shape" = unpickle_to_array("polygon");

-- Insert into Oxibooru table
INSERT INTO oxi."post_note" ("id", "post_id", "polygon", "text") OVERRIDING SYSTEM VALUE
SELECT "id", "post_id", "shape", "text" FROM public."post_note";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.post_note', 'id'), GREATEST((SELECT MAX("id") FROM oxi."post_note"), 1));

DROP FUNCTION unpickle_to_array;

-- =============================== Post Score ================================ --
-- Remove post scores that do not have valid post/user ids
DELETE FROM public."post_score"
WHERE NOT EXISTS (
    SELECT 1 FROM public."post" WHERE "post"."id" = "post_score"."post_id"
);

DELETE FROM public."post_score"
WHERE NOT EXISTS (
    SELECT 1 FROM public."user" WHERE "user"."id" = "post_score"."user_id"
);

-- Insert into Oxibooru table
INSERT INTO oxi."post_score" ("post_id", "user_id", "score", "time")
SELECT "post_id", "user_id", "score", "time" AT TIME ZONE 'UTC' FROM public."post_score";

-- ================================ Comments ================================= --
-- Remove comments that do not have valid user/post ids
DELETE FROM public."comment"
WHERE NOT EXISTS (
    SELECT 1 FROM public."user" WHERE "user"."id" = "comment"."user_id"
);

DELETE FROM public."comment"
WHERE NOT EXISTS (
    SELECT 1 FROM public."post" WHERE "post"."id" = "comment"."post_id"
);

-- Comment text is non-nullable in Oxibooru, so replace NULL values with an empty string
UPDATE public."comment"
SET "text" = ''
WHERE "text" IS NULL;

-- last_edit_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."comment"
SET "last_edit_time" = CURRENT_TIMESTAMP
WHERE "last_edit_time" IS NULL;

-- Insert into Oxibooru table
INSERT INTO oxi."comment" ("id", "user_id", "post_id", "text", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "user_id", "post_id", "text", "creation_time" AT TIME ZONE 'UTC', "last_edit_time" AT TIME ZONE 'UTC' FROM public."comment";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.comment', 'id'), GREATEST((SELECT MAX("id") FROM oxi."comment"), 1));

-- ============================= Comment Scores ============================== --
-- Remove comment scores that do not have valid comment/user ids
DELETE FROM public."comment_score"
WHERE NOT EXISTS (
    SELECT 1 FROM public."comment" WHERE "comment"."id" = "comment_score"."comment_id"
);

DELETE FROM public."comment_score"
WHERE NOT EXISTS (
    SELECT 1 FROM public."user" WHERE "user"."id" = "comment_score"."user_id"
);

-- Insert into Oxibooru table
INSERT INTO oxi."comment_score" ("comment_id", "user_id", "score", "time")
SELECT "comment_id", "user_id", "score", "time" AT TIME ZONE 'UTC' FROM public."comment_score";

-- ============================= Pool Categories ============================= --
-- First, set category_id of pools in default category to 0 (the id of the default category in Oxibooru)
ALTER TABLE public."pool" DROP CONSTRAINT "pool_category_id_fkey";
UPDATE public."pool"
SET "category_id" = 0
FROM public."pool_category"
WHERE public."pool"."category_id" = public."pool_category"."id" AND public."pool_category"."default" = true;

-- Then, update Oxibooru default category with properties of Szurubooru default category
UPDATE oxi."pool_category"
SET "name" = public."pool_category"."name",
    "color" = public."pool_category"."color"
FROM public."pool_category"
WHERE public."pool_category"."default" = true;

-- Lastly, remove Szurubooru default category
DELETE FROM public."pool_category"
WHERE "default" = true;

-- Insert into Oxibooru table
INSERT INTO oxi."pool_category" ("id", "name", "color", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "name", "color", CURRENT_TIMESTAMP FROM public."pool_category";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.pool_category', 'id'), GREATEST((SELECT MAX("id") FROM oxi."pool_category"), 1));

-- ================================== Pools ================================== --
-- Move tags that do not have valid category_id to default category
UPDATE public."pool"
SET "category_id" = 0
WHERE NOT EXISTS (
    SELECT 1 FROM public."pool_category" WHERE "pool_category"."id" = "pool"."category_id"
);

-- Descriptions are non-nullable in Oxibooru, so replace NULL values with empty description
UPDATE public."pool"
SET "description" = ''
WHERE "description" IS NULL;

-- last_edit_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."pool"
SET "last_edit_time" = CURRENT_TIMESTAMP
WHERE "last_edit_time" IS NULL;

-- Insert into Oxibooru table
INSERT INTO oxi."pool" ("id", "category_id", "description", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "category_id", "description", "creation_time" AT TIME ZONE 'UTC', "last_edit_time" AT TIME ZONE 'UTC' FROM public."pool";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.pool', 'id'), GREATEST((SELECT MAX("id") FROM oxi."pool"), 1));

-- =============================== Pool Names ================================ --
-- Remove pool names that do not have a valid pool_id
DELETE FROM public."pool_name"
WHERE NOT EXISTS (
    SELECT 1 FROM public."pool" WHERE "pool"."id" = "pool_name"."pool_id"
);

-- Create temporary table to deduplicate case insensitive names
CREATE TABLE "ci_pool_name" (
    "pool_id" INTEGER NOT NULL,
    "order" INTEGER NOT NULL,
    "name" TEXT NOT NULL,
    "dup_count" INTEGER NOT NULL DEFAULT 0
);

-- Add indexes or this part takes forever
CREATE INDEX ON "ci_pool_name" (LOWER("name"));
CREATE INDEX ON public."pool_name" (LOWER("name"));

INSERT INTO "ci_pool_name" ("pool_id", "order", "name")
SELECT "pool_id", ROW_NUMBER() OVER (PARTITION BY "pool_id" ORDER BY "ord") AS new_order, "name" FROM public."pool_name";

UPDATE "ci_pool_name"
SET "dup_count" = (SELECT COUNT(*) FROM public."pool_name" WHERE LOWER(public."pool_name"."name") = LOWER("ci_pool_name"."name"));

UPDATE "ci_pool_name"
SET "name" = CONCAT("name", '_name_modified_', "pool_id", '_', "order")
WHERE "dup_count" > 1;

-- Insert into Oxibooru table
INSERT INTO oxi."pool_name" ("pool_id", "order", "name")
SELECT "pool_id", "order" - 1, "name" FROM "ci_pool_name";

DROP TABLE "ci_pool_name";

-- =============================== Pool Posts ================================ --
-- Remove pool posts that do not have valid pool/post ids
DELETE FROM public."pool_post"
WHERE NOT EXISTS (
    SELECT 1 FROM public."pool" WHERE "pool"."id" = "pool_post"."pool_id"
);

DELETE FROM public."pool_post"
WHERE NOT EXISTS (
    SELECT 1 FROM public."post" WHERE "post"."id" = "pool_post"."post_id"
);

-- Insert into Oxibooru table
INSERT INTO oxi."pool_post" ("pool_id", "post_id", "order")
SELECT "pool_id", "post_id", "ord" FROM public."pool_post";

-- ================================ Snapshots ================================ --
-- Create an unpickle helper
CREATE FUNCTION unpickle_to_jsonb(raw BYTEA)
  RETURNS JSONB
  LANGUAGE plpython3u
AS $$
import pickle, json

# raw may already be bytes, or a memoryview/buffer
if hasattr(raw, 'tobytes'):
    data_bytes = raw.tobytes()
else:
    data_bytes = raw

try:
    py_obj = pickle.loads(data_bytes)
except Exception as e:
    plpy.error(f"Could not unpickle snapshot data: {e!s}")

# Dump to JSON text; Postgres will cast it to JSONB automatically
return json.dumps(py_obj)
$$;

-- The resource_pkey and resource_name columns have been combined in Oxibooru, so we do that here
UPDATE public."snapshot"
SET "resource_name" = "resource_pkey"
WHERE "resource_type" IN ('pool', 'post');

-- Snapshot operations in Oxibooru are represented by SMALLINT so we have to convert
UPDATE public."snapshot"
SET "operation" = CASE
    WHEN "operation" = 'created' THEN 0
    WHEN "operation" = 'modified' THEN 1
    WHEN "operation" = 'merged' THEN 2
    WHEN "operation" = 'deleted' THEN 3
END;
ALTER TABLE public."snapshot"
ALTER COLUMN "operation" TYPE SMALLINT USING "operation"::SMALLINT;

-- Resource types in Oxibooru are represented by SMALLINT so we have to convert
UPDATE public."snapshot"
SET "resource_type" = CASE
    WHEN "resource_type" = 'pool' THEN 1
    WHEN "resource_type" = 'pool_category' THEN 2
    WHEN "resource_type" = 'post' THEN 3
    WHEN "resource_type" = 'tag' THEN 4
    WHEN "resource_type" = 'tag_category' THEN 5
END;
ALTER TABLE public."snapshot"
ALTER COLUMN "resource_type" TYPE SMALLINT USING "resource_type"::SMALLINT;

-- Snapshot data is non-nullable JSONB in Oxibooru, so we need to convert
ALTER TABLE public."snapshot" ADD COLUMN "json_data" JSONB;

UPDATE public."snapshot"
SET "json_data" = unpickle_to_jsonb("data")
WHERE "data" IS NOT NULL;

UPDATE public."snapshot"
SET "json_data" = '{}'::JSONB
WHERE "json_data" IS NULL;

-- Insert into Oxibooru table
INSERT INTO oxi."snapshot" ("id", "user_id", "operation", "resource_type", "resource_id", "data", "creation_time") OVERRIDING SYSTEM VALUE
SELECT "id", "user_id", "operation", "resource_type", "resource_name", "json_data", "creation_time" AT TIME ZONE 'UTC' FROM public."snapshot";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.snapshot', 'id'), GREATEST((SELECT MAX("id") FROM oxi."snapshot"), 1));

DROP FUNCTION unpickle_to_jsonb;

-- ================================= Cleanup ================================= --
-- Re-enable triggers
ALTER TABLE oxi."user" ENABLE TRIGGER USER;
ALTER TABLE oxi."tag_category" ENABLE TRIGGER USER;
ALTER TABLE oxi."tag" ENABLE TRIGGER USER;
ALTER TABLE oxi."tag_name" ENABLE TRIGGER USER;
ALTER TABLE oxi."tag_implication" ENABLE TRIGGER USER;
ALTER TABLE oxi."tag_suggestion" ENABLE TRIGGER USER;
ALTER TABLE oxi."post" ENABLE TRIGGER USER;
ALTER TABLE oxi."post_relation" ENABLE TRIGGER USER;
ALTER TABLE oxi."post_tag" ENABLE TRIGGER USER;
ALTER TABLE oxi."post_favorite" ENABLE TRIGGER USER;
ALTER TABLE oxi."post_feature" ENABLE TRIGGER USER;
ALTER TABLE oxi."post_note" ENABLE TRIGGER USER;
ALTER TABLE oxi."post_score" ENABLE TRIGGER USER;
ALTER TABLE oxi."comment" ENABLE TRIGGER USER;
ALTER TABLE oxi."comment_score" ENABLE TRIGGER USER;
ALTER TABLE oxi."pool_category" ENABLE TRIGGER USER;
ALTER TABLE oxi."pool" ENABLE TRIGGER USER;
ALTER TABLE oxi."pool_name" ENABLE TRIGGER USER;
ALTER TABLE oxi."pool_post" ENABLE TRIGGER USER;

-- Drop python extension
DROP EXTENSION plpython3u;

-- Drop Szurubooru schema
DROP SCHEMA public CASCADE;
ALTER SCHEMA oxi RENAME TO public;

-- ================================ Statistics ================================ --
-- Database statistics
UPDATE "database_statistics"
SET "disk_usage" = (SELECT COALESCE(SUM("file_size"), 0) FROM "post"),
    "comment_count" = (SELECT COUNT(*) FROM "comment"),
    "pool_count" = (SELECT COUNT(*) FROM "pool"),
    "post_count" = (SELECT COUNT(*) FROM "post"),
    "tag_count" = (SELECT COUNT(*) FROM "tag"),
    "user_count" = (SELECT COUNT(*) FROM "user");

-- Comment statistics
INSERT INTO "comment_statistics" ("comment_id", "score")
SELECT "id", SUM(COALESCE("score", 0)) FROM "comment"
LEFT JOIN "comment_score" ON "comment_id" = "id"
GROUP BY "id";

-- Pool category statistics
UPDATE "pool_category_statistics"
SET "usage_count" = (SELECT COUNT(*) FROM "pool" WHERE "category_id" = 0)
WHERE "category_id" = 0;

INSERT INTO "pool_category_statistics" ("category_id", "usage_count")
SELECT "pool_category"."id", COUNT("pool"."id") FROM "pool_category"
LEFT JOIN "pool" ON "pool"."category_id" = "pool_category"."id"
WHERE "pool_category"."id" != 0
GROUP BY "pool_category"."id";

-- Pool statistics
INSERT INTO "pool_statistics" ("pool_id", "post_count")
SELECT "id", COUNT("post_id") FROM "pool"
LEFT JOIN "pool_post" ON "pool_id" = "id"
GROUP BY "id";

-- Post statistics
INSERT INTO "post_statistics" ("post_id", "tag_count", "pool_count", "note_count", "comment_count", "relation_count", "score", 
                               "favorite_count", "feature_count", "last_comment_time", "last_favorite_time", "last_feature_time")
SELECT tag_count."id", tag_count.count, pool_count.count, note_count.count, comment_count.count, relation_count.count, score.sum,
       favorite_count.count, feature_count.count, last_comment_time.t, last_favorite_time.t, last_feature_time.t FROM
    (SELECT "id", COUNT("tag_id") as count FROM "post"
     LEFT JOIN "post_tag" ON "post_id" = "id"
     GROUP BY "id") tag_count
INNER JOIN
    (SELECT "post"."id", COUNT("post_note"."id") as count FROM "post"
     LEFT JOIN "post_note" ON "post_note"."id" = "post"."id"
     GROUP BY "post"."id") note_count ON note_count."id" = tag_count."id"
INNER JOIN
    (SELECT "id", COUNT("pool_id") as count FROM "post"
     LEFT JOIN "pool_post" ON "post_id" = "id"
     GROUP BY "id") pool_count ON pool_count."id" = tag_count."id"
INNER JOIN
    (SELECT "post"."id", COUNT("comment"."id") as count FROM "post"
     LEFT JOIN "comment" ON "comment"."id" = "post"."id"
     GROUP BY "post"."id") comment_count ON comment_count."id" = tag_count."id"
INNER JOIN
    (SELECT "id", COUNT("child_id") as count FROM "post"
     LEFT JOIN "post_relation" ON "parent_id" = "id"
     GROUP BY "id") relation_count ON relation_count."id" = tag_count."id"
INNER JOIN
    (SELECT "id", SUM(COALESCE("score", 0)) as sum FROM "post"
     LEFT JOIN "post_score" ON "post_id" = "id"
     GROUP BY "id") score ON score."id" = tag_count."id"
INNER JOIN
    (SELECT "id", COUNT("post_favorite"."user_id") as count FROM "post"
     LEFT JOIN "post_favorite" ON "post_id" = "id"
     GROUP BY "id") favorite_count ON favorite_count."id" = tag_count."id"
INNER JOIN
    (SELECT "post"."id", COUNT("post_feature"."user_id") as count FROM "post"
     LEFT JOIN "post_feature" ON "post_feature"."post_id" = "post"."id"
     GROUP BY "post"."id") feature_count ON feature_count."id" = tag_count."id"
INNER JOIN
    (SELECT "post"."id", MAX("comment"."creation_time") as t FROM "post"
     LEFT JOIN "comment" on "comment"."post_id" = "post"."id"
     GROUP BY "post"."id") last_comment_time ON last_comment_time."id" = tag_count."id"
INNER JOIN
    (SELECT "id", MAX("time") as t FROM "post"
     LEFT JOIN "post_favorite" ON "post_id" = "id"
     GROUP BY "id") last_favorite_time ON last_favorite_time."id" = tag_count."id"
INNER JOIN
    (SELECT "post"."id", MAX("post_feature"."time") as t FROM "post"
     LEFT JOIN "post_feature" ON "post_feature"."post_id" = "post"."id"
     GROUP BY "post"."id") last_feature_time ON last_feature_time."id" = tag_count."id";

-- Tag category statistics
UPDATE "tag_category_statistics"
SET "usage_count" = (SELECT COUNT(*) FROM "tag" WHERE "category_id" = 0)
WHERE "category_id" = 0;

INSERT INTO "tag_category_statistics" ("category_id", "usage_count")
SELECT "tag_category"."id", COUNT("tag"."id") FROM "tag_category"
LEFT JOIN "tag" ON "tag"."category_id" = "tag_category"."id"
WHERE "tag_category"."id" != 0
GROUP BY "tag_category"."id";

-- Tag statistics
INSERT INTO "tag_statistics" ("tag_id", "usage_count", "implication_count", "suggestion_count")
SELECT usage_count."id", usage_count.count, implication_count.count, suggestion_count.count FROM 
    (SELECT "id", COUNT("post_id") as count FROM "tag" 
     LEFT JOIN "post_tag" ON "tag_id" = "id"
     GROUP BY "id") usage_count
INNER JOIN
    (SELECT "id", COUNT("child_id") as count FROM "tag"
     LEFT JOIN "tag_implication" ON "parent_id" = "id"
     GROUP BY "id") implication_count ON implication_count."id" = usage_count."id"
INNER JOIN
    (SELECT "id", COUNT("child_id") as count FROM "tag"
     LEFT JOIN "tag_suggestion" ON "parent_id" = "id"
     GROUP BY "id") suggestion_count ON suggestion_count."id" = usage_count."id";

-- User statistics
INSERT INTO "user_statistics" ("user_id", "comment_count", "favorite_count", "upload_count")
SELECT comment_count."id", comment_count.count, favorite_count.count, upload_count.count FROM 
    (SELECT "user"."id", COUNT("comment"."id") as count FROM "user"
     LEFT JOIN "comment" ON "user_id" = "user"."id"
     GROUP BY "user"."id") comment_count
INNER JOIN
    (SELECT "id", COUNT("post_id") as count FROM "user"
     LEFT JOIN "post_favorite" ON "user_id" = "id"
     GROUP BY "id") favorite_count ON favorite_count."id" = comment_count."id"
INNER JOIN
    (SELECT "user"."id", COUNT("post"."id") as count FROM "user"
     LEFT JOIN "post" ON "post"."user_id" = "user"."id"
     GROUP BY "user"."id") upload_count ON upload_count."id" = comment_count."id";