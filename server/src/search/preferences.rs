use crate::auth::Client;
use crate::config::Config;
use crate::model::enums::{PostSafety, UserRank};
use crate::schema::{post, post_statistics, post_tag, tag, tag_category, tag_name};
use diesel::dsl::{InnerJoin, IntoBoxed, Select, exists, sql};
use diesel::expression::SqlLiteral;
use diesel::pg::Pg;
use diesel::query_builder::QueryFragment;
use diesel::sql_types::{BigInt, Bool, Integer};
use diesel::{Expression, ExpressionMethods, QueryDsl, alias};

alias!(post as inner_post: PostAlias);

pub type BoxedQuery =
    IntoBoxed<'static, InnerJoin<Select<inner_post, SqlLiteral<Integer>>, post_statistics::table>, Pg>;

pub fn has_preferences(config: &Config, client: Client) -> bool {
    client.rank == UserRank::Anonymous && !config.anonymous_preferences.is_empty()
}

pub fn hidden_posts<C>(config: &Config, client: Client, post_id_column: C) -> Option<BoxedQuery>
where
    C: Expression<SqlType = BigInt> + QueryFragment<Pg> + Send + 'static,
{
    if client.rank != UserRank::Anonymous {
        return None;
    }
    let preferences = &config.anonymous_preferences;

    let mut query = inner_post
        .select(sql::<Integer>("0"))
        .inner_join(post_statistics::table)
        .into_boxed();

    // If no preferences are specified, no posts are hidden
    if preferences.is_empty() {
        return Some(query.filter(sql::<Bool>("0 = 1")));
    }

    if preferences.hide_sketchy {
        query = query.or_filter(inner_post.field(post::safety).eq(PostSafety::Sketchy));
    }
    if preferences.hide_unsafe {
        query = query.or_filter(inner_post.field(post::safety).eq(PostSafety::Unsafe));
    }
    if preferences.hide_untagged {
        query = query.or_filter(post_statistics::tag_count.eq(0));
    }
    if !preferences.tag_blacklist.is_empty() || !preferences.tag_category_blacklist.is_empty() {
        let blacklisted_posts = tag::table
            .select(sql::<Integer>("0"))
            .inner_join(tag_category::table)
            .inner_join(tag_name::table)
            .inner_join(post_tag::table)
            .filter(tag_name::name.eq_any(preferences.tag_blacklist.clone()))
            .or_filter(tag_category::name.eq_any(preferences.tag_category_blacklist.clone()))
            .filter(sql::<Bool>("").bind(inner_post.field(post::id).eq(post_tag::post_id)));
        query = query.or_filter(exists(blacklisted_posts));
    }
    Some(query.filter(sql::<Bool>("").bind(inner_post.field(post::id).eq(post_id_column))))
}
