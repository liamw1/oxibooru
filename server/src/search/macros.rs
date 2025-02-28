// Working with generic functions is Diesel is a nightmare, so I've created
// these macros to help with dynamically adding filters to a boxed query.

/// Used for applying filters on non-string non-time expressions.
#[macro_export]
macro_rules! apply_filter {
    ($query:expr, $expression:expr, $filter:expr, $criteria_type:ty) => {
        $crate::search::parse::criteria::<$criteria_type>($filter.criteria)
            .map(|criteria| $crate::apply_criteria!($query, $expression, $filter, criteria))
    };
}

/// Used for applying filters on time-based expressions.
#[macro_export]
macro_rules! apply_time_filter {
    ($query:expr, $expression:expr, $filter:expr) => {
        $crate::search::parse::time_criteria($filter.criteria)
            .map(|criteria| $crate::apply_criteria!($query, $expression, $filter, criteria))
    };
}

/// Used for applying filters on string-based expressions.
#[macro_export]
macro_rules! apply_str_filter {
    ($query:expr, $expression:expr, $filter:expr) => {
        match $crate::search::parse::str_criteria($filter.criteria) {
            $crate::search::StrCritera::Regular(criteria) => {
                $crate::apply_criteria!($query, $expression, $filter, criteria)
            }
            $crate::search::StrCritera::WildCard(pattern) => {
                if $filter.negated {
                    $query.filter($expression.not_like(pattern))
                } else {
                    $query.filter($expression.like(pattern))
                }
            }
        }
    };
}

/// Used for filtering based on subqueries which establish a one-to-many or many-to-many relationship.
///
/// For instance, let's say we have a query to find all posts that don't have the tag "wallpaper".
/// Dynamic joins aren't easy, so instead we use subqueries. We perform a subquery that joins the
/// post_tag table with the tag_name table and can return a list of post_tags matching our condition.
/// If we were to apply our filter naively, we would have a list of all post_tags associated with any
/// tag other than "wallpaper". But this means that our query would return all posts with any tag other
/// than "wallpaper", including posts with the tag "wallpaper"!
///
/// Instead, we should apply our filter without negation to the subquery, which returns post_tags
/// associated with "wallpaper". Then, we can plug that subquery into this function which performs
/// the negation, giving us the desired result.
#[macro_export]
macro_rules! apply_subquery_filter {
    ($query:expr, $expression:expr, $filter:expr, $subquery:expr) => {
        if $filter.negated {
            $query.filter($expression.ne_all(diesel::dsl::array($subquery)))
        } else {
            $query.filter($expression.eq_any(diesel::dsl::array($subquery)))
        }
    };
}

/// Applies a WHERE clause to the given `query`.
/// The `filter` determines what operation is applied to the given `expression`.
/// The operation is one of: eq_any, ge, le, between, ne_all, lt, gt, or not_between.
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
