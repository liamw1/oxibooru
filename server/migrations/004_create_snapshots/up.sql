CREATE TABLE "snapshot" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "user_id" INTEGER REFERENCES "user" ON DELETE SET NULL,
    "resource_id" INTEGER NOT NULL, --pkey
    "resource_type" VARCHAR(32) NOT NULL,
    "resource_name" VARCHAR(128) NOT NULL,
    "operation" VARCHAR(16) NOT NULL,
    "data" BYTEA,
    "creation_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);