use crate::string::SmallString;
use diesel::Identifiable;
use std::collections::HashMap;
use std::ops::IndexMut;
use std::rc::Rc;
use std::str::FromStr;

pub mod comment;
pub mod pool;
pub mod pool_category;
pub mod post;
pub mod snapshot;
pub mod tag;
pub mod tag_category;
pub mod user;
pub mod user_token;

// NOTE: The more complicated queries in this module rely on the behavior of diesel's
// grouped_by function preserving the relative order between elements. This seems to be the
// case, and the most straightforward way of implementing the function would have this behavior,
// but I don't see this as a guarantee anywhere in the documentation. If this changes, I'll need
// to reimplement a similar function with this behavior.

pub trait BoolFill {
    fn filled(val: bool) -> Self;
}

/// Creates a boolean `FieldTable` from an (optional) comma separated `fields` [str].
pub fn create_table<T, E>(fields: Option<&str>) -> Result<T, <E as FromStr>::Err>
where
    T: BoolFill + IndexMut<E, Output = bool>,
    E: FromStr,
{
    if let Some(fields_str) = fields {
        let mut table = T::filled(false);
        for field in fields_str.split(',') {
            table[E::from_str(field)?] = true;
        }
        Ok(table)
    } else {
        Ok(T::filled(true))
    }
}

/// Validates that a batch retrieval is the expected size.
fn check_batch_results(batch_size: usize, retrieved_size: usize) {
    assert!(retrieved_size == 0 || retrieved_size == batch_size);
}

/// Validates that a retrieval designed to fetch one element actually does contain only one element.
fn single<T>(mut batch: Vec<T>) -> T {
    assert_eq!(batch.len(), 1);
    batch.pop().expect("Batch contains exactly one element")
}

/// Convience function that shortens line counts.
fn retrieve<T, E, F>(enabled: bool, mut function: F) -> Result<Vec<T>, E>
where
    F: FnMut() -> Result<Vec<T>, E>,
{
    if enabled { function() } else { Ok(Vec::new()) }
}

/// For a given set of resources, orders them so that their primary keys are in the same order
/// as the order slice, which should be the same length as values.
///
/// NOTE: This algorithm is O(n^2) in `values.len()`, which could be made O(n) with a `HashMap` implementation.
/// However, for small n this Vec-based implementation is probably much faster. Since we retrieve
/// 40-50 resources at a time, I'm leaving it like this for the time being until it proves to be slow.
fn order_as<T>(values: Vec<T>, order: &[i64]) -> Vec<T>
where
    for<'a> &'a T: Identifiable<Id = &'a i64>,
{
    order_transformed_as(values, order, |value| *value.id())
}

/// Similar to [`order_as`], but extracts primary key of `values` using `get_id` function.
fn order_transformed_as<V, F>(mut values: Vec<V>, order: &[i64], get_id: F) -> Vec<V>
where
    F: Fn(&V) -> i64,
{
    assert_eq!(values.len(), order.len());

    let mut index = 0;
    while index < order.len() {
        let value_id = get_id(&values[index]);
        let correct_index = order
            .iter()
            .position(|&id| id == value_id)
            .expect("`order` must contain resource id");
        assert!(correct_index >= index, "Value id is not unique");
        if index == correct_index {
            index += 1;
        } else {
            values.swap(index, correct_index);
        }
    }
    values
}

/// Maps a set of resources to a Vec of Options that contains these resources ordered according
/// to the primary keys of `ordered_values`. If there are less `unorderd_values` than `ordered_values`,
/// the missing values are padded as None in the resulting vector.
///
/// The note in the above comment applies here as well.
fn order_like<V, T, F>(unordered_values: Vec<V>, ordered_values: &[T], get_id: F) -> Vec<Option<V>>
where
    for<'a> &'a T: Identifiable<Id = &'a i64>,
    F: Fn(&V) -> i64,
{
    assert!(unordered_values.len() <= ordered_values.len());

    let mut results: Vec<Option<V>> = std::iter::repeat_with(|| None).take(ordered_values.len()).collect();
    for value in unordered_values {
        let value_id = get_id(&value);
        let index = ordered_values
            .iter()
            .position(|ordered_value| *ordered_value.id() == value_id)
            .expect("`ordered_values` must contain resource id");
        results[index] = Some(value);
    }
    results
}

/// Organizes `ordered_names` into a map between each tag's id and its names while
/// minimizing allocations and clones. Assumes `ordered_names` is sorted by tag id
/// and tag name order.
fn collect_names(ordered_names: Vec<(i64, SmallString)>) -> HashMap<i64, Rc<[SmallString]>> {
    if ordered_names.is_empty() {
        return HashMap::new();
    }

    // Mark boundaries where names belong to single resource
    let mut name_boundaries = vec![0];
    for (i, window) in ordered_names.windows(2).enumerate() {
        let curr_id = window[0].0;
        let next_id = window[1].0;
        if next_id != curr_id {
            name_boundaries.push(i + 1);
        }
    }
    name_boundaries.push(ordered_names.len());

    // Collect resource names into names map
    let mut name_iter = ordered_names.into_iter();
    let mut names_map: HashMap<i64, Rc<[SmallString]>> = HashMap::new();
    names_map.reserve(name_boundaries.len() - 1);
    for window in name_boundaries.windows(2) {
        let [start, end] = window.try_into().expect("Window has two elements");
        let name_count = end - start;

        // Create buffer with exact capactiy so that it doesn't need to be reallocated to
        // the correct size when moving to the Rc
        let (id, first_name) = name_iter
            .next()
            .expect("There must be at least one name in a name boundary");
        let mut names = Vec::with_capacity(name_count);
        names.push(first_name);
        for _i in 1..name_count {
            let (_, name) = name_iter
                .next()
                .expect("There are `name_count` names in the name boundary");
            names.push(name);
        }

        names_map.insert(id, Rc::from(names));
    }
    names_map
}
