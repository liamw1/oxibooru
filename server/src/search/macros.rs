/*
    Working with generic functions is Diesel is a nightmare, so I've created
    these macros to help with dynamically adding filters to a boxed query.
*/

#[macro_export]
macro_rules! apply_str_filter {
    ($query:expr, $expression:expr, $filter:expr) => {
        $crate::apply_criteria!($query, $expression, $filter, $crate::search::parse::str_criteria($filter.criteria))
    };
}

#[macro_export]
macro_rules! apply_time_filter {
    ($query:expr, $expression:expr, $filter:expr) => {
        $crate::search::parse::time_criteria($filter.criteria)
            .map(|criteria| $crate::apply_criteria!($query, $expression, $filter, criteria))
    };
}

#[macro_export]
macro_rules! apply_filter {
    ($query:expr, $expression:expr, $filter:expr, $criteria_type:ty) => {
        $crate::search::parse::criteria::<$criteria_type>($filter.criteria)
            .map(|criteria| $crate::apply_criteria!($query, $expression, $filter, criteria))
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! apply_criteria {
    ($query:expr, $expression:expr, $filter:expr, $criteria:expr) => {
        if $filter.negated {
            match $criteria {
                $crate::search::Criteria::Values(values) => $query.filter($expression.ne_all(values)),
                $crate::search::Criteria::GreaterEq(value) => $query.filter($expression.lt(value)),
                $crate::search::Criteria::LessEq(value) => $query.filter($expression.gt(value)),
                $crate::search::Criteria::Range(range) => {
                    $query.filter($expression.not_between(range.start, range.end))
                }
            }
        } else {
            match $criteria {
                $crate::search::Criteria::Values(values) => $query.filter($expression.eq_any(values)),
                $crate::search::Criteria::GreaterEq(value) => $query.filter($expression.ge(value)),
                $crate::search::Criteria::LessEq(value) => $query.filter($expression.le(value)),
                $crate::search::Criteria::Range(range) => $query.filter($expression.between(range.start, range.end)),
            }
        }
    };
}

#[macro_export]
macro_rules! apply_having_clause {
    ($query:expr, $expression:expr, $filter:expr) => {
        $crate::search::parse::criteria::<i64>($filter.criteria).map(|criteria| {
            if $filter.negated {
                match criteria {
                    $crate::search::Criteria::Values(values) => $query.having($expression.ne_all(values)),
                    $crate::search::Criteria::GreaterEq(value) => $query.having($expression.lt(value)),
                    $crate::search::Criteria::LessEq(value) => $query.having($expression.gt(value)),
                    $crate::search::Criteria::Range(range) => {
                        $query.having($expression.not_between(range.start, range.end))
                    }
                }
            } else {
                match criteria {
                    $crate::search::Criteria::Values(values) => $query.having($expression.eq_any(values)),
                    $crate::search::Criteria::GreaterEq(value) => $query.having($expression.ge(value)),
                    $crate::search::Criteria::LessEq(value) => $query.having($expression.le(value)),
                    $crate::search::Criteria::Range(range) => {
                        $query.having($expression.between(range.start, range.end))
                    }
                }
            }
        })
    };
}

#[macro_export]
macro_rules! apply_sort {
    ($query:expr, $expression:expr, $sort:expr) => {
        match $sort.order {
            $crate::search::Order::Asc => $query.then_order_by($expression.asc()),
            $crate::search::Order::Desc => $query.then_order_by($expression.desc()),
        }
    };
}

#[macro_export]
macro_rules! finalize {
    ($query:expr, $expression:expr, $sort:expr, $extra_args:expr) => {{
        let query = match $extra_args {
            Some(args) => $query.offset(args.offset).limit(args.limit),
            None => $query,
        };
        $crate::apply_sort!(query, $expression, $sort)
    }};
}
