CREATE TABLE "tag_category" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "order" INTEGER NOT NULL,
    "name" VARCHAR(32) NOT NULL,
    "color" VARCHAR(32) NOT NULL,
    "last_edit_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE ("name")
);
SELECT diesel_manage_last_edit_time('tag_category');
INSERT INTO "tag_category" ("id", "order", "name", "color") OVERRIDING SYSTEM VALUE VALUES (0, 0, 'default', 'default');

CREATE TABLE "tag" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "category_id" INTEGER NOT NULL DEFAULT 0 REFERENCES "tag_category" ON DELETE SET DEFAULT,
    "description" TEXT NOT NULL DEFAULT '',
    "creation_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "last_edit_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
SELECT diesel_manage_last_edit_time('tag');

CREATE TABLE "tag_name" (
    "tag_id" INTEGER NOT NULL REFERENCES "tag" ON DELETE CASCADE,
    "order" INTEGER NOT NULL,
    "name" VARCHAR(128) NOT NULL,
    PRIMARY KEY ("tag_id", "order"),
    UNIQUE ("name")
);

CREATE TABLE "tag_implication" (
    "parent_id" INTEGER NOT NULL REFERENCES "tag" ON DELETE CASCADE,
    "child_id" INTEGER NOT NULL REFERENCES "tag" ON DELETE CASCADE,
    PRIMARY KEY ("parent_id", "child_id")
);

CREATE TABLE "tag_suggestion" (
    "parent_id" INTEGER NOT NULL REFERENCES "tag" ON DELETE CASCADE,
    "child_id" INTEGER NOT NULL REFERENCES "tag" ON DELETE CASCADE,
    PRIMARY KEY ("parent_id", "child_id")
);