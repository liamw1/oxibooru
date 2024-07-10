use crate::model::post::Post;
use crate::schema::{post, post_tag};
use crate::search::{self, ColumnFilter};
use diesel::prelude::*;

pub fn test() {
    let query = post::table.into_boxed();
    let query = query.filter(post::file_size.gt(100));

    let filter_type = search::FilterType::Range(0..1);
    let post_filter = search::SimpleFilter {
        filter_type,
        negated: false,
        column: post::id,
    };
    let query = post_filter.apply(query);

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
