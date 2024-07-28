pub mod comment;
pub mod pool;
pub mod pool_category;
pub mod post;
pub mod tag;
pub mod tag_category;
pub mod user;
pub mod user_token;

use crate::model::IntegerIdentifiable;

fn check_batch_results(batch_size: usize, post_count: usize) {
    assert!(batch_size == 0 || batch_size == post_count);
}

/*
    This algorithm is O(n^2) in post_ids.len(), which could be made O(n) with a HashMap implementation.
    However, for small n this Vec-based implementation is probably much faster. Since we retrieve
    40-50 posts at a time, I'm leaving it like this for the time being until it proves to be slow.
*/
fn order_by<T: IntegerIdentifiable>(mut values: Vec<T>, order: &[i32]) -> Vec<T> {
    let mut index = 0;
    while index < order.len() {
        let value_id = values[index].id();
        let correct_index = order.iter().position(|&id| id == value_id).unwrap();
        if index != correct_index {
            values.swap(index, correct_index);
        } else {
            index += 1;
        }
    }
    values
}

fn order_as<V, T, F>(unordered_values: Vec<V>, ordered_values: &[T], get_key: F) -> Vec<Option<V>>
where
    T: IntegerIdentifiable,
    F: Fn(&V) -> i32,
{
    assert!(unordered_values.len() <= ordered_values.len());

    let mut results: Vec<Option<V>> = std::iter::repeat_with(|| None).take(ordered_values.len()).collect();
    for value in unordered_values.into_iter() {
        let value_id = get_key(&value);
        let index = ordered_values
            .iter()
            .position(|ordered_value| ordered_value.id() == value_id)
            .unwrap();
        results[index] = Some(value);
    }
    results
}
