use crate::api::ApiResult;
use crate::auth::Client;
use crate::config::RegexType;
use crate::model::enums::ResourceType;
use crate::model::post::PostTag;
use crate::model::tag::{NewTag, NewTagName, TagImplication, TagName, TagSuggestion};
use crate::schema::post_tag;
use crate::schema::{tag, tag_implication, tag_name, tag_suggestion};
use crate::string::SmallString;
use crate::time::DateTime;
use crate::{api, config, snapshot};
use diesel::dsl::max;
use diesel::prelude::*;
use std::collections::hash_map::{Entry, IntoKeys};
use std::collections::{HashMap, HashSet};

/// Updates `last_edit_time` of tag with given `tag_id`.
pub fn last_edit_time(conn: &mut PgConnection, tag_id: i64) -> ApiResult<()> {
    diesel::update(tag::table.find(tag_id))
        .set(tag::last_edit_time.eq(DateTime::now()))
        .execute(conn)?;
    Ok(())
}

/// Appends `names` onto the current list of names for the tag with id `tag_id`.
pub fn add_names(
    conn: &mut PgConnection,
    tag_id: i64,
    current_name_count: i32,
    names: &[SmallString],
) -> ApiResult<()> {
    names
        .iter()
        .try_for_each(|name| api::verify_matches_regex(name, RegexType::Tag))?;

    let new_names: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (current_name_count + i as i32, name))
        .map(|(order, name)| NewTagName { tag_id, order, name })
        .collect();
    new_names.insert_into(tag_name::table).execute(conn)?;
    Ok(())
}

/// Deletes all names for the tag with id `tag_id`.
/// Returns number of names deleted.
pub fn delete_names(conn: &mut PgConnection, tag_id: i64) -> QueryResult<usize> {
    diesel::delete(tag_name::table)
        .filter(tag_name::tag_id.eq(tag_id))
        .execute(conn)
}

/// Adds `implied_ids` to the list of implications for the tag with id `tag_id`.
pub fn add_implications(conn: &mut PgConnection, tag_id: i64, implied_ids: &[i64]) -> ApiResult<()> {
    let new_implications: Vec<_> = implied_ids
        .iter()
        .map(|&child_id| {
            (tag_id != child_id)
                .then_some(TagImplication {
                    parent_id: tag_id,
                    child_id,
                })
                .ok_or(api::Error::CyclicDependency(ResourceType::TagImplication))
        })
        .collect::<Result<_, _>>()?;
    new_implications.insert_into(tag_implication::table).execute(conn)?;
    Ok(())
}

/// Adds `suggested_ids` to the list of suggestions for the tag with id `tag_id`.
pub fn add_suggestions(conn: &mut PgConnection, tag_id: i64, suggested_ids: &[i64]) -> ApiResult<()> {
    let new_suggestions: Vec<_> = suggested_ids
        .iter()
        .map(|&child_id| {
            (tag_id != child_id)
                .then_some(TagSuggestion {
                    parent_id: tag_id,
                    child_id,
                })
                .ok_or(api::Error::CyclicDependency(ResourceType::TagSuggestion))
        })
        .collect::<Result<_, _>>()?;
    new_suggestions.insert_into(tag_suggestion::table).execute(conn)?;
    Ok(())
}

