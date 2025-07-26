use crate::content::signature::{COMPRESSED_SIGNATURE_LEN, NUM_WORDS};
use crate::model::enums::{MimeType, PostFlags, PostSafety, PostType, Score};
use crate::model::tag::Tag;
use crate::model::user::User;
use crate::schema::{
    post, post_favorite, post_feature, post_note, post_relation, post_score, post_signature, post_tag,
};
use crate::time::DateTime;
use byteorder::{NetworkEndian, ReadBytesExt};
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::{Array, BigInt, Bytea, Integer, Nullable};
use diesel::{AsExpression, FromSqlRow};
use std::ops::Deref;
use std::str::FromStr;

#[derive(Debug, AsExpression)]
#[diesel(sql_type = Bytea)]
pub struct Checksum(Vec<u8>);

impl FromStr for Checksum {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        hex::decode(s).map(Self)
    }
}

impl ToSql<Bytea, Pg> for Checksum {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <[u8] as ToSql<Bytea, Pg>>::to_sql(&self.0, out)
    }
}

#[derive(Insertable)]
#[diesel(table_name = post)]
#[diesel(check_for_backend(Pg))]
pub struct NewPost<'a> {
    pub user_id: Option<i64>,
    pub file_size: i64,
    pub width: i32,
    pub height: i32,
    pub safety: PostSafety,
    pub type_: PostType,
    pub mime_type: MimeType,
    pub checksum: &'a [u8],
    pub checksum_md5: &'a [u8],
    pub flags: PostFlags,
    pub source: &'a str,
    pub description: &'a str,
}

#[derive(Clone, AsChangeset, Associations, Identifiable, Queryable, Selectable)]
#[diesel(treat_none_as_null = true)]
#[diesel(belongs_to(User))]
#[diesel(table_name = post)]
#[diesel(check_for_backend(Pg))]
pub struct Post {
    pub id: i64,
    pub user_id: Option<i64>,
    pub file_size: i64,
    pub width: i32,
    pub height: i32,
    pub safety: PostSafety,
    pub type_: PostType,
    pub mime_type: MimeType,
    pub checksum: Vec<u8>,
    pub checksum_md5: Vec<u8>,
    pub flags: PostFlags,
    pub source: String,
    pub creation_time: DateTime,
    pub last_edit_time: DateTime,
    pub generated_thumbnail_size: i64,
    pub custom_thumbnail_size: i64,
    pub description: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post, foreign_key = parent_id))]
#[diesel(table_name = post_relation)]
#[diesel(primary_key(parent_id, child_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostRelation {
    pub parent_id: i64,
    pub child_id: i64,
}

diesel::joinable!(post_relation -> post (parent_id));

impl PostRelation {
    pub fn new_pair(id_1: i64, id_2: i64) -> [Self; 2] {
        [PostRelation::new(id_1, id_2), PostRelation::new(id_2, id_1)]
    }

    fn new(parent_id: i64, child_id: i64) -> Self {
        Self { parent_id, child_id }
    }
}

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(Tag))]
#[diesel(table_name = post_tag)]
#[diesel(primary_key(post_id, tag_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostTag {
    pub post_id: i64,
    pub tag_id: i64,
}

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_favorite)]
#[diesel(primary_key(post_id, user_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostFavorite {
    pub post_id: i64,
    pub user_id: i64,
    pub time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = post_feature)]
#[diesel(check_for_backend(Pg))]
pub struct NewPostFeature {
    pub post_id: i64,
    pub user_id: i64,
    pub time: DateTime,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_feature)]
#[diesel(check_for_backend(Pg))]
pub struct PostFeature {
    pub id: i64,
    pub post_id: i64,
    pub user_id: i64,
    pub time: DateTime,
}

#[derive(Insertable)]
#[diesel(table_name = post_note)]
#[diesel(check_for_backend(Pg))]
pub struct NewPostNote<'a> {
    pub post_id: i64,
    pub polygon: &'a [f32],
    pub text: &'a str,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post))]
#[diesel(table_name = post_note)]
#[diesel(check_for_backend(Pg))]
pub struct PostNote {
    pub id: i64,
    pub post_id: i64,
    pub polygon: Vec<Option<f32>>,
    pub text: String,
}

#[derive(Associations, Identifiable, Insertable, Queryable, Selectable)]
#[diesel(belongs_to(Post), belongs_to(User))]
#[diesel(table_name = post_score)]
#[diesel(primary_key(post_id, user_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostScore {
    pub post_id: i64,
    pub user_id: i64,
    pub score: Score,
    pub time: DateTime,
}

