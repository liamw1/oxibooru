use crate::search::{Criteria, Error, TimeParsingError, UnparsedFilter};
use crate::util::DateTime;
use diesel::dsl::AsExpr;
use diesel::expression::{is_aggregate, MixedAggregates, ValidGrouping};
use diesel::internal::table_macro::{BoxedSelectStatement, FromClause};
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_builder::{AsQuery, QueryFragment};
use diesel::sql_types::{BigInt, Integer, SmallInt, Timestamptz, VarChar};
use std::str::FromStr;

type BoxedQuery<'a, C> =
    BoxedSelectStatement<'a, <<C as Column>::Table as AsQuery>::SqlType, FromClause<<C as Column>::Table>, Pg>;

pub fn apply_i16_filter<'a, T, C>(
    query: BoxedQuery<'a, C>,
    column: C,
    filter: UnparsedFilter<T>,
) -> Result<BoxedQuery<'a, C>, Error>
where
    C: Column
        + AppearsOnTable<C::Table>
        + ValidGrouping<()>
        + ExpressionMethods
        + Expression<SqlType = SmallInt>
        + QueryFragment<Pg>
        + Send
        + 'a,
    C::IsAggregate: MixedAggregates<<AsExpr<i16, C> as ValidGrouping<()>>::IsAggregate, Output = is_aggregate::No>,
{
    parse_criteria::<i16>(filter.criteria).map(|criteria| {
        if filter.negated {
            match criteria {
                Criteria::Values(values) => query.filter(column.ne_all(values)),
                Criteria::GreaterEq(value) => query.filter(column.lt(value)),
                Criteria::LessEq(value) => query.filter(column.gt(value)),
                Criteria::Range(range) => query.filter(column.not_between(range.start, range.end)),
            }
        } else {
            match criteria {
                Criteria::Values(values) => query.filter(column.eq_any(values)),
                Criteria::GreaterEq(value) => query.filter(column.ge(value)),
                Criteria::LessEq(value) => query.filter(column.le(value)),
                Criteria::Range(range) => query.filter(column.between(range.start, range.end)),
            }
        }
    })
}

pub fn apply_i32_filter<'a, T, C>(
    query: BoxedQuery<'a, C>,
    column: C,
    filter: UnparsedFilter<T>,
) -> Result<BoxedQuery<'a, C>, Error>
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
    parse_criteria::<i32>(filter.criteria).map(|criteria| {
        if filter.negated {
            match criteria {
                Criteria::Values(values) => query.filter(column.ne_all(values)),
                Criteria::GreaterEq(value) => query.filter(column.lt(value)),
                Criteria::LessEq(value) => query.filter(column.gt(value)),
                Criteria::Range(range) => query.filter(column.not_between(range.start, range.end)),
            }
        } else {
            match criteria {
                Criteria::Values(values) => query.filter(column.eq_any(values)),
                Criteria::GreaterEq(value) => query.filter(column.ge(value)),
                Criteria::LessEq(value) => query.filter(column.le(value)),
                Criteria::Range(range) => query.filter(column.between(range.start, range.end)),
            }
        }
    })
}

pub fn apply_i64_filter<'a, T, C>(
    query: BoxedQuery<'a, C>,
    column: C,
    filter: UnparsedFilter<T>,
) -> Result<BoxedQuery<'a, C>, Error>
where
    C: Column
        + AppearsOnTable<C::Table>
        + ValidGrouping<()>
        + ExpressionMethods
        + Expression<SqlType = BigInt>
        + QueryFragment<Pg>
        + Send
        + 'a,
    C::IsAggregate: MixedAggregates<<AsExpr<i64, C> as ValidGrouping<()>>::IsAggregate, Output = is_aggregate::No>,
{
    parse_criteria::<i64>(filter.criteria).map(|criteria| {
        if filter.negated {
            match criteria {
                Criteria::Values(values) => query.filter(column.ne_all(values)),
                Criteria::GreaterEq(value) => query.filter(column.lt(value)),
                Criteria::LessEq(value) => query.filter(column.gt(value)),
                Criteria::Range(range) => query.filter(column.not_between(range.start, range.end)),
            }
        } else {
            match criteria {
                Criteria::Values(values) => query.filter(column.eq_any(values)),
                Criteria::GreaterEq(value) => query.filter(column.ge(value)),
                Criteria::LessEq(value) => query.filter(column.le(value)),
                Criteria::Range(range) => query.filter(column.between(range.start, range.end)),
            }
        }
    })
}

