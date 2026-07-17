use crate::app::Context;
use crate::config::AnonymousPreferences;
use crate::model::enums::{PostSafety, UserRank};
use crate::schema::{post, post_statistics, post_tag, tag, tag_category, tag_name};
use crate::string::SmallString;
use diesel::dsl::{InnerJoin, IntoBoxed, Select, exists, sql};
use diesel::expression::SqlLiteral;
use diesel::pg::Pg;
use diesel::query_builder::QueryFragment;
use diesel::sql_types::{Array, BigInt, Bool, Integer, Text};
use diesel::{Expression, ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, alias};

alias!(post as inner_post: PostAlias);

pub type HiddenTagsBoxedQuery<'a> =
    IntoBoxed<'a, InnerJoin<InnerJoin<Select<tag::table, tag::id>, tag_category::table>, tag_name::table>, Pg>;

pub type HiddenPostsBoxedQuery =
    IntoBoxed<'static, InnerJoin<Select<inner_post, SqlLiteral<Integer>>, post_statistics::table>, Pg>;

pub fn get(ctx: &Context) -> Option<&AnonymousPreferences> {
    (ctx.client.rank == UserRank::Anonymous && has_preferences(ctx)).then_some(&ctx.config.anonymous_preferences)
}

pub fn has_preferences(ctx: &Context) -> bool {
    ctx.client.rank == UserRank::Anonymous && !ctx.config.anonymous_preferences.is_empty()
}

pub fn category_hidden(conn: &mut PgConnection, ctx: &Context, category_name: &str) -> QueryResult<bool> {
    if ctx.client.rank == UserRank::Anonymous {
        name_contained_in(conn, category_name, &ctx.config.anonymous_preferences.tag_category_blacklist)
    } else {
        Ok(false)
    }
}

pub fn tag_hidden(conn: &mut PgConnection, ctx: &Context, tag_name: &str, category_name: &str) -> QueryResult<bool> {
    Ok(if ctx.client.rank == UserRank::Anonymous {
        let preferences = &ctx.config.anonymous_preferences;
        name_contained_in(conn, tag_name, &preferences.tag_blacklist)?
            || name_contained_in(conn, category_name, &preferences.tag_category_blacklist)?
    } else {
        false
    })
}

pub fn hidden_categories(ctx: &Context) -> Option<&[SmallString]> {
    let preferences = &ctx.config.anonymous_preferences;
    (ctx.client.rank == UserRank::Anonymous && !preferences.is_empty()).then_some(&preferences.tag_category_blacklist)
}

pub fn hidden_tags<'a>(ctx: &'a Context) -> Option<HiddenTagsBoxedQuery<'a>> {
    let preferences = &ctx.config.anonymous_preferences;
    if ctx.client.rank != UserRank::Anonymous
        || (preferences.tag_blacklist.is_empty() && preferences.tag_category_blacklist.is_empty())
    {
        return None;
    }

    let mut query = tag::table
        .select(tag::id)
        .inner_join(tag_category::table)
        .inner_join(tag_name::table)
        .into_boxed();
    if !preferences.tag_blacklist.is_empty() {
        query = query.filter(tag_name::name.eq_any(&preferences.tag_blacklist));
    }
    if !preferences.tag_category_blacklist.is_empty() {
        query = query.or_filter(tag_category::name.eq_any(&preferences.tag_category_blacklist));
    }
    Some(query)
}

pub fn hidden_posts<C>(ctx: &Context, post_id_column: C) -> Option<HiddenPostsBoxedQuery>
where
    C: Expression<SqlType = BigInt> + QueryFragment<Pg> + Send + 'static,
{
    let preferences = &ctx.config.anonymous_preferences;
    if ctx.client.rank != UserRank::Anonymous || preferences.is_empty() {
        return None;
    }

    let mut query = inner_post
        .select(sql::<Integer>("0"))
        .inner_join(post_statistics::table)
        .into_boxed();

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

// Determines if `name` is CITEXT-equivalent to any elements in `haystack`.
// We use a query here because CITEXT semantics differ from comparing `str::to_lowercased`-ed strings in certain cases.
fn name_contained_in(conn: &mut PgConnection, name: &str, haystack: &[SmallString]) -> QueryResult<bool> {
    if haystack.is_empty() {
        return Ok(false);
    }

    diesel::select(
        sql::<Bool>("EXISTS (SELECT 1 FROM unnest(")
            .bind::<Array<Text>, _>(haystack)
            .sql("::text[]) WHERE unnest::CITEXT = ")
            .bind::<Text, _>(name)
            .sql("::CITEXT)"),
    )
    .get_result(conn)
}
