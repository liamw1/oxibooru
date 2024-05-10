CREATE TABLE "user" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "name" VARCHAR(32) NOT NULL,
    "rank" SMALLINT NOT NULL,
    "email" VARCHAR(64),
    "password_hash" VARCHAR(128) NOT NULL,
    "password_salt" VARCHAR(32) NOT NULL,
    "creation_time" TIMESTAMP WITH TIME ZONE NOT NULL,
    "last_login_time" TIMESTAMP WITH TIME ZONE NOT NULL,
    UNIQUE ("name")
);

CREATE TABLE "user_token" (
    "user_id" INTEGER PRIMARY KEY REFERENCES "user" ON DELETE CASCADE,
    "token" VARCHAR(36) NOT NULL,
    "note" VARCHAR(128),
    "enabled" BOOLEAN NOT NULL,
    "expiration_time" TIMESTAMP WITH TIME ZONE,
    "creation_time" TIMESTAMP WITH TIME ZONE NOT NULL,
    "last_edit_time" TIMESTAMP WITH TIME ZONE NOT NULL,
    "last_usage_time" TIMESTAMP WITH TIME ZONE NOT NULL
);