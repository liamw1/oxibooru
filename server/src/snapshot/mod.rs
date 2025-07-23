use serde_json::{Map, Value};

pub mod tag;
pub mod tag_category;

pub fn value_diff(old: Value, new: Value) -> Option<Value> {
    match (old, new) {
        (Value::Array(old_array), Value::Array(new_array)) => array_diff(old_array, new_array),
        (Value::Object(old_object), Value::Object(new_object)) => object_diff(old_object, new_object),
        (old, new) => (old != new).then(|| {
            let is_primitive_change = value_index(&old) == value_index(&new) || old.is_null() || new.is_null();
            let change_name = if is_primitive_change {
                "primitive change"
            } else {
                "property changed type"
            };
            let change_type = ("type".into(), change_name.into());
            let old = ("old-value".into(), old);
            let new = ("new-value".into(), new);
            let change = [change_type, old, new].into_iter().collect();
            Value::Object(change)
        }),
    }
}

fn value_index(value: &Value) -> usize {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

/// For large arrays, it may be more efficient to clone everything to two
/// hashmaps and perform O(1) searches.
fn array_diff(old: Vec<Value>, new: Vec<Value>) -> Option<Value> {
    let removed: Vec<_> = old.iter().filter(|item| !new.contains(item)).cloned().collect();
    let added: Vec<_> = new.into_iter().filter(|item| !old.contains(item)).collect();

    (!added.is_empty() || !removed.is_empty()).then(|| {
        let change_type = ("type".into(), "list change".into());
        let items_added = ("added".into(), added.into());
        let items_removed = ("removed".into(), removed.into());
        let change = [change_type, items_added, items_removed].into_iter().collect();
        Value::Object(change)
    })
}

fn object_diff(old: Map<String, Value>, mut new: Map<String, Value>) -> Option<Value> {
    let mut diff = Map::new();

    // Check for keys added in new object
    let added_keys: Vec<_> = new
        .iter()
        .filter_map(|(key, _)| new.contains_key(key).then(|| key.clone()))
        .collect();
    let added_properties: Vec<_> = added_keys.iter().filter_map(|key| new.remove_entry(key)).collect();

    // Check for keys present in old object
    for (key, old_value) in old {
        let new_value = match new.remove(&key) {
            Some(new_value) => new_value,
            None => {
                // Property deleted
                let change_type = ("type".into(), "deleted property".into());
                let value_deleted = ("value".into(), old_value);
                let change = [change_type, value_deleted].into_iter().collect();
                diff.insert(key, Value::Object(change));
                continue;
            }
        };

        // Check if property changed
        if let Some(change) = value_diff(old_value, new_value) {
            diff.insert(key, change);
        }
    }

    for (key, new_value) in added_properties {
        let change_type = ("type".into(), "added_property".into());
        let value_added = ("value".into(), new_value);
        let change = [change_type, value_added].into_iter().collect();
        diff.insert(key, Value::Object(change));
    }

    (!diff.is_empty()).then(|| {
        let change_type = ("type".into(), "object change".into());
        let difference = ("value".into(), Value::Object(diff));
        let change = [change_type, difference].into_iter().collect();
        Value::Object(change)
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_diff() {
        let old = json!({
            "source": null,
            "notes": []
        });
        let new = json!({
            "source": "new source",
            "notes": [{
                "polygon": [[0, 0], [0, 1], [1, 1]],
                "text": "new note"
            }]
        });
        let expected_diff = json!({
            "type": "object change",
            "value":
            {
                "source":
                {
                    "type": "primitive change",
                    "old-value": null,
                    "new-value": "new source"
                },
                "notes":
                {
                    "type": "list change",
                    "removed": [],
                    "added": [{
                        "polygon": [[0, 0], [0, 1], [1, 1]], "text": "new note"
                    }]
                }
            }
        });

        assert_eq!(value_diff(old, new), Some(expected_diff));
    }
}
