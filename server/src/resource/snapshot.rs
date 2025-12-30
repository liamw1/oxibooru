use crate::config::Config;
use crate::model::enums::{AvatarStyle, ResourceOperation, ResourceType};
use crate::model::snapshot::Snapshot;
use crate::resource;
use crate::resource::BoolFill;
use crate::resource::user::MicroUser;
use crate::schema::{snapshot, user};
use crate::string::SmallString;
use crate::time::DateTime;
use diesel::{ExpressionMethods, Identifiable, PgConnection, QueryDsl, QueryResult, RunQueryDsl};
use serde::Serialize;
use serde_json::Value;
use serde_with::skip_serializing_none;
use server_macros::non_nullable_options;
use strum::{EnumString, EnumTable};
use utoipa::ToSchema;

#[derive(Clone, Copy, EnumString, EnumTable)]
#[strum(serialize_all = "camelCase")]
pub enum Field {
    User,
    Operation,
    Type,
    Id,
    Data,
    Time,
}

impl BoolFill for FieldTable<bool> {
    fn filled(val: bool) -> Self {
        Self::filled(val)
    }
}

/// A snapshot is a version of a database resource.
///
/// **Field meaning**
///
/// - `<operation>`: what happened to the resource.
///
///     The value can be either of values below:
///
///     - `"created"` - the resource has been created
///     - `"modified"` - the resource has been modified
///     - `"deleted"` - the resource has been deleted
///     - `"merged"` - the resource has been merged to another resource
///
/// - `<resource-type>` and `<resource-id>`: the resource that was changed.
///
///     The values are correlated as per table below:
///
///     | `<resource-type>` | `<resource-id>`                  |
///     | ----------------- | -------------------------------  |
///     | `"tag"`           | first tag name at given time     |
///     | `"tag_category"`  | tag category name at given time  |
///     | `"post"`          | post ID                          |
///     | `"pool"`          | pool ID                          |
///     | `"pool_category"` | pool category name at given time |
///
/// - `<issuer>`: a [micro user resource](#micro-user) representing the user who
///     has made the change.
///
/// - `<data>`: the snapshot data, of which content depends on the `<operation>`.
///    More explained later.
///
/// - `<time>`: when the snapshot was created (i.e. when the resource was changed),
///   formatted as per RFC 3339.
///
/// **`<data>` field for creation snapshots**
///
/// The value can be either of structures below, depending on
/// `<resource-type>`:
///
/// - Tag category snapshot data (`<resource-type> = "tag_category"`)
///
///     *Example*
///
///     ```json5
///     {
///         "name":  "character",
///         "color": "#FF0000",
///         "default": false
///     }
///     ```
///
/// - Tag snapshot data (`<resource-type> = "tag"`)
///
///     *Example*
///
///     ```json5
///     {
///         "names":        ["tag1", "tag2", "tag3"],
///         "category":     "plain",
///         "implications": ["imp1", "imp2", "imp3"],
///         "suggestions":  ["sug1", "sug2", "sug3"]
///     }
///     ```
///
/// - Post snapshot data (`<resource-type> = "post"`)
///
///     *Example*
///
///     ```json5
///     {
///         "source": "http://example.com/",
///         "safety": "safe",
///         "checksum": "deadbeef",
///         "tags": ["tag1", "tag2"],
///         "relations": [1, 2],
///         "notes": [<note1>, <note2>, <note3>],
///         "flags": ["loop"],
///         "featured": false
///     }
///     ```
///     
///     `<note>`s are serialized the same way as [note resources](#note).
///
/// - Pool category snapshot data (`<resource-type> = "pool_category"`)
///
///     *Example*
///
///     ```json5
///     {
///         "name":  "collection",
///         "color": "#00FF00",
///         "default": false
///     }
///     ```
///
/// - Pool snapshot data (`<resource-type> = "pool"`)
///
///     *Example*
///
///     ```json5
///     {
///         "names":    ["primes", "primed", "primey"],
///         "category": "mathematical",
///         "posts":    [2, 3, 5, 7, 11, 13, 17]
///     }
///     ```
///
///
/// **`<data>` field for modification snapshots**
///
/// The value is a property-wise recursive diff between previous version of the
/// resource and its current version. Its structure is a `<dictionary-diff>` of
/// dictionaries as created by creation snapshots, which is described below.
///
/// `<primitive>`: any primitive (number or a string)
///
/// `<anything>`: any dictionary, list or primitive
///
/// `<dictionary-diff>`:
///
/// ```json5
/// {
///     "type": "object change",
///     "value":
///     {
///         "property-of-any-type-1":
///         {
///             "type": "deleted property",
///             "value": <anything>
///         },
///         "property-of-any-type-2":
///         {
///             "type": "added property",
///             "value": <anything>
///         },
///         "primitive-property":
///         {
///             "type": "primitive change",
///             "old-value": "<primitive>",
///             "new-value": "<primitive>"
///         },
///         "list-property": <list-diff>,
///         "dictionary-property": <dictionary-diff>
///     }
/// }
/// ```
///
/// `<list-diff>`:
///
/// ```json5
/// {
///     "type": "list change",
///     "removed": [<anything>, <anything>],
///     "added": [<anything>, <anything>]
/// }
/// ```
///
/// Example - a diff for a post that has changed source and has one note added.
/// Note the similarities with the structure of post creation snapshots.
///
/// ```json5
/// {
///     "type": "object change",
///     "value":
///     {
///         "source":
///         {
///             "type": "primitive change",
///             "old-value": None,
///             "new-value": "new source"
///         },
///         "notes":
///         {
///             "type": "list change",
///             "removed": [],
///             "added":
///             [
///                 {"polygon": [[0, 0], [0, 1], [1, 1]], "text": "new note"}
///             ]
///         }
///     }
/// }
/// ```
///
/// Since the snapshot dictionaries structure is pretty immutable, you probably
/// won't see `added property` or `deleted property` around. This observation holds
/// true even if the way the snapshots are generated changes - szurubooru stores
/// just the diffs rather than original snapshots, so it wouldn't be able to
/// generate a diff against an old version.
///
/// **`<data>` field for deletion snapshots**
///
/// Same as creation snapshot. In emergencies, it can be used to reconstruct
/// deleted entities. Please note that this does not constitute as means against
/// vandalism (it's still possible to cause chaos by mass editing - this should be
/// dealt with by configuring role privileges in the config) or replace database
/// backups.
///
/// **`<data>` field for merge snapshots**
///
/// A tuple containing 2 elements:
///
/// - resource type equivalent to `<resource-type>` of the target entity.
/// - resource ID equivalent to `<resource-id>` of the target entity.
#[non_nullable_options]
#[skip_serializing_none]
#[derive(Serialize, ToSchema)]
pub struct SnapshotInfo {
    #[schema(nullable)]
    user: Option<Option<MicroUser>>,
    operation: Option<ResourceOperation>,
    #[serde(rename(serialize = "type"))]
    resource_type: Option<ResourceType>,
    #[serde(rename(serialize = "id"))]
    resource_id: Option<SmallString>,
    data: Option<Value>,
    time: Option<DateTime>,
}

