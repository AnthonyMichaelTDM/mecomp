//----------------------------------------------------------------------------------------- std lib
use log::info;
use std::{net::SocketAddr, ops::Range};
//--------------------------------------------------------------------------------- other libraries
use ::tarpc::context::Context;
//-------------------------------------------------------------------------------- MECOMP libraries
use mecomp_core::{
    errors::LibraryError,
    library::{LibraryBrief, LibraryFull, LibraryHealth},
    playback::{Percent, RepeatMode, SeekType, StateAudio, StateRuntime},
    queue::Queue,
    rpc::MusicPlayer,
    search::SearchResult,
};
use mecomp_storage::db::schemas::{
    album::{Album, AlbumBrief, AlbumId},
    artist::{Artist, ArtistBrief, ArtistId},
    collection::{Collection, CollectionBrief, CollectionId},
    playlist::{Playlist, PlaylistBrief, PlaylistId},
    song::{Song, SongBrief, SongId},
};

use crate::{config::SETTINGS, services};

#[derive(Clone)]
pub struct MusicPlayerServer(pub SocketAddr);

impl MusicPlayer for MusicPlayerServer {
    async fn ping(self, _: Context) -> String {
        "pong".to_string()
    }

    #[doc = r" Rescans the music library."]
    async fn library_rescan(self, _: Context) -> Result<(), LibraryError> {
        services::library::rescan(
            &SETTINGS.library_paths,
            SETTINGS.artist_separator.as_deref(),
            SETTINGS.genre_separator.as_deref(),
            SETTINGS.conflict_resolution,
        )
        .await
        .map_err(|e| LibraryError::from(e))
    }

    #[doc = r" Returns brief information about the music library."]
    async fn library_brief(self, _context: Context) -> LibraryBrief {
        info!("Creating library brief");
        services::library::brief().await.unwrap()
    }

    #[doc = r" Returns full information about the music library. (all songs, artists, albums, etc.)"]
    async fn library_full(self, _context: Context) -> LibraryFull {
        todo!()
    }

    #[doc = r" Returns brief information about the music library's artists."]
    async fn library_artists_brief(self, _context: Context) -> Box<[ArtistBrief]> {
        todo!()
    }

    #[doc = r" Returns full information about the music library's artists."]
    async fn library_artists_full(self, _context: Context) -> Box<[Artist]> {
        todo!()
    }

    #[doc = r" Returns brief information about the music library's albums."]
    async fn library_albums_brief(self, _context: Context) -> Box<[AlbumBrief]> {
        todo!()
    }

    #[doc = r" Returns full information about the music library's albums."]
    async fn library_albums_full(self, _context: Context) -> Box<[Album]> {
        todo!()
    }

    #[doc = r" Returns brief information about the music library's songs."]
    async fn library_songs_brief(self, _context: Context) -> Box<[SongBrief]> {
        todo!()
    }

    #[doc = r" Returns full information about the music library's songs."]
    async fn library_songs_full(self, _context: Context) -> Box<[Song]> {
        todo!()
    }

    #[doc = r" Returns information about the health of the music library (are there any missing files, etc.)"]
    async fn library_health(self, _context: Context) -> LibraryHealth {
        todo!()
    }

    #[doc = r" Get a song by its ID."]
    async fn library_song_get(self, _context: Context, _id: SongId) -> Option<Song> {
        todo!()
    }

    #[doc = r" Get an album by its ID."]
    async fn library_album_get(self, _context: Context, _id: AlbumId) -> Option<Album> {
        todo!()
    }

    #[doc = r" Get an artist by its ID."]
    async fn library_artist_get(self, _context: Context, _id: ArtistId) -> Option<Artist> {
        todo!()
    }

    #[doc = r" Get a playlist by its ID."]
    async fn library_playlist_get(self, _context: Context, _id: PlaylistId) -> Option<Playlist> {
        todo!()
    }

    #[doc = r" tells the daemon to shutdown."]
    async fn daemon_shutdown(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" returns full information about the current state of the audio player (queue, current song, etc.)"]
    async fn state_audio(self, _context: Context) -> StateAudio {
        todo!()
    }

    #[doc = r" returns information about the current queue."]
    async fn state_queue(self, _context: Context) -> Queue {
        todo!()
    }

    #[doc = r" is the player currently playing?"]
    async fn state_playing(self, _context: Context) -> bool {
        todo!()
    }

    #[doc = r" what repeat mode is the player in?"]
    async fn state_repeat(self, _context: Context) -> RepeatMode {
        todo!()
    }

    #[doc = r" returns the current volume."]
    async fn state_volume(self, _context: Context) -> Percent {
        todo!()
    }

    #[doc = r" returns the current volume mute state."]
    async fn state_volume_muted(self, _context: Context) -> bool {
        todo!()
    }

