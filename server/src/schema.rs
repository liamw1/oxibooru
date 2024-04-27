// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Int4,
        #[max_length = 32]
        name -> Varchar,
        #[max_length = 64]
        password_hash -> Varchar,
        #[max_length = 32]
        password_salt -> Nullable<Varchar>,
        #[max_length = 64]
        email -> Nullable<Varchar>,
        #[max_length = 32]
        rank -> Varchar,
        creation_time -> Timestamptz,
        last_login_time -> Timestamptz,
    }
}
