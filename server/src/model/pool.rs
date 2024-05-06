use crate::model::post::Post;
use crate::schema::{pool, pool_category, pool_name, pool_post};
use crate::util;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use std::option::Option;

#[derive(Insertable)]
#[diesel(table_name = pool_category)]
pub struct NewPoolCategory<'a> {
    pub name: &'a str,
    pub color: &'a str,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = pool_category)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PoolCategory {
    pub id: i32,
    pub name: String,
    pub color: String,
}

impl PoolCategory {
    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        pool_category::table.count().first(conn)
    }
}

#[derive(Insertable)]
#[diesel(table_name = pool)]
pub struct NewPool {
    pub category_id: i32,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(PoolCategory, foreign_key = category_id))]
#[diesel(table_name = pool)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Pool {
    pub id: i32,
    pub category_id: i32,
    pub description: Option<String>,
    pub creation_time: DateTime<Utc>,
    pub last_edit_time: DateTime<Utc>,
}

impl Pool {
    pub fn new(conn: &mut PgConnection) -> QueryResult<Pool> {
        let now = chrono::Utc::now();
        let new_pool = NewPool {
            category_id: 0, // Default pool category
            creation_time: now,
            last_edit_time: now,
        };
        diesel::insert_into(pool::table)
            .values(&new_pool)
            .returning(Pool::as_returning())
            .get_result(conn)
    }

    pub fn count(conn: &mut PgConnection) -> QueryResult<i64> {
        pool::table.count().first(conn)
    }

    pub fn post_count(&self, conn: &mut PgConnection) -> QueryResult<i64> {
        PoolPost::belonging_to(self).count().first(conn)
    }

    pub fn add_name(&self, conn: &mut PgConnection, name: &str) -> QueryResult<PoolName> {
        let new_pool_name = NewPoolName {
            pool_id: self.id,
            order: 0,
            name,
        };
        diesel::insert_into(pool_name::table)
            .values(&new_pool_name)
            .returning(PoolName::as_returning())
            .get_result(conn)
    }

    pub fn add_post(&self, conn: &mut PgConnection, post: &Post) -> QueryResult<PoolPost> {
        let new_pool_post = NewPoolPost {
            pool_id: self.id,
            post_id: post.id,
            order: 0,
        };
        diesel::insert_into(pool_post::table)
            .values(&new_pool_post)
            .returning(PoolPost::as_returning())
            .get_result(conn)
    }

    pub fn delete(self, conn: &mut PgConnection) -> QueryResult<()> {
        conn.transaction(|conn| util::validate_uniqueness("pool", diesel::delete(&self).execute(conn)?))
    }
}

#[derive(Insertable)]
#[diesel(table_name = pool_name)]
pub struct NewPoolName<'a> {
    pub pool_id: i32,
    pub order: i32,
    pub name: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Pool))]
#[diesel(table_name = pool_name)]
#[diesel(check_for_backend(diesel::pg::Pg))]
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

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Pool), belongs_to(Post))]
#[diesel(table_name = pool_post)]
#[diesel(primary_key(pool_id, post_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
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
    use super::{Pool, PoolName, PoolPost};
    use crate::model::post::Post;
    use crate::test::*;
    use diesel::prelude::*;
    use diesel::result::Error;

    #[test]
    fn test_saving_pool() {
        let pool = establish_connection_or_panic().test_transaction::<Pool, Error, _>(|conn| Pool::new(conn));
        assert_eq!(pool.category_id, 0, "New pool is not in default category");
    }

    #[test]
    fn test_cascade_deletions() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let post_count = Post::count(conn)?;
            let pool_count = Pool::count(conn)?;
            let pool_name_count = PoolName::count(conn)?;
            let pool_post_count = PoolPost::count(conn)?;

            let pool = Pool::new(conn)?;
            pool.add_name(conn, "test_pool")?;
            pool.add_name(conn, "test_pool_alias")?;
            create_test_user(conn, test_user_name())
                .and_then(|user| create_test_post(conn, &user))
                .and_then(|post| pool.add_post(conn, &post))?;

            assert_eq!(Post::count(conn)?, post_count + 1, "Post insertion failed");
            assert_eq!(Pool::count(conn)?, pool_count + 1, "Pool insertion failed");
            assert_eq!(PoolName::count(conn)?, pool_name_count + 2, "Pool name insertion failed");
            assert_eq!(PoolPost::count(conn)?, pool_post_count + 1, "Pool post insertion failed");

            pool.delete(conn)?;

            assert_eq!(Post::count(conn)?, post_count + 1, "Post should not have been deleted");
            assert_eq!(Pool::count(conn)?, pool_count, "Pool deletion failed");
            assert_eq!(PoolName::count(conn)?, pool_name_count, "Pool name cascade deletion failed");
            assert_eq!(PoolPost::count(conn)?, pool_post_count, "Pool post cascade deletion failed");

            Ok(())
        });
    }

    #[test]
    fn test_tracking_post_count() {
        establish_connection_or_panic().test_transaction::<_, Error, _>(|conn| {
            let user = create_test_user(conn, test_user_name())?;
            let post1 = create_test_post(conn, &user)?;
            let post2 = create_test_post(conn, &user)?;
            let pool1 = Pool::new(conn)?;
            let pool2 = Pool::new(conn)?;

            pool1.add_post(conn, &post1)?;
            pool2.add_post(conn, &post1)?;
            pool2.add_post(conn, &post2)?;

            assert_eq!(pool1.post_count(conn)?, 1, "Pool should have one post");
            assert_eq!(pool2.post_count(conn)?, 2, "Pool should have two posts");
            assert_eq!(post1.pools_in(conn)?.len(), 2, "Post should be in two pools");
            assert_eq!(post2.pools_in(conn)?.len(), 1, "Post should be in one pool");

            post1.delete(conn)?;

            assert_eq!(pool1.post_count(conn)?, 0, "Pool should now have no posts");
            assert_eq!(pool2.post_count(conn)?, 1, "Pool should now have one post");

            post2.delete(conn)?;

            assert_eq!(pool2.post_count(conn)?, 0, "Both pools should be empty");

            Ok(())
        });
    }
}
