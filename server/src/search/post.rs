use crate::model::post::Post;
use crate::schema::{post, post_tag};
use crate::search::filter::*;
use crate::search::UnparsedFilter;
use diesel::prelude::*;

pub fn test() {
    let query = post::table.into_boxed();
    let query = query.filter(post::file_size.gt(100));

    let filter = UnparsedFilter {
        kind: 0,
        criteria: "0..1",
        negated: false,
    };
    let query = apply_i32_filter(query, post::id, filter).unwrap();

    let query = query.inner_join(post_tag::table);
    let query = query.filter(post_tag::tag_id.gt(1));

    let mut conn = crate::establish_connection().unwrap();
    let _res = query.select(Post::as_select()).load(&mut conn).unwrap();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_test() {
        test();
    }
}

// type Boxed<'a> = IntoBoxed<'a, InnerJoin<post::table, post_tag::table>, Pg>;
