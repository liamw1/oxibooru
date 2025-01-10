pub mod comment;
pub mod pool;
pub mod pool_category;
pub mod post;
pub mod tag;
pub mod tag_category;
pub mod user;
pub mod user_token;

use diesel::prelude::*;

// NOTE: The more complicated queries in this module rely on the behavior of diesel's
// grouped_by function preserving the relative order between elements. This seems to be the
// case, and the most straightforward way of implementing the function would have this behavior,
// but I don't see this as a guarantee anywhere in the documentation. If this changes, I'll need
// to reimplement a similar function with this behavior.

fn check_batch_results(batch_size: usize, post_count: usize) {
    assert!(batch_size == 0 || batch_size == post_count);
}

/// For a given set of resources, orders them so that their primary keys are in the same order
/// as the order slice, which should be the same length as values.
///
/// NOTE: This algorithm is O(n^2) in values.len(), which could be made O(n) with a HashMap implementation.
/// However, for small n this Vec-based implementation is probably much faster. Since we retrieve
/// 40-50 resources at a time, I'm leaving it like this for the time being until it proves to be slow.
fn order_as<T>(values: Vec<T>, order: &[i32]) -> Vec<T>
where
    for<'a> &'a T: Identifiable<Id = &'a i32>,
{
    order_transformed_as(values, order, |value| *value.id())
}

fn order_transformed_as<V, F>(mut values: Vec<V>, order: &[i32], get_id: F) -> Vec<V>
where
    F: Fn(&V) -> i32,
{
    assert_eq!(values.len(), order.len());

    let mut index = 0;
    while index < order.len() {
        let value_id = get_id(&values[index]);
        let correct_index = order.iter().position(|&id| id == value_id).unwrap();
        assert!(correct_index >= index, "Value id is not unique");
        if index != correct_index {
            values.swap(index, correct_index);
        } else {
            index += 1;
        }
    }
    values
}

/// Maps a set of resources to a Vec of Options that contains these resources ordered according
/// to the primary keys of ordered_values. If there are less unorderd_values than ordered_values,
/// the missing values are padded as None in the resulting vector.
///
/// The note in the above comment applies here as well.
fn order_like<V, T, F>(unordered_values: Vec<V>, ordered_values: &[T], get_id: F) -> Vec<Option<V>>
where
    for<'a> &'a T: Identifiable<Id = &'a i32>,
    F: Fn(&V) -> i32,
{
    assert!(unordered_values.len() <= ordered_values.len());

    let mut results: Vec<Option<V>> = std::iter::repeat_with(|| None).take(ordered_values.len()).collect();
    for value in unordered_values.into_iter() {
        let value_id = get_id(&value);
        let index = ordered_values
            .iter()
            .position(|ordered_value| *ordered_value.id() == value_id)
            .unwrap();
        results[index] = Some(value);
    }
    results
}
