pub mod album;
pub mod artist;
pub mod collection;
pub mod generic;
pub mod playlist;
pub mod song;

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
    query_test!(album::add_songs, "RELATE $album->album_to_song->$songs");
    query_test!(album::read_songs, "SELECT * FROM $album->album_to_song.out");
    query_test!(
        album::remove_songs,
        "DELETE $album->album_to_song WHERE out IN $songs"
    );
    query_test!(
        album::read_artist,
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
        artist::read_albums,
        "SELECT * FROM $id->artist_to_album->album"
    );

    query_test!(artist::add_album, "RELATE $id->artist_to_album->$album");

    query_test!(
        artist::add_album_to_artists,
        "RELATE $ids->artist_to_album->$album"
    );

    query_test!(artist::add_songs, "RELATE $id->artist_to_song->$songs");

    query_test!(
        artist::remove_songs,
        "DELETE $artist->artist_to_song WHERE out IN $songs"
    );

    query_test!(
        artist::read_songs,
        "RETURN array::union((SELECT * FROM $artist->artist_to_song->song), (SELECT * FROM $artist->artist_to_album->album->album_to_song->song))"
    );
}

#[cfg(test)]
mod collection_tests {
    use super::*;

    query_test!(
        collection::add_songs,
        "RELATE $id->collection_to_song->$songs"
    );

    query_test!(
        collection::read_songs,
        "SELECT * FROM $id->collection_to_song.out"
    );

    query_test!(
        collection::remove_songs,
        "DELETE $id->collection_to_song WHERE out IN $songs"
    );

    query_test!(
        collection::repair,
        "UPDATE $id SET song_count=$songs, runtime=$runtime"
    );
}

#[cfg(test)]
mod playlist_tests {
    use super::*;

    query_test!(playlist::add_songs, "RELATE $id->playlist_to_song->$songs");

    query_test!(
        playlist::read_songs,
        "SELECT * FROM $id->playlist_to_song.out"
    );

    query_test!(
        playlist::remove_songs,
        "DELETE $id->playlist_to_song WHERE out IN $songs"
    );

    query_test!(
        playlist::repair,
        "UPDATE $id SET song_count=$songs, runtime=$runtime"
    );
}

#[cfg(test)]
mod song_tests {
    use super::*;

    query_test!(
        song::read_song_by_path,
        "SELECT * FROM song WHERE path = $path LIMIT 1"
    );
    query_test!(song::read_album, "SELECT * FROM $id<-album_to_song.in");
    query_test!(song::read_artist, "SELECT * FROM $id<-artist_to_song.in");
    query_test!(
        song::read_album_artist,
        "SELECT * FROM $id<-album_to_song<-album<-artist_to_album.in"
    );
}
