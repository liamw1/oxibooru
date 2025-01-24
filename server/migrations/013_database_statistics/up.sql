-- Create database statistics table
CREATE TABLE "database_statistics" (
    "id" BOOLEAN PRIMARY KEY DEFAULT true,
    "disk_usage" BIGINT NOT NULL,
    "comment_count" BIGINT NOT NULL,
    "pool_count" BIGINT NOT NULL,
    "post_count" BIGINT NOT NULL,
    "tag_count" BIGINT NOT NULL,
    "user_count" BIGINT NOT NULL,
    CONSTRAINT "singular" CHECK ("id")
);

INSERT INTO "database_statistics" ("disk_usage", "comment_count", "pool_count", "post_count", "tag_count", "user_count")
VALUES (
    (SELECT COALESCE(SUM("file_size"), 0) FROM "post"),
    (SELECT COUNT(*) FROM "comment"),
    (SELECT COUNT(*) FROM "pool"),
    (SELECT COUNT(*) FROM "post"),
    (SELECT COUNT(*) FROM "tag"),
    (SELECT COUNT(*) FROM "user")
);

-- Create comment statistics table
CREATE TABLE "comment_statistics" (
    "comment_id" BIGINT PRIMARY KEY REFERENCES "comment" ON DELETE CASCADE,
    "score" BIGINT NOT NULL DEFAULT 0
);

INSERT INTO "comment_statistics" ("comment_id", "score")
SELECT "id", SUM(COALESCE("score", 0)) FROM "comment"
LEFT JOIN "comment_score" ON "comment_id" = "id"
GROUP BY "id";

-- Create pool category statistics table
CREATE TABLE "pool_category_statistics" (
    "category_id" BIGINT PRIMARY KEY REFERENCES "pool_category" ON DELETE CASCADE,
    "usage_count" BIGINT NOT NULL DEFAULT 0
);

INSERT INTO "pool_category_statistics" ("category_id", "usage_count")
SELECT "pool_category"."id", COUNT("pool"."id") FROM "pool_category"
LEFT JOIN "pool" ON "pool"."category_id" = "pool_category"."id"
GROUP BY "pool_category"."id";

-- Create pool statistics table
CREATE TABLE "pool_statistics" (
    "pool_id" BIGINT PRIMARY KEY REFERENCES "pool" ON DELETE CASCADE,
    "post_count" BIGINT NOT NULL DEFAULT 0
);

INSERT INTO "pool_statistics" ("pool_id", "post_count")
SELECT "id", COUNT("post_id") FROM "pool"
LEFT JOIN "pool_post" ON "pool_id" = "id"
GROUP BY "id";

-- Create post statistics table
CREATE TABLE "post_statistics" (
    "post_id" BIGINT PRIMARY KEY REFERENCES "post" ON DELETE CASCADE,
    "tag_count" BIGINT NOT NULL DEFAULT 0,
    "pool_count" BIGINT NOT NULL DEFAULT 0,
    "note_count" BIGINT NOT NULL DEFAULT 0,
    "comment_count" BIGINT NOT NULL DEFAULT 0,
    "relation_count" BIGINT NOT NULL DEFAULT 0,
    "score" BIGINT NOT NULL DEFAULT 0,
    "favorite_count" BIGINT NOT NULL DEFAULT 0,
    "feature_count" BIGINT NOT NULL DEFAULT 0,
    "last_comment_time" TIMESTAMP WITH TIME ZONE,
    "last_favorite_time" TIMESTAMP WITH TIME ZONE,
    "last_feature_time" TIMESTAMP WITH TIME ZONE
);

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

-- Create tag category statistics table
CREATE TABLE "tag_category_statistics" (
    "category_id" BIGINT PRIMARY KEY REFERENCES "tag_category" ON DELETE CASCADE,
    "usage_count" BIGINT NOT NULL DEFAULT 0
);

INSERT INTO "tag_category_statistics" ("category_id", "usage_count")
SELECT "tag_category"."id", COUNT("tag"."id") FROM "tag_category"
LEFT JOIN "tag" ON "tag"."category_id" = "tag_category"."id"
GROUP BY "tag_category"."id";