    #[doc = r" returns information about the runtime of the current song (seek position and duration)"]
    async fn state_runtime(self, _context: Context) -> StateRuntime {
        todo!()
    }

    #[doc = r" returns the current artist."]
    async fn current_artist(self, _context: Context) -> Option<Artist> {
        todo!()
    }

    #[doc = r" returns the current album."]
    async fn current_album(self, _context: Context) -> Option<Album> {
        todo!()
    }

    #[doc = r" returns the current song."]
    async fn current_song(self, _context: Context) -> Option<Song> {
        todo!()
    }

    #[doc = r" returns a random artist."]
    async fn rand_artist(self, _context: Context) -> Option<Artist> {
        todo!()
    }

    #[doc = r" returns a random album."]
    async fn rand_album(self, _context: Context) -> Option<Album> {
        todo!()
    }

    #[doc = r" returns a random song."]
    async fn rand_song(self, _context: Context) -> Option<Song> {
        todo!()
    }

    #[doc = r" returns a list of artists, albums, and songs matching the given search query."]
    async fn search(self, _context: Context, _query: String) -> Box<[SearchResult]> {
        todo!()
    }

    #[doc = r" returns a list of artists matching the given search query."]
    async fn search_artist(self, _context: Context, _query: String) -> Box<[Artist]> {
        todo!()
    }

    #[doc = r" returns a list of albums matching the given search query."]
    async fn search_album(self, _context: Context, _query: String) -> Box<[Album]> {
        todo!()
    }

    #[doc = r" returns a list of songs matching the given search query."]
    async fn search_song(self, _context: Context, _query: String) -> Box<[Song]> {
        todo!()
    }

    #[doc = r" toggles playback (play/pause)."]
    async fn playback_toggle(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" start playback (unpause)."]
    async fn playback_play(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" pause playback."]
    async fn playback_pause(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" set the current song to be the next song in the queue."]
    async fn playback_next(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" skip forward by the given amount of songs"]
    async fn playback_skip(self, _context: Context, _amount: usize) -> () {
        todo!()
    }

    #[doc = r" go back to the previous song."]
    #[doc = r" (if the current song is more than `threshold` seconds in, it will restart the current song instead)"]
    async fn playback_previous(self, _context: Context, _threshold: Option<usize>) -> () {
        todo!()
    }

    #[doc = r" go backwards by the given amount of songs."]
    async fn playback_back(self, _context: Context, _amountt: usize) -> () {
        todo!()
    }

    #[doc = r" stop playback."]
    #[doc = r" (clears the queue and stops playback)"]
    async fn playback_stop(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" clear the queue."]
    async fn playback_clear(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" seek forwards, backwards, or to an absolute second in the current song."]
    async fn playback_seek(self, _context: Context, _seek: SeekType, _seconds: u64) -> () {
        todo!()
    }

    #[doc = r" set the repeat mode."]
    async fn playback_repeat(self, _context: Context, mode: RepeatMode) -> () {
        let _ = mode;
        todo!()
    }

    #[doc = r" Shuffle the current queue, then start playing from the 1st Song in the queue."]
    async fn playback_shuffle(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" set the volume to the given value (0-100)."]
    #[doc = r" (if the value is greater than 100, it will be clamped to 100.)"]
    async fn playback_volume(self, _context: Context, _volume: u8) -> () {
        todo!()
    }

    #[doc = r" increase the volume by the given amount (0-100)."]
    #[doc = r" (volume will be clamped to 100.)"]
    async fn playback_volume_up(self, _context: Context, _amount: u8) -> () {
        todo!()
    }

    #[doc = r" decrease the volume by the given amount (0-100)."]
    #[doc = r" (volume will be clamped to 0.)"]
    async fn playback_volume_down(self, _context: Context, _amount: u8) -> () {
        todo!()
    }

    #[doc = r" toggle the volume mute."]
    async fn playback_volume_toggle_mute(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" mute the volume."]
    async fn playback_mute(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" unmute the volume."]
    async fn playback_unmute(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" add a song to the queue."]
    #[doc = r" (if the queue is empty, it will start playing the song.)"]
    async fn queue_add_song(self, _context: Context, _song: SongId) -> () {
        todo!()
    }

    #[doc = r" add an album to the queue."]
    #[doc = r" (if the queue is empty, it will start playing the album.)"]
    async fn queue_add_album(self, _context: Context, _album: AlbumId) -> () {
        todo!()
    }

    #[doc = r" add an artist to the queue."]
    #[doc = r" (if the queue is empty, it will start playing the artist.)"]
    async fn queue_add_artist(self, _context: Context, _artist: ArtistId) -> () {
        todo!()
    }

