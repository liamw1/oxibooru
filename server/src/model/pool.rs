use crate::model::post::Post;
use crate::model::TableName;
use crate::schema::{pool, pool_category, pool_name, pool_post};
use crate::util;
use crate::util::DateTime;
use diesel::pg::Pg;
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = pool_category)]
pub struct NewPoolCategory<'a> {
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Debug, PartialEq, Eq, Identifiable, Queryable, Selectable)]
#[diesel(table_name = pool_category)]
#[diesel(check_for_backend(Pg))]
pub struct PoolCategory {
    pub id: i32,
    pub name: String,
    pub color: String,
    pub last_edit_time: DateTime,
}

impl TableName for PoolCategory {
    fn table_name() -> &'static str {
        "pool_category"
    }
}

impl PoolCategory {
    pub fn new(conn: &mut PgConnection, name: &str, color: &str) -> QueryResult<Self> {
        let new_pool_category = NewPoolCategory { name, color };
        diesel::insert_into(pool_category::table)
            .values(&new_pool_category)
            .returning(Self::as_returning())
            .get_result(conn)
    }

    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        pool_category::table.count().first(conn)
    }

    pub fn update_name(mut self, conn: &mut PgConnection, name: String) -> QueryResult<Self> {
        self.name = name;
        util::update_single_row(conn, &self, pool_category::name.eq(&self.name))?;
        Ok(self)
    }
}

#[derive(Insertable)]
#[diesel(table_name = pool)]
pub struct NewPool {
    pub category_id: i32,
}

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(PoolCategory, foreign_key = category_id))]
#[diesel(table_name = pool)]
#[diesel(check_for_backend(Pg))]
pub struct Pool {
    pub id: i32,
    pub category_id: i32,
    pub description: Option<String>,
    pub creation_time: DateTime,
}

impl TableName for Pool {
    fn table_name() -> &'static str {
        "pool"
    }
}

impl Pool {
    pub fn new(conn: &mut PgConnection) -> QueryResult<Self> {
        let new_pool = NewPool {
            category_id: 0, // Default pool category
        };
        diesel::insert_into(pool::table)
            .values(&new_pool)
            .returning(Self::as_returning())
            .get_result(conn)
    }

    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        pool::table.count().first(conn)
    }

    pub fn post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        PoolPost::belonging_to(self).count().first(conn)
    }

    pub fn names(&self, conn: &mut PgConnection) -> QueryResult<Vec<String>> {
        PoolName::belonging_to(self).select(pool_name::name).load(conn)
    }

    pub fn add_name(&self, conn: &mut PgConnection, name: &str) -> QueryResult<PoolName> {
        let name_count = PoolName::belonging_to(self).count().first::<i64>(conn)?;
        let new_pool_name = NewPoolName {
            pool_id: self.id,
            order: i32::try_from(name_count).unwrap(),
            name,
        };
        diesel::insert_into(pool_name::table)
            .values(&new_pool_name)
            .returning(PoolName::as_returning())
            .get_result(conn)
    }

    pub fn add_post(&self, conn: &mut PgConnection, post: &Post) -> QueryResult<PoolPost> {
        let post_count = self.post_count(conn)?;
        let new_pool_post = NewPoolPost {
            pool_id: self.id,
            post_id: post.id,
            order: i32::try_from(post_count).unwrap(),
        };
        diesel::insert_into(pool_post::table)
            .values(&new_pool_post)
            .returning(PoolPost::as_returning())
            .get_result(conn)
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        util::delete(conn, &self)
    }
}

#[derive(Insertable)]
#[diesel(table_name = pool_name)]
pub struct NewPoolName<'a> {
    pub pool_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Pool))]
#[diesel(table_name = pool_name)]
#[diesel(check_for_backend(Pg))]
pub struct PoolName {
    pub id: i32,
    pub pool_id: i32,
    pub order: i32,
    pub name: String,
}

impl PoolName {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        pool_name::table.count().first(conn)
    }
}

pub type NewPoolPost = PoolPost;

#[derive(Associations, Queryable, Selectable)]
#[diesel(belongs_to(Post))]
#[diesel(table_name = pool_post)]
#[diesel(check_for_backend(Pg))]
pub struct PoolPostPostId {
    pub post_id: i32,
}

#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Pool), belongs_to(Post))]
#[diesel(table_name = pool_post)]
#[diesel(primary_key(pool_id, post_id))]
#[diesel(check_for_backend(Pg))]
pub struct PoolPost {
    pub pool_id: i32,
    pub post_id: i32,
    pub order: i32,
}

impl PoolPost {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        pool_post::table.count().first(conn)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::post::Post;
    use crate::test::*;

    #[test]
    fn save_pool() {
        let pool = test_transaction(|conn: &mut PgConnection| Pool::new(conn));
        assert_eq!(pool.category_id, 0);
    }

    #[test]
    fn cascade_deletions() {
        test_transaction(|conn: &mut PgConnection| {
            let post_count = Post::count(conn)?;
            let pool_count = Pool::count(conn)?;
            let pool_name_count = PoolName::count(conn)?;
            let pool_post_count = PoolPost::count(conn)?;

            let pool = Pool::new(conn)?;
            pool.add_name(conn, "test_pool")?;
            pool.add_name(conn, "test_pool_alias")?;
            create_test_user(conn, TEST_USERNAME)
                .and_then(|user| create_test_post(conn, &user))
                .and_then(|post| pool.add_post(conn, &post))?;

            assert_eq!(Post::count(conn)?, post_count + 1);
            assert_eq!(Pool::count(conn)?, pool_count + 1);
            assert_eq!(PoolName::count(conn)?, pool_name_count + 2);
            assert_eq!(PoolPost::count(conn)?, pool_post_count + 1);
            assert_eq!(pool.names(conn)?, vec!["test_pool", "test_pool_alias"]);

            pool.delete(conn)?;

            assert_eq!(Post::count(conn)?, post_count + 1);
            assert_eq!(Pool::count(conn)?, pool_count);
            assert_eq!(PoolName::count(conn)?, pool_name_count);
            assert_eq!(PoolPost::count(conn)?, pool_post_count);

            Ok(())
        });
    }

    #[test]
    fn track_post_count() {
        test_transaction(|conn: &mut PgConnection| {
            let user = create_test_user(conn, TEST_USERNAME)?;
            let post1 = create_test_post(conn, &user)?;
            let post2 = create_test_post(conn, &user)?;
            let pool1 = Pool::new(conn)?;
            let pool2 = Pool::new(conn)?;

            pool1.add_post(conn, &post1)?;
            pool2.add_post(conn, &post1)?;
            pool2.add_post(conn, &post2)?;

            assert_eq!(pool1.post_count(conn)?, 1);
            assert_eq!(pool2.post_count(conn)?, 2);
            assert_eq!(post1.pools_in(conn)?.len(), 2);
            assert_eq!(post2.pools_in(conn)?.len(), 1);

            post1.delete(conn)?;

            assert_eq!(pool1.post_count(conn)?, 0);
            assert_eq!(pool2.post_count(conn)?, 1);

            post2.delete(conn)?;

            assert_eq!(pool2.post_count(conn)?, 0);

            Ok(())
        });
    }
}
