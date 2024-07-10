pub mod parse;
pub mod post;

use diesel::dsl::AsExpr;
use diesel::expression::{is_aggregate, MixedAggregates, ValidGrouping};
use diesel::internal::table_macro::{BoxedSelectStatement, FromClause};
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_builder::{AsQuery, QueryFragment};
use diesel::sql_types::Integer;
use std::ops::Range;

trait ColumnFilter<'a, C>
where
    C: Column,
{
    // We can alias these ugly types once associated type defaults are stabilized
    fn apply(
        self,
        query: BoxedSelectStatement<'a, <C::Table as AsQuery>::SqlType, FromClause<C::Table>, Pg>,
    ) -> BoxedSelectStatement<'a, <C::Table as AsQuery>::SqlType, FromClause<C::Table>, Pg>;
}

#[derive(Debug, PartialEq, Eq)]
enum FilterType<V> {
    Values(Vec<V>),
    GreaterEq(V),
    LessEq(V),
    Range(Range<V>),
}

struct SimpleFilter<C, V> {
    filter_type: FilterType<V>,
    negated: bool,
    column: C,
}

impl<'a, C> ColumnFilter<'a, C> for SimpleFilter<C, i32>
where
    C: Column
        + AppearsOnTable<C::Table>
        + ValidGrouping<()>
        + ExpressionMethods
        + Expression<SqlType = Integer>
        + QueryFragment<Pg>
        + Send
        + 'a,
    C::IsAggregate: MixedAggregates<<AsExpr<i32, C> as ValidGrouping<()>>::IsAggregate, Output = is_aggregate::No>,
{
    fn apply(
        self,
        query: BoxedSelectStatement<'a, <C::Table as AsQuery>::SqlType, FromClause<C::Table>, Pg>,
    ) -> BoxedSelectStatement<'a, <C::Table as AsQuery>::SqlType, FromClause<C::Table>, Pg> {
        if self.negated {
            match self.filter_type {
                FilterType::Values(values) => query.filter(self.column.ne_all(values)),
                FilterType::GreaterEq(value) => query.filter(self.column.lt(value)),
                FilterType::LessEq(value) => query.filter(self.column.gt(value)),
                FilterType::Range(range) => query.filter(self.column.not_between(range.start, range.end)),
            }
        } else {
            match self.filter_type {
                FilterType::Values(values) => query.filter(self.column.eq_any(values)),
                FilterType::GreaterEq(value) => query.filter(self.column.ge(value)),
                FilterType::LessEq(value) => query.filter(self.column.le(value)),
                FilterType::Range(range) => query.filter(self.column.between(range.start, range.end)),
            }
        }
    }
}
