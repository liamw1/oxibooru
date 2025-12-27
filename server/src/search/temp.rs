use crate::schema::{
    pool, pool_category, pool_statistics, post, post_favorite, post_score, post_statistics, tag, tag_category,
    tag_statistics, user,
};

diesel::table! {
    matching (id) {
        id -> Int8
    }
}

diesel::table! {
    nonmatching (id) {
        id -> Int8
    }
}

diesel::allow_tables_to_appear_in_same_query!(matching, pool);
diesel::allow_tables_to_appear_in_same_query!(matching, pool_category);
diesel::allow_tables_to_appear_in_same_query!(matching, pool_statistics);
diesel::allow_tables_to_appear_in_same_query!(matching, post);
diesel::allow_tables_to_appear_in_same_query!(matching, post_favorite);
diesel::allow_tables_to_appear_in_same_query!(matching, post_score);
diesel::allow_tables_to_appear_in_same_query!(matching, post_statistics);
diesel::allow_tables_to_appear_in_same_query!(matching, tag);
diesel::allow_tables_to_appear_in_same_query!(matching, tag_category);
diesel::allow_tables_to_appear_in_same_query!(matching, tag_statistics);
diesel::allow_tables_to_appear_in_same_query!(matching, user);
diesel::allow_tables_to_appear_in_same_query!(nonmatching, pool);
diesel::allow_tables_to_appear_in_same_query!(nonmatching, pool_category);
diesel::allow_tables_to_appear_in_same_query!(nonmatching, pool_statistics);
diesel::allow_tables_to_appear_in_same_query!(nonmatching, post);
diesel::allow_tables_to_appear_in_same_query!(nonmatching, post_statistics);
diesel::allow_tables_to_appear_in_same_query!(nonmatching, tag);
diesel::allow_tables_to_appear_in_same_query!(nonmatching, tag_category);
diesel::allow_tables_to_appear_in_same_query!(nonmatching, tag_statistics);
diesel::allow_tables_to_appear_in_same_query!(nonmatching, user);