impl SnapshotInfo {
    pub fn new_batch(
        conn: &mut PgConnection,
        config: &Config,
        snapshots: Vec<Snapshot>,
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let batch_size = snapshots.len();
        let mut users = resource::retrieve(fields[Field::User], || get_users(conn, config, &snapshots))?;
        resource::check_batch_results(batch_size, users.len());

        let results = snapshots
            .into_iter()
            .rev()
            .map(|snapshot| Self {
                user: users.pop(),
                operation: fields[Field::Operation].then_some(snapshot.operation),
                resource_type: fields[Field::Type].then_some(snapshot.resource_type),
                resource_id: fields[Field::Id].then_some(snapshot.resource_id),
                data: fields[Field::Data].then_some(snapshot.data),
                time: fields[Field::Time].then_some(snapshot.creation_time),
            })
            .collect::<Vec<_>>();
        Ok(results.into_iter().rev().collect())
    }

    pub fn new_batch_from_ids(
        conn: &mut PgConnection,
        config: &Config,
        snapshot_ids: &[i64],
        fields: &FieldTable<bool>,
    ) -> QueryResult<Vec<Self>> {
        let unordered_snapshots = snapshot::table.filter(snapshot::id.eq_any(snapshot_ids)).load(conn)?;
        let snapshots = resource::order_as(unordered_snapshots, snapshot_ids);
        Self::new_batch(conn, config, snapshots, fields)
    }
}

fn get_users(conn: &mut PgConnection, config: &Config, snapshots: &[Snapshot]) -> QueryResult<Vec<Option<MicroUser>>> {
    let snapshot_ids: Vec<_> = snapshots.iter().map(Identifiable::id).collect();
    snapshot::table
        .inner_join(user::table)
        .select((snapshot::id, user::name, user::avatar_style))
        .filter(snapshot::id.eq_any(snapshot_ids))
        .load::<(i64, SmallString, AvatarStyle)>(conn)
        .map(|user_info| {
            resource::order_like(user_info, snapshots, |&(id, ..)| id)
                .into_iter()
                .map(|user_info| user_info.map(|(_, name, avatar_style)| MicroUser::new(config, name, avatar_style)))
                .collect()
        })
}
