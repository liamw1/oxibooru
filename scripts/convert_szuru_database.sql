-- ================================== Users ================================== --
-- last_login_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."user"
SET "last_login_time" = CURRENT_TIMESTAMP
WHERE "last_login_time" IS NULL;

-- Ranks in Oxibooru are represent by SMALLINT so we have to convert
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

-- Avatar styles in Oxibooru are represent by SMALLINT so we have to convert
UPDATE public."user"
SET "avatar_style" = CASE "avatar_style"
    WHEN 'gravatar' THEN 0
    WHEN 'manual' THEN 1
END;
ALTER TABLE public."user"
ALTER COLUMN "avatar_style" TYPE SMALLINT USING "avatar_style"::SMALLINT;

-- Password hash and salt won't be useful in Oxibooru, but there's nothing we can really do about that
INSERT INTO oxi."user" ("id", "name", "password_hash", "password_salt", "email", "rank", "avatar_style", "creation_time", "last_login_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "name", "password_hash", "password_salt", "email", "rank", "avatar_style", "creation_time" AT TIME ZONE 'UTC', "last_login_time" AT TIME ZONE 'UTC', CURRENT_TIMESTAMP FROM public."user";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.user', 'id'), (SELECT MAX("id") FROM oxi."user"));

-- ============================= Tag Categories ============================== --
-- First, set category_id of tags in default category to 0 (the id of the default category in Oxibooru)
ALTER TABLE public."tag" DROP CONSTRAINT "tag_category_id_fkey";
UPDATE public."tag"
SET "category_id" = 0
FROM public."tag_category"
WHERE public."tag"."category_id" = public."tag_category"."id"
AND public."tag_category"."default" = true;

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

INSERT INTO oxi."tag_category" ("id", "order", "name", "color", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "order", "name", "color", CURRENT_TIMESTAMP FROM public."tag_category";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.tag_category', 'id'), (SELECT MAX("id") FROM oxi."tag_category"));

-- ================================== Tags =================================== --
-- Descriptions are non-nullable in Oxibooru, so replace NULL values with empty description
UPDATE public."tag"
SET "description" = ''
WHERE "description" IS NULL;

-- last_edit_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."tag"
SET "last_edit_time" = CURRENT_TIMESTAMP
WHERE "last_edit_time" IS NULL;

INSERT INTO oxi."tag" ("id", "category_id", "description", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "category_id", "description", "creation_time" AT TIME ZONE 'UTC', "last_edit_time" AT TIME ZONE 'UTC' FROM public."tag";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.tag', 'id'), (SELECT MAX("id") FROM oxi."tag"));

-- ================================ Tag Names ================================ --
INSERT INTO oxi."tag_name" ("tag_id", "order", "name")
SELECT "tag_id", "ord", "name" FROM public."tag_name";

-- ============================ Tag Implications ============================= --
INSERT INTO oxi."tag_implication" ("parent_id", "child_id")
SELECT "parent_id", "child_id" FROM public."tag_implication";

-- ============================= Tag Suggestions ============================= --
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

-- last_edit_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."post"
SET "last_edit_time" = CURRENT_TIMESTAMP
WHERE "last_edit_time" IS NULL;

-- Safety ratings in Oxibooru are represent by SMALLINT so we have to convert
UPDATE public."post"
SET "safety" = CASE "safety"
    WHEN 'safe' THEN 0
    WHEN 'sketchy' THEN 1
    WHEN 'questionable' THEN 1
    WHEN 'unsafe' THEN 2
END;
ALTER TABLE public."post"
ALTER COLUMN "safety" TYPE SMALLINT USING "safety"::SMALLINT;

-- Post types in Oxibooru are represent by SMALLINT so we have to convert
UPDATE public."post"
SET "type" = CASE
    WHEN "type" = 'image' THEN 0
    WHEN "type" = 'animation' THEN 1
    WHEN "type" = 'video' THEN 2
END;
ALTER TABLE public."post"
ALTER COLUMN "type" TYPE SMALLINT USING "type"::SMALLINT;

-- MIME types in Oxibooru are represent by SMALLINT so we have to convert
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
END;
ALTER TABLE public."post"
ALTER COLUMN "mime-type" TYPE SMALLINT USING "mime-type"::SMALLINT;

-- Post flags in Oxibooru are represent by SMALLINT so we have to convert
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

INSERT INTO oxi."post" ("id", "user_id", "file_size", "width", "height", "safety", "type", "mime_type", "checksum", "checksum_md5", "flags", "source", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "user_id", "file_size", "image_width", "image_height", "safety", "type", "mime-type", "checksum", "checksum_md5", "flags", "source", "creation_time" AT TIME ZONE 'UTC', "last_edit_time" AT TIME ZONE 'UTC' FROM public."post";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.post', 'id'), (SELECT MAX("id") FROM oxi."post"));

-- ============================= Post Relations ============================== --
INSERT INTO oxi."post_relation" ("parent_id", "child_id")
SELECT "parent_id", "child_id" FROM public."post_relation";

-- ================================ Post Tags ================================ --
INSERT INTO oxi."post_tag" ("post_id", "tag_id")
SELECT "post_id", "tag_id" FROM public."post_tag";

-- ============================= Post Favorites ============================== --
INSERT INTO oxi."post_favorite" ("post_id", "user_id", "time")
SELECT "post_id", "user_id", "time" AT TIME ZONE 'UTC' FROM public."post_favorite";

-- ============================== Post Feature =============================== --
INSERT INTO oxi."post_feature" ("id", "post_id", "user_id", "time") OVERRIDING SYSTEM VALUE
SELECT "id", "post_id", "user_id", "time" AT TIME ZONE 'UTC' FROM public."post_feature";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.post_feature', 'id'), (SELECT MAX("id") FROM oxi."post_feature"));

-- ================================ Post Note ================================ --
-- Converting the polygon column to REAL[][2] is hard, so I'm omitting post note conversion for now
--INSERT INTO oxi."post_note" ("id", "post_id", "polygon", "text") OVERRIDING SYSTEM VALUE
--SELECT "id", "post_id", "polygon", "text" FROM public."post_note";
--
---- Update sequence
--SELECT setval(pg_get_serial_sequence('oxi.post_note', 'id'), (SELECT MAX("id") FROM oxi."post_note"));

-- =============================== Post Score ================================ --
INSERT INTO oxi."post_score" ("post_id", "user_id", "score", "time")
SELECT "post_id", "user_id", "score", "time" AT TIME ZONE 'UTC' FROM public."post_score";

-- ================================ Comments ================================= --
-- Comment text is non-nullable in Oxibooru, so replace NULL values with an empty string
UPDATE public."comment"
SET "text" = ''
WHERE "text" IS NULL;

-- last_edit_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."comment"
SET "last_edit_time" = CURRENT_TIMESTAMP
WHERE "last_edit_time" IS NULL;

INSERT INTO oxi."comment" ("id", "user_id", "post_id", "text", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "user_id", "post_id", "text", "creation_time" AT TIME ZONE 'UTC', "last_edit_time" AT TIME ZONE 'UTC' FROM public."comment";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.comment', 'id'), (SELECT MAX("id") FROM oxi."comment"));

-- ============================= Comment Scores ============================== --
INSERT INTO oxi."comment_score" ("comment_id", "user_id", "score", "time")
SELECT "comment_id", "user_id", "score", "time" AT TIME ZONE 'UTC' FROM public."comment_score";

-- ============================= Pool Categories ============================= --
-- First, set category_id of pools in default category to 0 (the id of the default category in Oxibooru)
ALTER TABLE public."pool" DROP CONSTRAINT "pool_category_id_fkey";
UPDATE public."pool"
SET "category_id" = 0
FROM public."pool_category"
WHERE public."pool"."category_id" = public."pool_category"."id"
AND public."pool_category"."default" = true;

-- Then, update Oxibooru default category with properties of Szurubooru default category
UPDATE oxi."pool_category"
SET "name" = public."pool_category"."name",
    "color" = public."pool_category"."color"
FROM public."pool_category"
WHERE public."pool_category"."default" = true;

-- Lastly, remove Szurubooru default category
DELETE FROM public."pool_category"
WHERE "default" = true;

INSERT INTO oxi."pool_category" ("id", "name", "color", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "name", "color", CURRENT_TIMESTAMP FROM public."pool_category";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.pool_category', 'id'), (SELECT MAX("id") FROM oxi."pool_category"));

-- ================================== Pools ================================== --
-- Descriptions are non-nullable in Oxibooru, so replace NULL values with empty description
UPDATE public."pool"
SET "description" = ''
WHERE "description" IS NULL;

-- last_edit_time is non-nullable in Oxibooru, so replace NULL values with CURRENT_TIMESTAMP
UPDATE public."pool"
SET "last_edit_time" = CURRENT_TIMESTAMP
WHERE "last_edit_time" IS NULL;

INSERT INTO oxi."pool" ("id", "category_id", "description", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "category_id", "description", "creation_time" AT TIME ZONE 'UTC', "last_edit_time" AT TIME ZONE 'UTC' FROM public."pool";

-- Update sequence
SELECT setval(pg_get_serial_sequence('oxi.pool', 'id'), (SELECT MAX("id") FROM oxi."pool"));

-- =============================== Pool Names ================================ --
INSERT INTO oxi."pool_name" ("pool_id", "order", "name")
SELECT "pool_id", "ord", "name" FROM public."pool_name";

-- =============================== Pool Posts ================================ --
INSERT INTO oxi."pool_post" ("pool_id", "post_id", "order")
SELECT "pool_id", "post_id", "ord" FROM public."pool_post";

-- ================================= Cleanup ================================= --
DROP SCHEMA public CASCADE;
ALTER SCHEMA oxi RENAME TO public;