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
    ($query:expr, $expression:expr, $filter:expr) => {{
        use diesel::TextExpressionMethods;
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
    }};
}

/// Applies DISTINCT to the given `query` if the given `filter` is multivalued
/// Intended as an optimization for some range/wildcard queries.
#[macro_export]
macro_rules! apply_distinct_if_multivalued {
    ($query:expr, $filter:expr) => {
        if $crate::search::parse::is_multivalued($filter.condition) {
            $query.distinct()
        } else {
            $query
        }
    };
}

#[macro_export]
macro_rules! update_matching_filter_cache {
    ($conn:expr, $filtered_ids:expr, $column:expr, $state:expr) => {
        if !$state.completed {
            if $state.has_matching {
                // Matches are intersected
                let intersecting_subselect = $filtered_ids
                    .select(diesel::dsl::sql::<diesel::sql_types::Integer>("0"))
                    .filter(
                        diesel::dsl::sql::<diesel::sql_types::Bool>("")
                            .bind($column.eq($crate::search::temp::matching::id)),
                    );
                diesel::delete($crate::search::temp::matching::table)
                    .filter(diesel::dsl::not(diesel::dsl::exists(intersecting_subselect)))
                    .execute($conn)
            } else {
                $state.has_matching = true;
                diesel::sql_query("CREATE TEMP TABLE matching (id BIGINT PRIMARY KEY) ON COMMIT DROP")
                    .execute($conn)?;

                // Insert instead of intersecting for first positive filter
                diesel::insert_into($crate::search::temp::matching::table)
                    .values($filtered_ids)
                    .into_columns($crate::search::temp::matching::id)
                    .execute($conn)
            }
        } else {
            Ok(0)
        }
    };
}

#[macro_export]
macro_rules! update_nonmatching_filter_cache {
    ($conn:expr, $filtered_ids:expr, $state:expr) => {
        if !$state.completed {
            // Nonmatches are unioned
            let insert_statement = diesel::insert_into($crate::search::temp::nonmatching::table)
                .values($filtered_ids)
                .into_columns($crate::search::temp::nonmatching::id);

            if $state.has_nonmatching {
                // Avoid primary key conflicts if nonmatching table already has rows
                insert_statement.on_conflict_do_nothing().execute($conn)
            } else {
                $state.has_nonmatching = true;
                diesel::sql_query("CREATE TEMP TABLE nonmatching (id BIGINT PRIMARY KEY) ON COMMIT DROP")
                    .execute($conn)?;

                insert_statement.execute($conn)
            }
        } else {
            Ok(0)
        }
    };
}

/// Stores the results of a complex filter to one of the TEMP tables.
/// Positive filter results will be intersected with the `matching` table while
/// negative filter results will be unioned with the `nonmatching` table.
///
/// `filtered_ids` should be a query that returns the IDs of the resources matching the unnegated filter.
/// `column` should be a column that represents the ID of the resource.
#[macro_export]
macro_rules! update_filter_cache {
    ($conn:expr, $filtered_ids:expr, $column:expr, $filter:expr, $state:expr) => {
        if $filter.negated {
            $crate::update_nonmatching_filter_cache!($conn, $filtered_ids, $state)
        } else {
            $crate::update_matching_filter_cache!($conn, $filtered_ids, $column, $state)
        }
    };
}

/// Applies the cached results in the `matching` and `nonmatching` TEMP tables
/// as a filter on the specified `column`. When this function is called, the
/// cache is considered fully built and any subsequent calls to [`update_filter_cache`]
/// in the same transaction won't modify the cache.
#[macro_export]
macro_rules! apply_cache_filters {
    ($query:expr, $column:expr, $state:expr) => {{
        $state.completed = true;
        let query = if $state.has_matching {
            let matching_subselect = $crate::search::temp::matching::table
                .select(diesel::dsl::sql::<diesel::sql_types::Integer>("0"))
                .filter($crate::search::temp::matching::id.eq($column));
            $query.filter(diesel::dsl::exists(matching_subselect))
        } else {
            $query
        };
        if $state.has_nonmatching {
            let nonmatching_subselect = $crate::search::temp::nonmatching::table
                .select(diesel::dsl::sql::<diesel::sql_types::Integer>("0"))
                .filter($crate::search::temp::nonmatching::id.eq($column));
            query.filter(diesel::dsl::not(diesel::dsl::exists(nonmatching_subselect)))
        } else {
            query
        }
    }};
}

/// Applies an ordering to the given `query`.
/// Order is either ASC or DESC.
#[macro_export]
macro_rules! apply_sort {
    ($query:expr, $expression:expr, $sort:expr) => {{
        use diesel::PgSortExpressionMethods;
        match $sort.order {
            $crate::search::Order::Asc => $query.then_order_by($expression.asc()),
            $crate::search::Order::Desc => $query.then_order_by($expression.desc().nulls_last()),
        }
    }};
}

/// Applies seeded random ordering to the given `query`.
#[macro_export]
macro_rules! apply_random_sort {
    ($conn:expr, $client:expr, $query:expr, $criteria:expr) => {{
        if let Err(err) = $crate::search::set_seed($conn, $client) {
            tracing::warn!(
                "Unable to set seed for random sort. Results may not be consistent between requests. Details:\n{err}"
            );
        }
        match $criteria.extra_args {
            Some(args) => $query
                .order($crate::search::random())
                .offset(args.offset)
                .limit(args.limit),
            None => $query.order($crate::search::random()),
        }
    }};
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