-- Create tag statistics table
CREATE TABLE "tag_statistics" (
    "tag_id" BIGINT PRIMARY KEY REFERENCES "tag" ON DELETE CASCADE,
    "usage_count" BIGINT NOT NULL DEFAULT 0,
    "implication_count" BIGINT NOT NULL DEFAULT 0,
    "suggestion_count" BIGINT NOT NULL DEFAULT 0
);

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

-- Create user statistics table
CREATE TABLE "user_statistics" (
    "user_id" BIGINT PRIMARY KEY REFERENCES "user" ON DELETE CASCADE,
    "comment_count" BIGINT NOT NULL DEFAULT 0,
    "favorite_count" BIGINT NOT NULL DEFAULT 0,
    "upload_count" BIGINT NOT NULL DEFAULT 0
);

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

-- Add new columns for keeping track of database file sizes
ALTER TABLE "post" ADD COLUMN "generated_thumbnail_size" BIGINT NOT NULL DEFAULT 0;
ALTER TABLE "post" ADD COLUMN "custom_thumbnail_size" BIGINT NOT NULL DEFAULT 0;
ALTER TABLE "user" ADD COLUMN "custom_avatar_size" BIGINT NOT NULL DEFAULT 0;

-- Add comment triggers
CREATE FUNCTION update_comment_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
        INSERT INTO "comment_statistics" ("comment_id") VALUES (NEW."id");
    ELSE
        count_change := -1;
    END IF;

    UPDATE "database_statistics"
    SET "comment_count" = "comment_count" + count_change;

    UPDATE "post_statistics"
    SET "comment_count" = "comment_count" + count_change,
        "last_comment_time" = (SELECT MAX("creation_time") FROM "comment" WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id"))
    WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id");

    UPDATE "user_statistics"
    SET "comment_count" = "comment_count" + count_change
    WHERE "user_id" = COALESCE(NEW."user_id", OLD."user_id");

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER comment_update_trigger AFTER INSERT OR DELETE ON "comment"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_comment_statistics();

-- Add comment_score triggers
CREATE FUNCTION update_comment_score_statistics() RETURNS TRIGGER AS $$
BEGIN
    UPDATE "comment_statistics"
    SET "score" = "score" + COALESCE(NEW."score", 0) - COALESCE(OLD."score", 0)
    WHERE "comment_id" = COALESCE(NEW."comment_id", OLD."comment_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER comment_score_update_trigger AFTER INSERT OR UPDATE OR DELETE ON "comment_score"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_comment_score_statistics();

-- Add pool_category triggers
CREATE FUNCTION update_pool_category_statistics() RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO "pool_category_statistics" ("category_id") VALUES (NEW."id");
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE FUNCTION update_default_pool_category_statistics() RETURNS TRIGGER AS $$
BEGIN
    UPDATE "pool_category_statistics"
    SET "usage_count" = "usage_count" + (SELECT "usage_count" FROM "pool_category_statistics" WHERE "category_id" = OLD."id")
    WHERE "category_id" = 0;

    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER pool_category_update_trigger AFTER INSERT ON "pool_category"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_pool_category_statistics();

CREATE TRIGGER default_pool_category_update_trigger BEFORE DELETE ON "pool_category"
FOR EACH ROW EXECUTE FUNCTION update_default_pool_category_statistics();

-- Add pool triggers
CREATE FUNCTION update_pool_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
        INSERT INTO "pool_statistics" ("pool_id") VALUES (NEW."id");
    ELSIF TG_OP = 'DELETE' THEN
        count_change := -1;
    ELSE
        IF NEW."category_id" IS DISTINCT FROM OLD."category_id" THEN
            UPDATE "pool_category_statistics"
            SET "usage_count" = "usage_count" - 1
            WHERE "category_id" = OLD."category_id";

            UPDATE "pool_category_statistics"
            SET "usage_count" = "usage_count" + 1
            WHERE "category_id" = NEW."category_id";
        END IF;
        RETURN NEW;
    END IF;

    UPDATE "database_statistics"
    SET "pool_count" = "pool_count" + count_change;

    UPDATE "pool_category_statistics"
    SET "usage_count" = "usage_count" + count_change
    WHERE "category_id" = COALESCE(NEW."category_id", OLD."category_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER pool_update_trigger AFTER INSERT OR UPDATE OR DELETE ON "pool"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_pool_statistics();

-- Add pool_post triggers
CREATE FUNCTION update_pool_post_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
    ELSE
        count_change := -1;
    END IF;

    UPDATE "pool_statistics"
    SET "post_count" = "post_count" + count_change
    WHERE "pool_id" = COALESCE(NEW."pool_id", OLD."pool_id");

    UPDATE "post_statistics"
    SET "pool_count" = "pool_count" + count_change
    WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER pool_post_update_trigger AFTER INSERT OR DELETE ON "pool_post"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_pool_post_statistics();

-- Add post triggers
CREATE FUNCTION update_post_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
        INSERT INTO "post_statistics" ("post_id") VALUES (NEW."id");
    ELSIF TG_OP = 'DELETE' THEN
        count_change := -1;
    ELSE
        count_change := 0;
    END IF;

    UPDATE "database_statistics"
    SET "post_count" = "post_count" + count_change,
        "disk_usage" = "disk_usage" + COALESCE(NEW."file_size", 0) + COALESCE(NEW."generated_thumbnail_size", 0) + COALESCE(NEW."custom_thumbnail_size", 0)
                                    - COALESCE(OLD."file_size", 0) - COALESCE(OLD."generated_thumbnail_size", 0) - COALESCE(OLD."custom_thumbnail_size", 0);
                                    
    UPDATE "user_statistics"
    SET "upload_count" = "upload_count" + count_change
    WHERE "user_id" = COALESCE(NEW."user_id", OLD."user_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER post_update_trigger AFTER INSERT OR UPDATE OR DELETE ON "post"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_post_statistics();

-- Add post_relation triggers
CREATE FUNCTION update_post_relation_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
    ELSE
        count_change := -1;
    END IF;

    UPDATE "post_statistics"
    SET "relation_count" = "relation_count" + count_change
    WHERE "post_id" = COALESCE(NEW."parent_id", OLD."parent_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER post_relation_update_trigger AFTER INSERT OR DELETE ON "post_relation"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_post_relation_statistics();

-- Add post_tag triggers
CREATE FUNCTION update_post_tag_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
    ELSE
        count_change := -1;
    END IF;

    UPDATE "post_statistics"
    SET "tag_count" = "tag_count" + count_change
    WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id");

    UPDATE "tag_statistics"
    SET "usage_count" = "usage_count" + count_change
    WHERE "tag_id" = COALESCE(NEW."tag_id", OLD."tag_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER post_tag_update_trigger AFTER INSERT OR DELETE ON "post_tag"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_post_tag_statistics();

-- Add post_favorite triggers
CREATE FUNCTION update_post_favorite_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
    ELSE
        count_change := -1;
    END IF;

    UPDATE "post_statistics"
    SET "favorite_count" = "favorite_count" + count_change,
        "last_favorite_time" = (SELECT MAX("time") FROM "post_favorite" WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id"))
    WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id");

    UPDATE "user_statistics"
    SET "favorite_count" = "favorite_count" + count_change
    WHERE "user_id" = COALESCE(NEW."user_id", OLD."user_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER post_favorite_update_trigger AFTER INSERT OR DELETE ON "post_favorite"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_post_favorite_statistics();

-- Add post_feature triggers
CREATE FUNCTION update_post_feature_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
    ELSE
        count_change := -1;
    END IF;

    UPDATE "post_statistics"
    SET "feature_count" = "feature_count" + count_change,
        "last_feature_time" = (SELECT MAX("time") FROM "post_feature" WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id"))
    WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER post_feature_update_trigger AFTER INSERT OR DELETE ON "post_feature"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_post_feature_statistics();

-- Add post_note triggers
CREATE FUNCTION update_post_note_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
    ELSE
        count_change := -1;
    END IF;

    UPDATE "post_statistics"
    SET "note_count" = "note_count" + count_change
    WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER post_note_update_trigger AFTER INSERT OR DELETE ON "post_note"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_post_note_statistics();

-- Add post_score triggers
CREATE FUNCTION update_post_score_statistics() RETURNS TRIGGER AS $$
BEGIN
    UPDATE "post_statistics"
    SET "score" = "score" + COALESCE(NEW."score", 0) - COALESCE(OLD."score", 0)
    WHERE "post_id" = COALESCE(NEW."post_id", OLD."post_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER post_score_update_trigger AFTER INSERT OR UPDATE OR DELETE ON "post_score"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_post_score_statistics();

-- Add tag_category triggers
CREATE FUNCTION update_tag_category_statistics() RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO "tag_category_statistics" ("category_id") VALUES (NEW."id");
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE FUNCTION update_default_tag_category_statistics() RETURNS TRIGGER AS $$
BEGIN
    UPDATE "tag_category_statistics"
    SET "usage_count" = "usage_count" + (SELECT "usage_count" FROM "tag_category_statistics" WHERE "category_id" = OLD."id")
    WHERE "category_id" = 0;

    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER tag_category_update_trigger AFTER INSERT ON "tag_category"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_tag_category_statistics();

CREATE TRIGGER default_category_update_trigger BEFORE DELETE ON "tag_category"
FOR EACH ROW EXECUTE FUNCTION update_default_tag_category_statistics();

-- Add tag triggers
CREATE FUNCTION update_tag_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
        INSERT INTO "tag_statistics" ("tag_id") VALUES (NEW."id");
    ELSIF TG_OP = 'DELETE' THEN
        count_change := -1;
    ELSE
        IF NEW."category_id" IS DISTINCT FROM OLD."category_id" THEN
            UPDATE "tag_category_statistics"
            SET "usage_count" = "usage_count" - 1
            WHERE "category_id" = OLD."category_id";

            UPDATE "tag_category_statistics"
            SET "usage_count" = "usage_count" + 1
            WHERE "category_id" = NEW."category_id";
        END IF;
        RETURN NEW;
    END IF;

    UPDATE "database_statistics"
    SET "tag_count" = "tag_count" + count_change;

    UPDATE "tag_category_statistics"
    SET "usage_count" = "usage_count" + count_change
    WHERE "category_id" = COALESCE(NEW."category_id", OLD."category_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER tag_update_trigger AFTER INSERT OR UPDATE OR DELETE ON "tag"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_tag_statistics();

-- Add tag_implication triggers
CREATE FUNCTION update_tag_implication_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
    ELSE
        count_change := -1;
    END IF;

    UPDATE "tag_statistics"
    SET "implication_count" = "implication_count" + count_change
    WHERE "tag_id" = COALESCE(NEW."parent_id", OLD."parent_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER tag_implication_update_trigger AFTER INSERT OR DELETE ON "tag_implication"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_tag_implication_statistics();

-- Add tag_suggestion triggers
CREATE FUNCTION update_tag_suggestion_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
    ELSE
        count_change := -1;
    END IF;

    UPDATE "tag_statistics"
    SET "suggestion_count" = "suggestion_count" + count_change
    WHERE "tag_id" = COALESCE(NEW."parent_id", OLD."parent_id");

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER tag_suggestion_update_trigger AFTER INSERT OR DELETE ON "tag_suggestion"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_tag_suggestion_statistics();

-- Add user triggers
CREATE FUNCTION update_user_statistics() RETURNS TRIGGER AS $$
DECLARE
    count_change BIGINT;
BEGIN
    IF TG_OP = 'INSERT' THEN
        count_change := 1;
        INSERT INTO "user_statistics" ("user_id") VALUES (NEW."id");
    ELSIF TG_OP = 'DELETE' THEN
        count_change := -1;
    ELSE
        count_change := 0;
    END IF;

    UPDATE "database_statistics"
    SET "user_count" = "user_count" + count_change,
        "disk_usage" = "disk_usage" + COALESCE(NEW."custom_avatar_size", 0) - COALESCE(OLD."custom_avatar_size", 0);

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER user_update_trigger AFTER INSERT OR UPDATE OR DELETE ON "user"
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION update_user_statistics();