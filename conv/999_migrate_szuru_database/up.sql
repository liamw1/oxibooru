INSERT INTO "user" ("id", "name", "password_hash", "password_salt", "rank", "avatar_style", "creation_time", "last_login_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "name", '', '', 2, 0, "creation_time" AT TIME ZONE 'UTC', "last_login_time" AT TIME ZONE 'UTC', CURRENT_TIMESTAMP FROM szuru_user;


UPDATE "szuru_tag_category"
SET "name" = 'old_default'
WHERE "name" = 'default';

INSERT INTO "tag_category" ("id", "order", "name", "color", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "order", "name", "color", CURRENT_TIMESTAMP FROM szuru_tag_category;


INSERT INTO "tag" ("id", "category_id", "description", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "category_id", "description", "creation_time" AT TIME ZONE 'UTC', CURRENT_TIMESTAMP FROM szuru_tag
WHERE "szuru_tag"."description" IS NOT NULL;

INSERT INTO "tag" ("id", "category_id", "description", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "category_id", '', "creation_time" AT TIME ZONE 'UTC', CURRENT_TIMESTAMP FROM szuru_tag
WHERE "szuru_tag"."description" IS NULL;


INSERT INTO "tag_name" ("tag_id", "order", "name")
SELECT "tag_id", "ord", "name" FROM szuru_tag_name;


INSERT INTO "tag_implication" ("parent_id", "child_id")
SELECT "parent_id", "child_id" FROM szuru_tag_implication;

INSERT INTO "tag_suggestion" ("parent_id", "child_id")
SELECT "parent_id", "child_id" FROM szuru_tag_suggestion;


UPDATE "szuru_post"
SET "safety" = CASE "safety"
    WHEN 'safe' THEN 0
    WHEN 'sketchy' THEN 1
    WHEN 'questionable' THEN 1
    WHEN 'unsafe' THEN 2
END;
ALTER TABLE "szuru_post"
ALTER COLUMN "safety" TYPE SMALLINT USING "safety"::SMALLINT;

UPDATE "szuru_post"
SET "type" = CASE
    WHEN "type" = 'image' THEN 0
    WHEN "type" = 'animation' THEN 1
    WHEN "type" = 'video' THEN 2
    WHEN "type" = 'flash' THEN 3
    WHEN "type" = 'youtube' THEN 4
END;
ALTER TABLE "szuru_post"
ALTER COLUMN "type" TYPE SMALLINT USING "type"::SMALLINT;

UPDATE "szuru_post"
SET "mime-type" = CASE
    WHEN "mime-type" = 'image/bmp' THEN 0
    WHEN "mime-type" = 'image/gif' THEN 1
    WHEN "mime-type" = 'image/jpeg' THEN 2
    WHEN "mime-type" = 'image/png' THEN 3
    WHEN "mime-type" = 'image/webp' THEN 4
    WHEN "mime-type" = 'video/mp4' THEN 5
    WHEN "mime-type" = 'video/mov' THEN 6
    WHEN "mime-type" = 'video/webm' THEN 7
END;
ALTER TABLE "szuru_post"
ALTER COLUMN "mime-type" TYPE SMALLINT USING "mime-type"::SMALLINT;

INSERT INTO "post" ("id", "user_id", "file_size", "width", "height", "safety", "type", "mime_type", "checksum", "creation_time", "last_edit_time") OVERRIDING SYSTEM VALUE
SELECT "id", "user_id", "file_size", "image_width", "image_height", "safety", "type", "mime-type", "checksum", "creation_time" AT TIME ZONE 'UTC', CURRENT_TIMESTAMP FROM szuru_post;


INSERT INTO "post_relation" ("parent_id", "child_id")
SELECT "parent_id", "child_id" FROM szuru_post_relation;


INSERT INTO "post_tag" ("post_id", "tag_id")
SELECT "post_id", "tag_id" FROM szuru_post_tag;