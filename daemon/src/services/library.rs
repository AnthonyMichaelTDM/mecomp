use log::{info, warn};
use mecomp_storage::{db::schemas::song::Song, errors::Error};

pub async fn rescan() -> Result<(), Error> {
    info!("Rescanning library");
    tokio::task::block_in_place(|| async move {
        // get all the songs in the current library
        let songs = Song::read_all().await?;

        // for each song, check if the file still exists
        for song in songs {
            let path = song.path;
            if !path.exists() {
                // remove the song from the library
                warn!("Song {} no longer exists, deleting", path.to_string_lossy());
                Song::delete(song.id).await?;
            } else {
                // check if the metadata of the file is the same as the metadata in the database

                // if the file has been modified, update the song's metadata, unless the difference is the runtime

                // if the file has not been modified, do nothing

                todo!();
            }
        }

        // if the file has been modified, update the song's metadata

        // if the file has not been modified, do nothing

        // for each file in the library, check if the file is new

        Ok::<_, Error>(())
    })
    .await
}
