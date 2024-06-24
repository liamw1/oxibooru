pub mod comment;
pub mod enums;
pub mod pool;
pub mod post;
pub mod snapshot;
pub mod tag;
pub mod user;

pub trait TableName {
    fn table_name() -> &'static str;
}