    #[doc = r" add a playlist to the queue."]
    #[doc = r" (if the queue is empty, it will start playing the playlist.)"]
    async fn queue_add_playlist(self, _context: Context, _playlist: PlaylistId) -> () {
        todo!()
    }

    #[doc = r" add a collection to the queue."]
    #[doc = r" (if the queue is empty, it will start playing the collection.)"]
    async fn queue_add_collection(self, _context: Context, _collection: CollectionId) -> () {
        todo!()
    }

    #[doc = r" add a random song to the queue."]
    #[doc = r" (if the queue is empty, it will start playing the song.)"]
    async fn queue_add_rand_song(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" add a random album to the queue."]
    #[doc = r" (if the queue is empty, it will start playing the album.)"]
    async fn queue_add_rand_album(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" add a random artist to the queue."]
    #[doc = r" (if the queue is empty, it will start playing the artist.)"]
    async fn queue_add_rand_artist(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" set the current song to a queue index."]
    #[doc = r" if the index is out of bounds, it will be clamped to the nearest valid index."]
    async fn queue_set_index(self, _context: Context, _index: usize) -> () {
        todo!()
    }

    #[doc = r" remove a range of songs from the queue."]
    #[doc = r" if the range is out of bounds, it will be clamped to the nearest valid range."]
    async fn queue_remove_range(self, _context: Context, _range: Range<usize>) -> () {
        todo!()
    }

    #[doc = r" Returns brief information about the users playlists."]
    async fn playlist_list(self, _context: Context) -> Box<[PlaylistBrief]> {
        todo!()
    }

    #[doc = r" create a new playlist."]
    async fn playlist_new(self, _context: Context, _name: String) -> PlaylistId {
        todo!()
    }

    #[doc = r" remove a playlist."]
    async fn playlist_remove(self, _context: Context, _name: String) -> bool {
        todo!()
    }

    #[doc = r" clone a playlist."]
    #[doc = r" (creates a new playlist with the same name and contents as the given playlist.)"]
    async fn playlist_clone(self, _context: Context, _name: String) -> () {
        todo!()
    }

    #[doc = r" get the id of a playlist."]
    async fn playlist_get_id(self, _context: Context, _name: String) -> Option<PlaylistId> {
        todo!()
    }

    #[doc = r" remove a song from a playlist."]
    #[doc = r" if the song is not in the playlist, this will do nothing."]
    async fn playlist_remove_song(
        self,
        _context: Context,
        _playlist: PlaylistId,
        _song: SongId,
    ) -> () {
        todo!()
    }

    #[doc = r" Add an artist to a playlist."]
    async fn playlist_add_artist(
        self,
        _context: Context,
        _playlist: PlaylistId,
        _artist: ArtistId,
    ) -> () {
        todo!()
    }

    #[doc = r" Add an album to a playlist."]
    async fn playlist_add_album(
        self,
        _context: Context,
        _playlist: PlaylistId,
        _album: AlbumId,
    ) -> () {
        todo!()
    }

    #[doc = r" Add a song to a playlist."]
    async fn playlist_add_song(
        self,
        _context: Context,
        _playlist: PlaylistId,
        _song: SongId,
    ) -> () {
        todo!()
    }

    #[doc = r" Get a playlist by its ID."]
    async fn playlist_get(self, _context: Context, _id: PlaylistId) -> Option<Playlist> {
        todo!()
    }

    #[doc = r" Collections: Return brief information about the users auto curration collections."]
    async fn collection_list(self, _context: Context) -> Box<[CollectionBrief]> {
        todo!()
    }

    #[doc = r" Collections: Recluster the users library, creating new collections."]
    async fn collection_recluster(self, _context: Context) -> () {
        todo!()
    }

    #[doc = r" Collections: get a collection by its ID."]
    async fn collection_get(self, _context: Context, _id: CollectionId) -> Option<Collection> {
        todo!()
    }

    #[doc = r" Collections: freeze a collection (convert it to a playlist)."]
    async fn collection_freeze(
        self,
        _context: Context,
        _id: CollectionId,
        _name: String,
    ) -> PlaylistId {
        todo!()
    }

    #[doc = r" Radio: get the `n` most similar songs to the given song."]
    async fn radio_get_similar_songs(
        self,
        _context: Context,
        _song: SongId,
        _n: usize,
    ) -> Box<[SongId]> {
        todo!()
    }

    #[doc = r" Radio: get the `n` most similar artists to the given artist."]
    async fn radio_get_similar_artist(
        self,
        _context: Context,
        _artist: ArtistId,
        _n: usize,
    ) -> Box<[ArtistId]> {
        todo!()
    }

    #[doc = r" Radio: get the `n` most similar albums to the given album."]
    async fn radio_get_similar_album(
        self,
        _context: Context,
        _album: AlbumId,
        _n: usize,
    ) -> Box<[AlbumId]> {
        todo!()
    }
}
