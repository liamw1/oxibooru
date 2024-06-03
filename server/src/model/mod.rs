pub mod comment;
pub mod pool;
pub mod post;
pub mod privilege;
pub mod snapshot;
pub mod tag;
pub mod user;

pub trait TableName {
    fn table_name() -> &'static str;
}
