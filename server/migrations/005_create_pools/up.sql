CREATE TABLE "pool_category" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "name" CITEXT NOT NULL UNIQUE,
    "color" VARCHAR(32) NOT NULL,
    "last_edit_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
SELECT diesel_manage_last_edit_time('pool_category');
INSERT INTO "pool_category" ("id", "name", "color") OVERRIDING SYSTEM VALUE VALUES (0, 'default', 'default');

CREATE TABLE "pool" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "category_id" INTEGER NOT NULL DEFAULT 0 REFERENCES "pool_category" ON DELETE SET DEFAULT,
    "description" TEXT NOT NULL DEFAULT '',
    "creation_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "last_edit_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
SELECT diesel_manage_last_edit_time('pool');

CREATE TABLE "pool_name" (
    "pool_id" INTEGER NOT NULL REFERENCES "pool" ON DELETE CASCADE,
    "order" INTEGER NOT NULL,
    "name" CITEXT NOT NULL UNIQUE,
    PRIMARY KEY ("pool_id", "order")
);

CREATE TABLE "pool_post" (
    "pool_id" INTEGER NOT NULL REFERENCES "pool" ON DELETE CASCADE,
    "post_id" INTEGER NOT NULL REFERENCES "post" ON DELETE CASCADE,
    "order" INTEGER NOT NULL,
    PRIMARY KEY ("pool_id", "post_id")
);