CREATE TABLE "user" (
    "id" INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    "name" VARCHAR(32) NOT NULL,
    "rank" SMALLINT NOT NULL,
    "email" VARCHAR(64),
    "avatar_style" VARCHAR(32) NOT NULL DEFAULT 'gravatar',
    "password_hash" VARCHAR(128) NOT NULL,
    "password_salt" VARCHAR(32) NOT NULL,
    "creation_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "last_login_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "last_edit_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE ("name")
);
SELECT diesel_manage_last_edit_time('user');

CREATE TABLE "user_token" (
    "user_id" INTEGER PRIMARY KEY REFERENCES "user" ON DELETE CASCADE,
    "token" UUID NOT NULL,
    "note" VARCHAR(128),
    "enabled" BOOLEAN NOT NULL,
    "expiration_time" TIMESTAMP WITH TIME ZONE,
    "creation_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "last_edit_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "last_usage_time" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE("token")
);
SELECT diesel_manage_last_edit_time('user_token');