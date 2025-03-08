#![allow(clippy::module_name_repetitions)]

#[cfg(not(feature = "db"))]
use super::{Id, Thing};
use std::time::Duration;
#[cfg(feature = "db")]
use surrealdb::sql::{Id, Thing};

pub type ArtistId = Thing;

pub const TABLE_NAME: &str = "artist";

/// This struct holds all the metadata about a particular [`Artist`].
/// An [`Artist`] is a collection of [`super::album::Album`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "db", derive(surrealqlx::Table))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "db", Table("artist"))]
pub struct Artist {
    /// the unique identifier for this [`Artist`].
    #[cfg_attr(feature = "db", field("any"))]
    pub id: ArtistId,

    /// The [`Artist`]'s name.
    #[cfg_attr(
        feature = "db",
        field(dt = "string", index(unique), index(text("custom_analyzer")))
    )]
    pub name: String,

    /// Total runtime.
    #[cfg_attr(feature = "db", field(dt = "duration"))]
    #[cfg_attr(
        feature = "db",
        serde(
            serialize_with = "super::serialize_duration_as_sql_duration",
            deserialize_with = "super::deserialize_duration_from_sql_duration"
        )
    )]
    pub runtime: Duration,

    /// the number of albums this artist has.
    #[cfg_attr(feature = "db", field(dt = "int"))]
    pub album_count: usize,

    /// the number of songs this artist has.
    #[cfg_attr(feature = "db", field(dt = "int"))]
    pub song_count: usize,
}

impl Artist {
    #[must_use]
    #[inline]
    pub fn generate_id() -> ArtistId {
        Thing::from((TABLE_NAME, Id::ulid()))
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ArtistChangeSet {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub name: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(
        feature = "db",
        serde(serialize_with = "super::serialize_duration_option_as_sql_duration")
    )]
    pub runtime: Option<Duration>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub album_count: Option<usize>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub song_count: Option<usize>,
}

/// This struct holds all the metadata about a particular [`Artist`].
/// An [`Artist`] is a collection of [`super::album::Album`]s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtistBrief {
    pub id: ArtistId,
    pub name: String,
    pub runtime: std::time::Duration,
    pub albums: usize,
    pub songs: usize,
}

impl From<Artist> for ArtistBrief {
    #[inline]
    fn from(artist: Artist) -> Self {
        Self {
            id: artist.id,
            name: artist.name,
            runtime: artist.runtime,
            albums: artist.album_count,
            songs: artist.song_count,
        }
    }
}

impl From<&Artist> for ArtistBrief {
    #[inline]
    fn from(artist: &Artist) -> Self {
        Self {
            id: artist.id.clone(),
            name: artist.name.clone(),
            runtime: artist.runtime,
            albums: artist.album_count,
            songs: artist.song_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    #[fixture]
    fn artist() -> Artist {
        Artist {
            id: Thing::from((TABLE_NAME, "id")),
            name: "artist".into(),
            runtime: Duration::from_secs(3600),
            album_count: 10,
            song_count: 100,
        }
    }

    #[fixture]
    fn artist_brief() -> ArtistBrief {
        ArtistBrief {
            id: Thing::from((TABLE_NAME, "id")),
            name: "artist".into(),
            runtime: Duration::from_secs(3600),
            albums: 10,
            songs: 100,
        }
    }

    #[rstest]
    #[case(artist(), artist_brief())]
    #[case(&artist(), artist_brief())]
    fn test_artist_brief_from_artist<T: Into<ArtistBrief>>(
        #[case] artist: T,
        #[case] brief: ArtistBrief,
    ) {
        let actual: ArtistBrief = artist.into();
        assert_eq!(actual, brief);
    }
}
