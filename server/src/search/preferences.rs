use crate::model::enums::PostSafety;
use crate::schema::{post, post_statistics, post_tag, tag, tag_category, tag_name};
use crate::string::SmallString;
use diesel::dsl::{InnerJoin, IntoBoxed, Select, exists, sql};
use diesel::expression::SqlLiteral;
use diesel::pg::Pg;
use diesel::query_builder::QueryFragment;
use diesel::sql_types::{Array, BigInt, Bool, Integer, Text};
use diesel::{Expression, ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl, alias};
use serde::Deserialize;
use std::collections::HashSet;

alias!(post as inner_post: PostAlias);

pub type HiddenTagsBoxedQuery<'a> =
    IntoBoxed<'a, InnerJoin<InnerJoin<Select<tag::table, tag::id>, tag_category::table>, tag_name::table>, Pg>;

pub type HiddenPostsBoxedQuery =
    IntoBoxed<'static, InnerJoin<Select<inner_post, SqlLiteral<Integer>>, post_statistics::table>, Pg>;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub tag_blacklist: Vec<SmallString>,
    pub tag_category_blacklist: Vec<SmallString>,
    pub hide_unsafe: bool,
    pub hide_sketchy: bool,
    pub hide_untagged: bool,
}

impl Preferences {
    pub fn is_empty(&self) -> bool {
        self.tag_blacklist.is_empty()
            && self.tag_category_blacklist.is_empty()
            && !self.hide_unsafe
            && !self.hide_sketchy
            && !self.hide_untagged
    }

    pub fn merge(&mut self, rhs: &Self) {
        let combined_blacklist: HashSet<_> = self.tag_blacklist.drain(..).chain(rhs.tag_blacklist.clone()).collect();
        let combined_category_blacklist: HashSet<_> = self
            .tag_category_blacklist
            .drain(..)
            .chain(rhs.tag_category_blacklist.clone())
            .collect();

        self.tag_blacklist.extend(combined_blacklist);
        self.tag_category_blacklist.extend(combined_category_blacklist);
        self.hide_unsafe |= rhs.hide_unsafe;
        self.hide_sketchy |= rhs.hide_sketchy;
        self.hide_untagged |= rhs.hide_untagged;
    }

    pub fn category_hidden(&self, conn: &mut PgConnection, category_name: &str) -> QueryResult<bool> {
        name_contained_in(conn, category_name, &self.tag_category_blacklist)
    }

    pub fn tag_hidden(&self, conn: &mut PgConnection, tag_name: &str, category_name: &str) -> QueryResult<bool> {
        Ok(name_contained_in(conn, tag_name, &self.tag_blacklist)?
            || name_contained_in(conn, category_name, &self.tag_category_blacklist)?)
    }

    pub fn hidden_categories(&self) -> Option<&[SmallString]> {
        (!self.is_empty()).then_some(&self.tag_category_blacklist)
    }

    pub fn hidden_tags(&self) -> Option<HiddenTagsBoxedQuery<'_>> {
        if self.tag_blacklist.is_empty() && self.tag_category_blacklist.is_empty() {
            return None;
        }

        let mut query = tag::table
            .select(tag::id)
            .inner_join(tag_category::table)
            .inner_join(tag_name::table)
            .into_boxed();
        if !self.tag_blacklist.is_empty() {
            query = query.filter(tag_name::name.eq_any(&self.tag_blacklist));
        }
        if !self.tag_category_blacklist.is_empty() {
            query = query.or_filter(tag_category::name.eq_any(&self.tag_category_blacklist));
        }
        Some(query)
    }

    pub fn hidden_posts<C>(&self, post_id_column: C) -> Option<HiddenPostsBoxedQuery>
    where
        C: Expression<SqlType = BigInt> + QueryFragment<Pg> + Send + 'static,
    {
        if self.is_empty() {
            return None;
        }

        let mut query = inner_post
            .select(sql::<Integer>("0"))
            .inner_join(post_statistics::table)
            .into_boxed();

        if self.hide_sketchy {
            query = query.or_filter(inner_post.field(post::safety).eq(PostSafety::Sketchy));
        }
        if self.hide_unsafe {
            query = query.or_filter(inner_post.field(post::safety).eq(PostSafety::Unsafe));
        }
        if self.hide_untagged {
            query = query.or_filter(post_statistics::tag_count.eq(0));
        }
        if !self.tag_blacklist.is_empty() || !self.tag_category_blacklist.is_empty() {
            let blacklisted_posts = tag::table
                .select(sql::<Integer>("0"))
                .inner_join(tag_category::table)
                .inner_join(tag_name::table)
                .inner_join(post_tag::table)
                .filter(tag_name::name.eq_any(self.tag_blacklist.clone()))
                .or_filter(tag_category::name.eq_any(self.tag_category_blacklist.clone()))
                .filter(sql::<Bool>("").bind(inner_post.field(post::id).eq(post_tag::post_id)));
            query = query.or_filter(exists(blacklisted_posts));
        }
        Some(query.filter(sql::<Bool>("").bind(inner_post.field(post::id).eq(post_id_column))))
    }
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