/// Returns all tag ids implied from the given set of names.
/// Returned ids will be distinct.
///
/// Requires tag creation privileges if new names are given.
/// Checks that each new name matches on the Tag regex.
pub fn get_or_create_tag_ids(
    conn: &mut PgConnection,
    client: Client,
    names: Vec<SmallString>,
    detect_cycles: bool,
) -> ApiResult<(Vec<i64>, Vec<SmallString>)> {
    let mut implied_ids: Vec<i64> = tag_name::table
        .select(tag_name::tag_id)
        .filter(tag_name::name.eq_any(&names))
        .distinct()
        .load(conn)?;

    // Collect implied tag ids and build dependency graph
    let mut dependency_graph = DependencyGraph::new(&implied_ids);
    loop {
        let implications: Vec<TagImplication> = tag_implication::table
            .filter(tag_implication::parent_id.eq_any(&implied_ids))
            .distinct()
            .load(conn)?;

        // Remove ids we've already seen
        let previous_len = dependency_graph.len();
        implied_ids = implications
            .into_iter()
            .filter(|&implication| dependency_graph.insert(implication))
            .map(|implication| implication.child_id)
            .collect();

        if dependency_graph.len() == previous_len {
            break;
        }
    }
    if detect_cycles && dependency_graph.has_cycle() {
        return Err(api::Error::CyclicDependency(ResourceType::TagImplication));
    }

    let mut tag_ids: Vec<_> = dependency_graph.into_nodes().collect();
    let existing_names: HashSet<SmallString> = tag_name::table
        .select(tag_name::name)
        .filter(tag_name::tag_id.eq_any(&tag_ids))
        .load(conn)?
        .into_iter()
        .map(|name: SmallString| name.to_lowercase())
        .collect();

    let new_names: Vec<_> = names
        .into_iter()
        .filter(|name| !existing_names.contains(&name.to_lowercase()))
        .collect();
    new_names
        .iter()
        .try_for_each(|name| api::verify_matches_regex(name, RegexType::Tag))?;

    // Create new tags if given unique names
    if !new_names.is_empty() {
        api::verify_privilege(client, config::privileges().tag_create)?;

        let new_tag_ids: Vec<i64> = vec![NewTag::default(); new_names.len()]
            .insert_into(tag::table)
            .returning(tag::id)
            .get_results(conn)?;
        let new_tag_names: Vec<_> = new_tag_ids
            .iter()
            .zip(new_names.iter())
            .map(|(&tag_id, name)| NewTagName { tag_id, order: 0, name })
            .collect();
        new_tag_names.insert_into(tag_name::table).execute(conn)?;

        snapshot::tag::new_name_snapshots(conn, client, new_names)?;
        tag_ids.extend(new_tag_ids);
    }

    let primary_tag_names = tag_name::table
        .select(tag_name::name)
        .filter(tag_name::tag_id.eq_any(&tag_ids))
        .filter(TagName::primary())
        .load(conn)?;
    Ok((tag_ids, primary_tag_names))
}

pub fn merge(conn: &mut PgConnection, absorbed_id: i64, merge_to_id: i64) -> ApiResult<()> {
    // Merge implications
    let involved_implications: Vec<TagImplication> = tag_implication::table
        .filter(tag_implication::parent_id.eq(absorbed_id))
        .or_filter(tag_implication::child_id.eq(absorbed_id))
        .or_filter(tag_implication::parent_id.eq(merge_to_id))
        .or_filter(tag_implication::child_id.eq(merge_to_id))
        .load(conn)?;
    let merged_implications: HashSet<_> = involved_implications
        .iter()
        .copied()
        .map(|mut implication| {
            if implication.parent_id == absorbed_id {
                implication.parent_id = merge_to_id;
            } else if implication.child_id == absorbed_id {
                implication.child_id = merge_to_id;
            }
            implication
        })
        .filter(|implication| implication.parent_id != implication.child_id)
        .collect();
    diesel::delete(tag_implication::table)
        .filter(tag_implication::parent_id.eq(merge_to_id))
        .or_filter(tag_implication::child_id.eq(merge_to_id))
        .execute(conn)?;
    let merged_implications: Vec<_> = merged_implications.into_iter().collect();
    merged_implications.insert_into(tag_implication::table).execute(conn)?;

    // Merge suggestions
    let involved_suggestions: Vec<TagSuggestion> = tag_suggestion::table
        .filter(tag_suggestion::parent_id.eq(absorbed_id))
        .or_filter(tag_suggestion::child_id.eq(absorbed_id))
        .or_filter(tag_suggestion::parent_id.eq(merge_to_id))
        .or_filter(tag_suggestion::child_id.eq(merge_to_id))
        .load(conn)?;
    let merged_suggestions: HashSet<_> = involved_suggestions
        .iter()
        .copied()
        .map(|mut suggestion| {
            if suggestion.parent_id == absorbed_id {
                suggestion.parent_id = merge_to_id;
            } else if suggestion.child_id == absorbed_id {
                suggestion.child_id = merge_to_id;
            }
            suggestion
        })
        .filter(|suggestion| suggestion.parent_id != suggestion.child_id)
        .collect();
    diesel::delete(tag_suggestion::table)
        .filter(tag_suggestion::parent_id.eq(merge_to_id))
        .or_filter(tag_suggestion::child_id.eq(merge_to_id))
        .execute(conn)?;
    let merged_suggestions: Vec<_> = merged_suggestions.into_iter().collect();
    merged_suggestions.insert_into(tag_suggestion::table).execute(conn)?;

    // Merge usages
    let merge_to_posts = post_tag::table
        .select(post_tag::post_id)
        .filter(post_tag::tag_id.eq(merge_to_id))
        .into_boxed();
    let new_post_tags: Vec<_> = post_tag::table
        .select(post_tag::post_id)
        .filter(post_tag::tag_id.eq(absorbed_id))
        .filter(post_tag::post_id.ne_all(merge_to_posts))
        .load(conn)?
        .into_iter()
        .map(|post_id| PostTag {
            post_id,
            tag_id: merge_to_id,
        })
        .collect();
    new_post_tags.insert_into(post_tag::table).execute(conn)?;

    // Merge names
    let current_name_count = tag_name::table
        .select(max(tag_name::order) + 1)
        .filter(tag_name::tag_id.eq(merge_to_id))
        .first::<Option<_>>(conn)?
        .unwrap_or(0);
    let removed_names = diesel::delete(tag_name::table.filter(tag_name::tag_id.eq(absorbed_id)))
        .returning(tag_name::name)
        .get_results(conn)?;
    add_names(conn, merge_to_id, current_name_count, &removed_names)?;

    diesel::delete(tag::table.find(absorbed_id)).execute(conn)?;
    last_edit_time(conn, merge_to_id)
}

