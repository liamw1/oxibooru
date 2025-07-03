// Working with generic functions in Diesel is a nightmare, so I've created
// these macros to help with dynamically adding filters to a boxed query.

/// Used for applying filters on non-string non-time expressions.
#[macro_export]
macro_rules! apply_filter {
    ($query:expr, $expression:expr, $filter:expr, $condition_type:ty) => {
        $crate::search::parse::condition::<$condition_type>($filter.condition)
            .map(|condition| $crate::apply_condition!($query, $expression, $filter, condition))
    };
}

/// Used for applying filters on time-based expressions.
#[macro_export]
macro_rules! apply_time_filter {
    ($query:expr, $expression:expr, $filter:expr) => {
        $crate::search::parse::time_condition($filter.condition).map(|condition| {
            let condition = match condition {
                $crate::search::Condition::Values(times) => {
                    type TimeRange = diesel::pg::sql_types::Range<diesel::sql_types::Timestamptz>;
                    let expression = diesel::dsl::sql::<diesel::sql_types::Bool>("tstzmultirange(VARIADIC ")
                        .bind::<diesel::sql_types::Array<TimeRange>, _>(times)
                        .sql(") @> ")
                        .bind($expression);
                    return match $filter.negated {
                        true => $query.filter(diesel::dsl::not(expression)),
                        false => $query.filter(expression),
                    };
                }
                $crate::search::Condition::GreaterEq(time) => $crate::search::Condition::GreaterEq(time.start),
                $crate::search::Condition::LessEq(time) => $crate::search::Condition::LessEq(time.start),
                $crate::search::Condition::Range(range) => {
                    $crate::search::Condition::Range(range.start.start..range.end.end)
                }
            };
            $crate::apply_condition!($query, $expression, $filter, condition)
        })
    };
}

/// Used for applying filters on string-based expressions.
#[macro_export]
macro_rules! apply_str_filter {
    ($query:expr, $expression:expr, $filter:expr) => {
        match $crate::search::parse::str_condition($filter.condition) {
            $crate::search::StrCondition::Regular(condition) => {
                $crate::apply_condition!($query, $expression, $filter, condition)
            }
            $crate::search::StrCondition::WildCard(pattern) => match $filter.negated {
                // Even though most text-based columns are CITEXT, we cast to lower
                // here to get postgres to use TEXT index. We do this because CITEXT
                // indexes do not work with patterns.
                true => $query.filter($crate::search::lower($expression).not_like(pattern)),
                false => $query.filter($crate::search::lower($expression).like(pattern)),
            },
        }
    };
}

/// Applies DISTINCT to the given `query` if the given `filter` is multivalued
/// Intended as an optimization for some range/wildcard queries.
#[macro_export]
macro_rules! apply_distinct_if_multivalued {
    ($query:expr, $filter:expr) => {
        match $filter.is_multivalued() {
            true => $query.distinct(),
            false => $query,
        }
    };
}

/// Applies an ordering to the given `query`.
/// Order is either ASC or DESC.
#[macro_export]
macro_rules! apply_sort {
    ($query:expr, $expression:expr, $sort:expr) => {
        match $sort.order {
            $crate::search::Order::Asc => $query.then_order_by($expression.asc()),
            $crate::search::Order::Desc => $query.then_order_by($expression.desc().nulls_last()),
        }
    };
}

/// Applies random ordering to the given `query`.
#[macro_export]
macro_rules! apply_random_sort {
    ($query:expr, $criteria:expr) => {
        match $criteria.extra_args {
            Some(args) => $query
                .order($crate::search::random())
                .offset(args.offset)
                .limit(args.limit),
            None => $query.order($crate::search::random()),
        }
    };
}

/// Applies a WHERE clause to the given `query`.
/// The `filter` determines what operation is applied to the given `expression`.
/// The operation is one of: eq_any, ge, le, between, ne_all, lt, gt, or not_between.
#[doc(hidden)]
#[macro_export]
macro_rules! apply_condition {
    ($query:expr, $expression:expr, $filter:expr, $condition:expr) => {
        if $filter.negated {
            match $condition {
                $crate::search::Condition::Values(values) => $query.filter($expression.ne_all(values)),
                $crate::search::Condition::GreaterEq(value) => $query.filter($expression.lt(value)),
                $crate::search::Condition::LessEq(value) => $query.filter($expression.gt(value)),
                $crate::search::Condition::Range(range) => {
                    $query.filter($expression.not_between(range.start, range.end))
                }
            }
        } else {
            match $condition {
                $crate::search::Condition::Values(values) => $query.filter($expression.eq_any(values)),
                $crate::search::Condition::GreaterEq(value) => $query.filter($expression.ge(value)),
                $crate::search::Condition::LessEq(value) => $query.filter($expression.le(value)),
                $crate::search::Condition::Range(range) => $query.filter($expression.between(range.start, range.end)),
            }
        }
    };
}