#[derive(Debug, AsExpression, FromSqlRow)]
#[diesel(sql_type = Array<Nullable<BigInt>>)]
pub struct CompressedSignature([i64; COMPRESSED_SIGNATURE_LEN]);

impl ToSql<Array<Nullable<BigInt>>, Pg> for CompressedSignature {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <[i64] as ToSql<Array<BigInt>, Pg>>::to_sql(self.0.as_slice(), out)
    }
}

impl FromSql<Array<Nullable<BigInt>>, Pg> for CompressedSignature {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        deserialize_array(value).map(Self)
    }
}

impl Deref for CompressedSignature {
    type Target = [i64; COMPRESSED_SIGNATURE_LEN];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<[i64; COMPRESSED_SIGNATURE_LEN]> for CompressedSignature {
    fn from(value: [i64; COMPRESSED_SIGNATURE_LEN]) -> Self {
        Self(value)
    }
}

#[derive(Debug, AsExpression, FromSqlRow)]
#[diesel(sql_type = Array<Nullable<Integer>>)]
pub struct SignatureIndexes([i32; NUM_WORDS]);

impl ToSql<Array<Nullable<Integer>>, Pg> for SignatureIndexes {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        <[i32] as ToSql<Array<Integer>, Pg>>::to_sql(self.0.as_slice(), out)
    }
}

impl FromSql<Array<Nullable<Integer>>, Pg> for SignatureIndexes {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        deserialize_array(value).map(Self)
    }
}

impl From<[i32; NUM_WORDS]> for SignatureIndexes {
    fn from(value: [i32; NUM_WORDS]) -> Self {
        Self(value)
    }
}

#[derive(Insertable)]
#[diesel(table_name = post_signature)]
#[diesel(check_for_backend(Pg))]
pub struct NewPostSignature {
    pub post_id: i64,
    pub signature: CompressedSignature,
    pub words: SignatureIndexes,
}

#[derive(Associations, Identifiable, Queryable, Selectable)]
#[diesel(belongs_to(Post))]
#[diesel(table_name = post_signature)]
#[diesel(primary_key(post_id))]
#[diesel(check_for_backend(Pg))]
pub struct PostSignature {
    pub post_id: i64,
    pub signature: CompressedSignature,
}

impl PostSignature {
    pub fn find_similar_candidates(conn: &mut PgConnection, words: [i32; NUM_WORDS]) -> QueryResult<Vec<Self>> {
        conn.transaction(|conn| {
            // Postgres really wants to perform a seq scan here, which is much slower than
            // an index scan. We temporarily disable seq scans to force it to use the index scan.
            diesel::sql_query("SET LOCAL enable_seqscan=false").execute(conn)?;
            post_signature::table
                .select(PostSignature::as_select())
                .filter(post_signature::words.overlaps_with(words.as_slice()))
                .load(conn)
        })
    }
}

/// Deserializes a database query `value` into a fixed-size array of length `N`.
///
/// Implementation adapted from `Vec<T>::from_sql<Array<ST>>`.
fn deserialize_array<T, const N: usize, A>(value: PgValue<'_>) -> deserialize::Result<[T; N]>
where
    T: Copy + Default + FromSql<A, Pg>,
{
    let mut bytes = value.as_bytes();

    let num_dimensions = bytes.read_i32::<NetworkEndian>()?;
    match num_dimensions {
        0 => return Err("array was zero dimensional".into()),
        1 => (),
        _ => return Err("multi-dimensional arrays are not supported".into()),
    }

    let has_null = bytes.read_i32::<NetworkEndian>()? != 0;
    if has_null {
        return Err("found NULL value".into());
    }

    let _oid = bytes.read_i32::<NetworkEndian>()?;
    let num_elements = bytes.read_i32::<NetworkEndian>()?;
    let _lower_bound = bytes.read_i32::<NetworkEndian>()?;
    if num_elements as usize != N {
        return Err(format!("expected array of length {N} but found array of length {num_elements}").into());
    }

    let mut deserialized_array = [T::default(); N];
    for element in deserialized_array.iter_mut() {
        let elem_size = bytes.read_i32::<NetworkEndian>()?;
        let (elem_bytes, new_bytes) = bytes.split_at(elem_size.try_into()?);
        bytes = new_bytes;
        *element = T::from_sql(PgValue::new(elem_bytes, &value))?;
    }
    Ok(deserialized_array)
}