pub fn apply_time_filter<'a, T, C>(
    query: BoxedQuery<'a, C>,
    column: C,
    filter: UnparsedFilter<T>,
) -> Result<BoxedQuery<'a, C>, Error>
where
    C: Column
        + AppearsOnTable<C::Table>
        + ValidGrouping<()>
        + ExpressionMethods
        + Expression<SqlType = Timestamptz>
        + QueryFragment<Pg>
        + Send
        + 'a,
    C::IsAggregate: MixedAggregates<<AsExpr<DateTime, C> as ValidGrouping<()>>::IsAggregate, Output = is_aggregate::No>,
{
    parse_time_criteria(filter.criteria).map(|criteria| {
        if filter.negated {
            match criteria {
                Criteria::Values(values) => query.filter(column.ne_all(values)),
                Criteria::GreaterEq(value) => query.filter(column.lt(value)),
                Criteria::LessEq(value) => query.filter(column.gt(value)),
                Criteria::Range(range) => query.filter(column.not_between(range.start, range.end)),
            }
        } else {
            match criteria {
                Criteria::Values(values) => query.filter(column.eq_any(values)),
                Criteria::GreaterEq(value) => query.filter(column.ge(value)),
                Criteria::LessEq(value) => query.filter(column.le(value)),
                Criteria::Range(range) => query.filter(column.between(range.start, range.end)),
            }
        }
    })
}

pub fn apply_str_filter<'a, T, C>(
    query: BoxedQuery<'a, C>,
    column: C,
    filter: UnparsedFilter<'a, T>,
) -> BoxedQuery<'a, C>
where
    C: Column
        + AppearsOnTable<C::Table>
        + ValidGrouping<()>
        + ExpressionMethods
        + Expression<SqlType = VarChar>
        + QueryFragment<Pg>
        + Send
        + 'a,
    C::IsAggregate: MixedAggregates<<AsExpr<&'a str, C> as ValidGrouping<()>>::IsAggregate, Output = is_aggregate::No>,
{
    let criteria = parse_str_criteria(filter.criteria);
    if filter.negated {
        match criteria {
            Criteria::Values(values) => query.filter(column.ne_all(values)),
            Criteria::GreaterEq(value) => query.filter(column.lt(value)),
            Criteria::LessEq(value) => query.filter(column.gt(value)),
            Criteria::Range(range) => query.filter(column.not_between(range.start, range.end)),
        }
    } else {
        match criteria {
            Criteria::Values(values) => query.filter(column.eq_any(values)),
            Criteria::GreaterEq(value) => query.filter(column.ge(value)),
            Criteria::LessEq(value) => query.filter(column.le(value)),
            Criteria::Range(range) => query.filter(column.between(range.start, range.end)),
        }
    }
}

fn parse_time(time: &str) -> Result<DateTime, TimeParsingError> {
    if time == "today" {
        return Ok(DateTime::today());
    } else if time == "yesterday" {
        return Ok(DateTime::yesterday());
    }

    let mut date_iterator = time.split('-');
    let year: i32 = date_iterator
        .next()
        .ok_or(TimeParsingError::TooFewArgs)
        .and_then(|value| value.parse().map_err(TimeParsingError::from))?;
    let month: Option<u8> = date_iterator.next().map(|value| value.parse()).transpose()?;
    let day: Option<u8> = date_iterator.next().map(|value| value.parse()).transpose()?;
    if date_iterator.next().is_some() {
        return Err(TimeParsingError::TooManyArgs);
    }

    DateTime::from_date(year, month.unwrap_or(1), day.unwrap_or(1)).map_err(TimeParsingError::from)
}

