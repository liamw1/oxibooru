CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(32) NOT NULL,
    password_hash VARCHAR(64) NOT NULL,
    password_salt VARCHAR(32),
    email VARCHAR(64),
    rank VARCHAR(32) NOT NULL,
    creation_time TIMESTAMP WITH TIME ZONE NOT NULL,
    last_login_time TIMESTAMP WITH TIME ZONE NOT NULL,
    UNIQUE (name)
)