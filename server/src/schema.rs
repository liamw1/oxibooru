// @generated automatically by Diesel CLI.

diesel::table! {
    comment (id) {
        id -> Int8,
        user_id -> Nullable<Int8>,
        post_id -> Int8,
        text -> Text,
        creation_time -> Timestamptz,
        last_edit_time -> Timestamptz,
    }
}

diesel::table! {
    comment_score (comment_id, user_id) {
        comment_id -> Int8,
        user_id -> Int8,
        score -> Int2,
        time -> Timestamptz,
    }
}

diesel::table! {
    comment_statistics (comment_id) {
        comment_id -> Int8,
        score -> Int8,
    }
}

diesel::table! {
    database_statistics (id) {
        id -> Bool,
        disk_usage -> Int8,
        comment_count -> Int8,
        pool_count -> Int8,
        post_count -> Int8,
        tag_count -> Int8,
        user_count -> Int8,
        signature_version -> Int4,
    }
}

diesel::table! {
    pool (id) {
        id -> Int8,
        category_id -> Int8,
        description -> Text,
        creation_time -> Timestamptz,
        last_edit_time -> Timestamptz,
    }
}

diesel::table! {
    pool_category (id) {
        id -> Int8,
        name -> Citext,
        #[max_length = 32]
        color -> Varchar,
        last_edit_time -> Timestamptz,
    }
}

diesel::table! {
    pool_category_statistics (category_id) {
        category_id -> Int8,
        usage_count -> Int8,
    }
}

diesel::table! {
    pool_name (pool_id, order) {
        pool_id -> Int8,
        order -> Int4,
        name -> Citext,
    }
}

diesel::table! {
    pool_post (pool_id, post_id) {
        pool_id -> Int8,
        post_id -> Int8,
        order -> Int8,
    }
}

diesel::table! {
    pool_statistics (pool_id) {
        pool_id -> Int8,
        post_count -> Int8,
    }
}

diesel::table! {
    post (id) {
        id -> Int8,
        user_id -> Nullable<Int8>,
        file_size -> Int8,
        width -> Int4,
        height -> Int4,
        safety -> Int2,
        #[sql_name = "type"]
        type_ -> Int2,
        mime_type -> Int2,
        checksum -> Bytea,
        checksum_md5 -> Bytea,
        flags -> Int2,
        source -> Text,
        creation_time -> Timestamptz,
        last_edit_time -> Timestamptz,
        generated_thumbnail_size -> Int8,
        custom_thumbnail_size -> Int8,
    }
}

diesel::table! {
    post_favorite (post_id, user_id) {
        post_id -> Int8,
        user_id -> Int8,
        time -> Timestamptz,
    }
}

diesel::table! {
    post_feature (id) {
        id -> Int8,
        post_id -> Int8,
        user_id -> Int8,
        time -> Timestamptz,
    }
}

diesel::table! {
    post_note (id) {
        id -> Int8,
        post_id -> Int8,
        polygon -> Array<Nullable<Float4>>,
        text -> Text,
    }
}

diesel::table! {
    post_relation (parent_id, child_id) {
        parent_id -> Int8,
        child_id -> Int8,
    }
}

diesel::table! {
    post_score (post_id, user_id) {
        post_id -> Int8,
        user_id -> Int8,
        score -> Int2,
        time -> Timestamptz,
    }
}

diesel::table! {
    post_signature (post_id) {
        post_id -> Int8,
        words -> Array<Nullable<Int4>>,
        signature -> Array<Nullable<Int8>>,
    }
}