fn parse_time_criteria(filter: &str) -> Result<Criteria<DateTime>, Error> {
    if let Some(split_str) = filter.split_once("..") {
        return match split_str {
            (left, "") => parse_time(left).map(Criteria::GreaterEq).map_err(Error::from),
            ("", right) => parse_time(right).map(Criteria::LessEq).map_err(Error::from),
            (left, right) => Ok(Criteria::Range(parse_time(left)?..parse_time(right)?)),
        };
    }
    filter
        .split(',')
        .map(parse_time)
        .collect::<Result<_, _>>()
        .map(Criteria::Values)
        .map_err(Error::from)
}

fn parse_str_criteria(filter: &str) -> Criteria<&str> {
    if let Some(split_str) = filter.split_once("..") {
        return match split_str {
            (left, "") => Criteria::GreaterEq(left),
            ("", right) => Criteria::LessEq(right),
            (left, right) => Criteria::Range(left..right),
        };
    }
    Criteria::Values(filter.split(',').collect())
}

fn parse_criteria<T>(filter: &str) -> Result<Criteria<T>, Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error,
    <T as FromStr>::Err: 'static,
{
    if let Some(split_str) = filter.split_once("..") {
        return match split_str {
            (left, "") => Ok(Criteria::GreaterEq(left.parse().map_err(Box::from)?)),
            ("", right) => Ok(Criteria::LessEq(right.parse().map_err(Box::from)?)),
            (left, right) => Ok(Criteria::Range(left.parse().map_err(Box::from)?..right.parse().map_err(Box::from)?)),
        };
    }
    filter
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map(Criteria::Values)
        .map_err(Box::from)
        .map_err(Error::from)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::enums::PostSafety;

    #[test]
    fn time_parsing() {
        assert_eq!(parse_time("1970").unwrap(), DateTime::from_date(1970, 1, 1).unwrap());
        assert_eq!(parse_time("2024").unwrap(), DateTime::from_date(2024, 1, 1).unwrap());
        assert_eq!(parse_time("2024-7").unwrap(), DateTime::from_date(2024, 7, 1).unwrap());
        assert_eq!(parse_time("2024-7-11").unwrap(), DateTime::from_date(2024, 7, 11).unwrap());
        assert_eq!(parse_time("2024-07-11").unwrap(), DateTime::from_date(2024, 7, 11).unwrap());
        assert_eq!(parse_time("2024-2-29").unwrap(), DateTime::from_date(2024, 2, 29).unwrap());

        assert!(parse_time("").is_err());
        assert!(parse_time("2000-01-01-01").is_err());
        assert!(parse_time("2025-2-29").is_err());
        assert!(parse_time("Hello World!").is_err());
        assert!(parse_time("a-b-c").is_err());
        assert!(parse_time("--").is_err());
    }

    #[test]
    fn criteria_parsing() {
        assert_eq!(parse_criteria("1").unwrap(), Criteria::Values(vec![1]));
        assert_eq!(parse_criteria("-137").unwrap(), Criteria::Values(vec![-137]));
        assert_eq!(parse_criteria("0,1,2,3").unwrap(), Criteria::Values(vec![0, 1, 2, 3]));
        assert_eq!(parse_criteria("-4,-1,0,0,70,6").unwrap(), Criteria::Values(vec![-4, -1, 0, 0, 70, 6]));
        assert_eq!(parse_criteria("7..").unwrap(), Criteria::GreaterEq(7));
        assert_eq!(parse_criteria("..7").unwrap(), Criteria::LessEq(7));
        assert_eq!(parse_criteria("0..1").unwrap(), Criteria::Range(0..1));
        assert_eq!(parse_criteria("-10..5").unwrap(), Criteria::Range(-10..5));

        assert_eq!(parse_str_criteria("str"), Criteria::Values(vec!["str"]));
        assert_eq!(parse_str_criteria("a,b,c"), Criteria::Values(vec!["a", "b", "c"]));
        assert_eq!(parse_str_criteria("a.."), Criteria::GreaterEq("a"));
        assert_eq!(parse_str_criteria("..z"), Criteria::LessEq("z"));
        assert_eq!(parse_str_criteria("a..z"), Criteria::Range("a".."z"));

        assert_eq!(parse_criteria("safe").unwrap(), Criteria::Values(vec![PostSafety::Safe]));
        assert_eq!(parse_criteria("safe..unsafe").unwrap(), Criteria::Range(PostSafety::Safe..PostSafety::Unsafe));
    }
}
