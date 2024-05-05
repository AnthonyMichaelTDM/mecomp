pub mod album;
pub mod artist;

#[cfg(test)]
macro_rules! query_test {
    ($test_name:ident,$mod:ident, $name:ident, $expected:expr) => {
        #[test]
        fn $test_name() {
            use ::surrealdb::opt::IntoQuery;
            ::pretty_assertions::assert_eq!(
                $mod::$name().into_query().unwrap(),
                $expected.into_query().unwrap()
            );
        }
    };
    ($mod:ident :: $name:ident, $expected:expr) => {
        query_test!($name, $mod, $name, $expected);
    };
}

#[cfg(test)]
mod album_tests {
    use super::*;

    query_test!(album::read_by_name, "SELECT * FROM album WHERE title=$name");
    query_test!(
        album::read_by_name_and_album_artist,
        "SELECT * FROM album WHERE title=$title AND artist=$artist"
    );
    query_test!(
        album::relate_album_to_songs,
        "RELATE $album->album_to_song->$songs"
    );
    query_test!(
        album::read_songs_in_album,
        "SELECT * FROM $album->album_to_song.out"
    );
    query_test!(
        album::remove_songs_from_album,
        "DELETE $album->album_to_song WHERE out IN $songs"
    );
    query_test!(
        album::read_artist_of_album,
        "SELECT * FROM $id<-artist_to_album<-artist"
    );
}

#[cfg(test)]
mod artist_tests {
    use super::*;

    query_test!(
        artist::read_by_name,
        "SELECT * FROM artist WHERE name = $name LIMIT 1"
    );

    query_test!(
        artist::read_by_names,
        "SELECT * FROM artist WHERE name IN $names"
    );

    query_test!(artist::read_many, "SELECT * FROM $ids");

    query_test!(
        artist::read_albums_by_artist,
        "SELECT * FROM $id->artist_to_album->album"
    );

    query_test!(
        artist::relate_artist_to_album,
        "RELATE $id->artist_to_album->$album"
    );

    query_test!(
        artist::relate_artists_to_album,
        "RELATE $ids->artist_to_album->$album"
    );

    query_test!(
        artist::relate_artist_to_songs,
        "RELATE $id->artist_to_song->$songs"
    );

    query_test!(
        artist::remove_songs_from_artist,
        "DELETE $artist->artist_to_song WHERE out IN $songs"
    );

    query_test!(
        artist::read_songs_by_artist,
        "RETURN array::union((SELECT * FROM $artist->artist_to_song->song), (SELECT * FROM $artist->artist_to_album->album->album_to_song->song))"
    );
}
