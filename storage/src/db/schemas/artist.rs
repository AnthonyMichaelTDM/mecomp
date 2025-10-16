#![allow(clippy::module_name_repetitions)]

use super::Id;
#[cfg(not(feature = "db"))]
use super::RecordId;
use std::time::Duration;
#[cfg(feature = "db")]
use surrealdb::RecordId;

pub type ArtistId = RecordId;

pub const TABLE_NAME: &str = "artist";

/// This struct holds all the metadata about a particular [`Artist`].
/// An [`Artist`] is a collection of [`super::album::Album`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "db", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "db", Table("artist"))]
pub struct Artist {
    /// the unique identifier for this [`Artist`].
    #[cfg_attr(feature = "db", field(dt = "record"))]
    pub id: ArtistId,

    /// The [`Artist`]'s name.
    #[cfg_attr(feature = "db", field(dt = "string"))]
    #[cfg_attr(feature = "db", index(unique, text("custom_analyzer")))]
    pub name: String,

    /// Total runtime.
    #[cfg_attr(
        feature = "db",
        field(
            "TYPE any VALUE <future> {
LET $songs = (SELECT id,runtime FROM $this.id->artist_to_song->song);
LET $albums = (SELECT id,runtime FROM $this.id->artist_to_album->album->album_to_song->song);
LET $distinct = array::distinct(array::concat($songs, $albums));
LET $total = $distinct.fold(0s, |$acc, $song| $acc + $song.runtime);
RETURN IF $total IS NONE { 0s } ELSE { $total };
}"
        )
    )]
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "super::serialize_duration_as_sql_duration",
            deserialize_with = "super::deserialize_duration_from_sql_duration"
        )
    )]
    pub runtime: Duration,

    /// the number of albums this artist has.
    #[cfg_attr(
        feature = "db",
        field(
            "TYPE any VALUE <future> { 
LET $count = (SELECT count() FROM $this.id->artist_to_album->album GROUP ALL);
RETURN IF $count IS NONE { 0 } ELSE IF $count.len() == 0 { 0 } ELSE { ($count[0]).count };
}"
        )
    )]
    pub album_count: usize,

    /// the number of songs this artist has.
    ///
    /// This computed field is a bit more complex than the others,
    /// as it needs to count the number of songs in both albums and singles.
    #[cfg_attr(
        feature = "db",
        field(
            "TYPE any VALUE <future> {
LET $songs = (SELECT id FROM $this.id->artist_to_song->song);
LET $albums = (SELECT id FROM $this.id->artist_to_album->album->album_to_song->song);
LET $distinct = array::distinct(array::concat($songs, $albums));
LET $count = count($distinct);
RETURN $count;
} "
        )
    )]
    pub song_count: usize,
}

impl Artist {
    pub const BRIEF_FIELDS: &'static str = "id,name";

    #[must_use]
    #[inline]
    pub fn generate_id() -> ArtistId {
        RecordId::from_table_key(TABLE_NAME, Id::ulid())
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ArtistChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<String>,
}

/// This struct holds all the metadata about a particular [`Artist`].
/// An [`Artist`] is a collection of [`super::album::Album`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtistBrief {
    pub id: ArtistId,
    pub name: String,
}

impl From<Artist> for ArtistBrief {
    #[inline]
    fn from(artist: Artist) -> Self {
        Self {
            id: artist.id,
            name: artist.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::fixture;

    #[fixture]
    fn artist() -> Artist {
        Artist {
            id: RecordId::from((TABLE_NAME, "id")),
            name: "artist".into(),
            runtime: Duration::from_secs(3600),
            album_count: 10,
            song_count: 100,
        }
    }

    #[fixture]
    fn artist_brief() -> ArtistBrief {
        ArtistBrief {
            id: RecordId::from((TABLE_NAME, "id")),
            name: "artist".into(),
        }
    }

    #[test]
    fn test_artist_brief_from_artist() {
        let actual: ArtistBrief = artist().into();
        assert_eq!(actual, artist_brief());
    }
}
