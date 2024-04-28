CREATE TABLE "post" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "user_id" INTEGER REFERENCES "user" ON DELETE SET NULL,
    "file_size" BIGINT NOT NULL,
    "width" INTEGER NOT NULL,
    "height" INTEGER NOT NULL,
    "safety" VARCHAR(32) NOT NULL,
    "type" VARCHAR(32) NOT NULL,
    "mime_type" VARCHAR(32) NOT NULL,   -- MIME stands for Multipurpose Internet Mail Extensions
    "checksum" VARCHAR(64) NOT NULL,
    "checksum_md5" VARCHAR(32),
    "flags" VARCHAR(32),
    "source" VARCHAR(2048),
    "creation_time" TIMESTAMP WITH TIME ZONE NOT NULL,
    "last_edit_time" TIMESTAMP WITH TIME ZONE NOT NULL
);

CREATE TABLE "post_relation" (
    "parent_id" INTEGER NOT NULL REFERENCES "post" ON DELETE CASCADE,
    "child_id" INTEGER NOT NULL REFERENCES "post" ON DELETE CASCADE,
    PRIMARY KEY ("parent_id", "child_id")
);

CREATE TABLE "post_tag" (
    "post_id" INTEGER NOT NULL REFERENCES "post" ON DELETE CASCADE,
    "tag_id" INTEGER NOT NULL REFERENCES "tag" ON DELETE CASCADE,
    PRIMARY KEY ("post_id", "tag_id")
);

CREATE TABLE "post_favorite" (
    "post_id" INTEGER NOT NULL REFERENCES "post" ON DELETE CASCADE,
    "user_id" INTEGER NOT NULL REFERENCES "user" ON DELETE CASCADE,
    "time" TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY ("post_id", "user_id")
);

CREATE TABLE "post_feature" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "post_id" INTEGER NOT NULL REFERENCES "post" ON DELETE CASCADE,
    "user_id" INTEGER NOT NULL REFERENCES "user" ON DELETE CASCADE
);

CREATE TABLE "post_note" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "post_id" INTEGER NOT NULL REFERENCES "post" ON DELETE CASCADE,
    "polygon" BYTEA NOT NULL,
    "text" TEXT NOT NULL
);

CREATE TABLE "post_score" (
    "post_id" INTEGER NOT NULL REFERENCES "post" ON DELETE CASCADE,
    "user_id" INTEGER REFERENCES "user" ON DELETE SET NULL,
    "score" INTEGER NOT NULL,
    "time" TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY ("post_id", "user_id")
);

CREATE TABLE "post_signature" (
    "post_id" INTEGER PRIMARY KEY REFERENCES "post" ON DELETE CASCADE,
    "signature" BYTEA NOT NULL,
    "words" INTEGER[] NOT NULL
);