enum TraversalState {
    Visited,
    Explored,
}

struct DependencyGraph {
    // Maps children to parents
    nodes: HashMap<i64, HashSet<i64>>,
}

impl DependencyGraph {
    fn new(starting_nodes: &[i64]) -> Self {
        Self {
            nodes: starting_nodes.iter().map(|&tag_id| (tag_id, HashSet::new())).collect(),
        }
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }

    fn insert(&mut self, implication: TagImplication) -> bool {
        let parents = self.nodes.entry(implication.child_id).or_default();
        let new_node = parents.is_empty();
        parents.insert(implication.parent_id);
        new_node
    }

    fn into_nodes(self) -> IntoKeys<i64, HashSet<i64>> {
        self.nodes.into_keys()
    }

    /// Determines if depedency graph has a cycle using a depth-first search approach.
    /// Runs in O(V + E) time where V is the number of vertices and E is the number of edges of the graph.
    fn has_cycle(&self) -> bool {
        let mut traversed_nodes = HashMap::new();
        for (base_node_id, parents) in &self.nodes {
            let mut traversal_stack = vec![(*base_node_id, parents.iter())];
            while let Some((current_node_id, parents)) = traversal_stack.last_mut() {
                let Some(next_node_id) = parents.next() else {
                    // Once all parents have been traversed, mark node as explored
                    traversed_nodes.insert(*current_node_id, TraversalState::Explored);
                    traversal_stack.pop();
                    continue;
                };

                match traversed_nodes.entry(*next_node_id) {
                    Entry::Occupied(entry) => match entry.get() {
                        TraversalState::Visited => return true,
                        TraversalState::Explored => continue,
                    },
                    Entry::Vacant(entry) => entry.insert(TraversalState::Visited),
                };

                if let Some(parents) = self.nodes.get(next_node_id) {
                    traversal_stack.push((*next_node_id, parents.iter()));
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::tag::TagImplication;

    #[test]
    fn trivial_graphs() {
        let empty_graph = DependencyGraph::new(&[]);
        assert!(!empty_graph.has_cycle());

        let single_node = DependencyGraph::new(&[0]);
        assert!(!single_node.has_cycle());

        let mut trivial_cycle = DependencyGraph::new(&[0]);
        trivial_cycle.insert(TagImplication {
            parent_id: 0,
            child_id: 0,
        });
        assert!(trivial_cycle.has_cycle());

        let mut single_link = DependencyGraph::new(&[0, 1]);
        single_link.insert(TagImplication {
            parent_id: 1,
            child_id: 0,
        });
        assert!(!single_link.has_cycle());

        single_link.insert(TagImplication {
            parent_id: 0,
            child_id: 1,
        });
        assert!(single_link.has_cycle());
    }

    #[test]
    fn dependency_graph_with_cycle() {
        let mut graph = DependencyGraph::new(&[0]);
        let links = [(0, 1), (1, 2), (1, 3), (3, 4), (5, 6), (5, 7), (6, 8), (6, 4), (4, 3)];
        for (parent_id, child_id) in links {
            graph.insert(TagImplication { parent_id, child_id });
        }

        assert!(graph.has_cycle());
    }

    #[test]
    fn dependency_graph_without_cycle() {
        let mut graph = DependencyGraph::new(&[0]);
        let links = [
            (0, 1),
            (0, 2),
            (0, 3),
            (2, 5),
            (3, 4),
            (3, 9),
            (4, 5),
            (5, 6),
            (5, 7),
            (6, 8),
            (7, 8),
            (8, 9),
            (9, 10),
        ];
        for (parent_id, child_id) in links {
            graph.insert(TagImplication { parent_id, child_id });
        }

        assert!(!graph.has_cycle());
    }
}
