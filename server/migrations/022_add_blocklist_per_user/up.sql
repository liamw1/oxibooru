CREATE TABLE "user_tag_blocklist" (
    "user_id" BIGINT NOT NULL REFERENCES "user" ON DELETE CASCADE,
    "tag_id" BIGINT NOT NULL REFERENCES "tag" ON DELETE CASCADE,
    PRIMARY KEY ("user_id", "tag_id")
);