diesel::table! {
    post_statistics (post_id) {
        post_id -> Int8,
        tag_count -> Int8,
        pool_count -> Int8,
        note_count -> Int8,
        comment_count -> Int8,
        relation_count -> Int8,
        score -> Int8,
        favorite_count -> Int8,
        feature_count -> Int8,
        last_comment_time -> Nullable<Timestamptz>,
        last_favorite_time -> Nullable<Timestamptz>,
        last_feature_time -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    post_tag (post_id, tag_id) {
        post_id -> Int8,
        tag_id -> Int8,
    }
}

diesel::table! {
    tag (id) {
        id -> Int8,
        category_id -> Int8,
        description -> Text,
        creation_time -> Timestamptz,
        last_edit_time -> Timestamptz,
    }
}

diesel::table! {
    tag_category (id) {
        id -> Int8,
        order -> Int4,
        name -> Citext,
        #[max_length = 32]
        color -> Varchar,
        last_edit_time -> Timestamptz,
    }
}

diesel::table! {
    tag_category_statistics (category_id) {
        category_id -> Int8,
        usage_count -> Int8,
    }
}

diesel::table! {
    tag_implication (parent_id, child_id) {
        parent_id -> Int8,
        child_id -> Int8,
    }
}

diesel::table! {
    tag_name (tag_id, order) {
        tag_id -> Int8,
        order -> Int4,
        name -> Citext,
    }
}

diesel::table! {
    tag_statistics (tag_id) {
        tag_id -> Int8,
        usage_count -> Int8,
        implication_count -> Int8,
        suggestion_count -> Int8,
    }
}

diesel::table! {
    tag_suggestion (parent_id, child_id) {
        parent_id -> Int8,
        child_id -> Int8,
    }
}

diesel::table! {
    user (id) {
        id -> Int8,
        name -> Citext,
        #[max_length = 128]
        password_hash -> Varchar,
        #[max_length = 32]
        password_salt -> Varchar,
        email -> Nullable<Citext>,
        rank -> Int2,
        avatar_style -> Int2,
        creation_time -> Timestamptz,
        last_login_time -> Timestamptz,
        last_edit_time -> Timestamptz,
        custom_avatar_size -> Int8,
    }
}

diesel::table! {
    user_statistics (user_id) {
        user_id -> Int8,
        comment_count -> Int8,
        favorite_count -> Int8,
        upload_count -> Int8,
    }
}

diesel::table! {
    user_token (id) {
        id -> Uuid,
        user_id -> Int8,
        #[max_length = 128]
        note -> Varchar,
        enabled -> Bool,
        expiration_time -> Nullable<Timestamptz>,
        creation_time -> Timestamptz,
        last_edit_time -> Timestamptz,
        last_usage_time -> Timestamptz,
    }
}

diesel::joinable!(comment -> post (post_id));
diesel::joinable!(comment -> user (user_id));
diesel::joinable!(comment_score -> comment (comment_id));
diesel::joinable!(comment_score -> user (user_id));
diesel::joinable!(comment_statistics -> comment (comment_id));
diesel::joinable!(pool -> pool_category (category_id));
diesel::joinable!(pool_category_statistics -> pool_category (category_id));
diesel::joinable!(pool_name -> pool (pool_id));
diesel::joinable!(pool_post -> pool (pool_id));
diesel::joinable!(pool_post -> post (post_id));
diesel::joinable!(pool_statistics -> pool (pool_id));
diesel::joinable!(post -> user (user_id));
diesel::joinable!(post_favorite -> post (post_id));
diesel::joinable!(post_favorite -> user (user_id));
diesel::joinable!(post_feature -> post (post_id));
diesel::joinable!(post_feature -> user (user_id));
diesel::joinable!(post_note -> post (post_id));
diesel::joinable!(post_score -> post (post_id));
diesel::joinable!(post_score -> user (user_id));
diesel::joinable!(post_signature -> post (post_id));
diesel::joinable!(post_statistics -> post (post_id));
diesel::joinable!(post_tag -> post (post_id));
diesel::joinable!(post_tag -> tag (tag_id));
diesel::joinable!(tag -> tag_category (category_id));
diesel::joinable!(tag_category_statistics -> tag_category (category_id));
diesel::joinable!(tag_name -> tag (tag_id));
diesel::joinable!(tag_statistics -> tag (tag_id));
diesel::joinable!(user_statistics -> user (user_id));
diesel::joinable!(user_token -> user (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    comment,
    comment_score,
    comment_statistics,
    database_statistics,
    pool,
    pool_category,
    pool_category_statistics,
    pool_name,
    pool_post,
    pool_statistics,
    post,
    post_favorite,
    post_feature,
    post_note,
    post_relation,
    post_score,
    post_signature,
    post_statistics,
    post_tag,
    tag,
    tag_category,
    tag_category_statistics,
    tag_implication,
    tag_name,
    tag_statistics,
    tag_suggestion,
    user,
    user_statistics,
    user_token,
);
