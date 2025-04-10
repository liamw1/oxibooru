DROP TABLE "user_token";

CREATE TABLE "user_token" (
    "user_id" INTEGER PRIMARY KEY REFERENCES "user" ON DELETE CASCADE,
    "token" UUID NOT NULL UNIQUE,
    "note" VARCHAR(128) NOT NULL DEFAULT '',
    "enabled" BOOLEAN NOT NULL,
    "expiration_time" TIMESTAMP WITH TIME ZONE,
    "creation_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "last_edit_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "last_usage_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);