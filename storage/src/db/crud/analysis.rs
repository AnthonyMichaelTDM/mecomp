//! CRUD operations for the analysis table

use one_or_many::OneOrMany;
use surrealdb::{Connection, Surreal};
use tracing::instrument;

use crate::{
    db::{
        queries::analysis::{
            add_to_song, nearest_neighbors, nearest_neighbors_to_many, read_for_song, read_song,
            read_songs_without_analysis,
        },
        schemas::{
            analysis::{Analysis, AnalysisId, TABLE_NAME},
            song::{Song, SongId},
        },
    },
    errors::{Error, StorageResult},
};

impl Analysis {
    /// create a new analysis for the given song
    ///
    /// If an analysis already exists for the song, this will return None.
    #[instrument]
    pub async fn create<C: Connection>(
        db: &Surreal<C>,
        song_id: SongId,
        analysis: Self,
    ) -> StorageResult<Option<Self>> {
        if Self::read_for_song(db, song_id.clone()).await?.is_some() {
            return Ok(None);
        }

        // create the analysis
        let result: Option<Self> = db
            .create((TABLE_NAME, analysis.id.clone()))
            .content(analysis)
            .await?;

        if let Some(analysis) = result {
            // relate the song to the analysis
            db.query(add_to_song())
                .bind(("id", analysis.id.clone()))
                .bind(("song", song_id))
                .await?;

            // return the analysis
            Ok(Some(analysis))
        } else {
            Ok(None)
        }
    }

    #[instrument]
    pub async fn read<C: Connection>(
        db: &Surreal<C>,
        id: AnalysisId,
    ) -> StorageResult<Option<Self>> {
        Ok(db.select((TABLE_NAME, id)).await?)
    }

    #[instrument]
    pub async fn read_all<C: Connection>(db: &Surreal<C>) -> StorageResult<Vec<Self>> {
        Ok(db.select(TABLE_NAME).await?)
    }

    /// Read the analysis for a song
    ///
    /// If the song does not have an analysis, this will return None.
    #[instrument]
    pub async fn read_for_song<C: Connection>(
        db: &Surreal<C>,
        song_id: SongId,
    ) -> StorageResult<Option<Self>> {
        Ok(db
            .query(read_for_song())
            .bind(("song", song_id))
            .await?
            .take(0)?)
    }

    /// Read the analysis for OneOrMany song(s)
    ///
    /// Needed for clustering(?)
    ///
    /// We return a Vec<Option<Analysis>>, where None means the song doesn't have an analysis, so that it's up to the caller to handle songs without analyses.
    #[instrument]
    pub async fn read_for_songs<C: Connection>(
        db: &Surreal<C>,
        song_ids: Vec<SongId>,
    ) -> StorageResult<Vec<Option<Self>>> {
        futures::future::try_join_all(song_ids.into_iter().map(|id| Self::read_for_song(db, id)))
            .await
    }

    /// Read the song for an analysis
    #[instrument]
    pub async fn read_song<C: Connection>(db: &Surreal<C>, id: AnalysisId) -> StorageResult<Song> {
        Option::<Song>::map_or_else(
            db.query(read_song()).bind(("id", id)).await?.take(0)?,
            || Err(Error::NotFound),
            Ok,
        )
    }

    /// Read the song for OneOrMany analyses
    ///
    /// needed to convert a list of analyses (such as what we get from nearest_neighbors) into a list of songs
    #[instrument]
    pub async fn read_songs<C: Connection>(
        db: &Surreal<C>,
        ids: OneOrMany<AnalysisId>,
    ) -> StorageResult<OneOrMany<Song>> {
        futures::future::try_join_all(ids.into_iter().map(|id| Self::read_song(db, id)))
            .await
            .map(OneOrMany::from)
    }

    /// Get all the songs that don't have an analysis
    #[instrument]
    pub async fn read_songs_without_analysis<C: Connection>(
        db: &Surreal<C>,
    ) -> StorageResult<Vec<Song>> {
        Ok(db.query(read_songs_without_analysis()).await?.take(0)?)
    }

    /// Delete an analysis
    #[instrument]
    pub async fn delete<C: Connection>(
        db: &Surreal<C>,
        id: AnalysisId,
    ) -> StorageResult<Option<Self>> {
        Ok(db.delete((TABLE_NAME, id)).await?)
    }

    /// Find the `n` nearest neighbors to an analysis
    #[instrument]
    pub async fn nearest_neighbors<C: Connection>(
        db: &Surreal<C>,
        id: AnalysisId,
        n: u32,
    ) -> StorageResult<Vec<Self>> {
        let features = Self::read(db, id.clone())
            .await?
            .ok_or(Error::NotFound)?
            .features;

        Ok(db
            .query(nearest_neighbors(n))
            .bind(("id", id))
            .bind(("target", features))
            .await?
            .take(0)?)
    }

    /// Find the `n` nearest neighbors to a list of analyses
    ///
    /// The provided analyses should not be included in the results
    #[instrument]
    pub async fn nearest_neighbors_to_many<C: Connection>(
        db: &Surreal<C>,
        ids: Vec<AnalysisId>,
        n: u32,
    ) -> StorageResult<Vec<Self>> {
        // find the average "features" of the given analyses
        let analyses =
            futures::future::try_join_all(ids.iter().map(|id| Self::read(db, id.clone())))
                .await?
                .into_iter()
                .map(|analysis| analysis.ok_or(Error::NotFound))
                .collect::<Result<Vec<Self>, Error>>()?;

        #[allow(clippy::cast_precision_loss)]
        let num_analyses = analyses.len() as f64;

        let avg_features = analyses.iter().fold(vec![0.; 20], |acc, analysis| {
            acc.iter()
                .zip(analysis.features.iter())
                .map(|(a, b)| a + (b / num_analyses))
                .collect::<Vec<_>>()
        });

        Ok(db
            .query(nearest_neighbors_to_many(n))
            .bind(("ids", ids))
            .bind(("target", avg_features))
            .await?
            .take(0)?)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        db::schemas::song::SongChangeSet,
        test_utils::{arb_song_case, create_song_with_overrides, init_test_database},
    };

    use anyhow::Result;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_create() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };

        // create the analysis
        let result = Analysis::create(&db, song.id.clone(), analysis.clone()).await?;
        assert_eq!(result, Some(analysis.clone()));

        // if we try to create another analysis for the same song, we get Ok(None)
        let analysis = Analysis {
            id: Analysis::generate_id(),
            features: [1.; 20],
        };
        let result = Analysis::create(&db, song.id.clone(), analysis.clone()).await?;
        assert_eq!(result, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };

        // create the analysis
        let result = Analysis::create(&db, song.id.clone(), analysis.clone()).await?;
        assert_eq!(result, Some(analysis.clone()));

        // read the analysis
        let result = Analysis::read(&db, analysis.id.clone()).await?;
        assert_eq!(result, Some(analysis));

        Ok(())
    }

    #[tokio::test]
    async fn test_read_all() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };

        // create the analysis
        let result = Analysis::create(&db, song.id.clone(), analysis.clone()).await?;
        assert_eq!(result, Some(analysis.clone()));

        // read all the analyses
        let result = Analysis::read_all(&db).await?;
        assert_eq!(result, vec![analysis]);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_for_song() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };

        // the song doesn't have an analysis yet
        let result = Analysis::read_for_song(&db, song.id.clone()).await?;
        assert_eq!(result, None);

        // create the analysis
        let result = Analysis::create(&db, song.id.clone(), analysis.clone()).await?;
        assert_eq!(result, Some(analysis.clone()));

        // read the analysis for the song
        let result = Analysis::read_for_song(&db, song.id.clone()).await?;
        assert_eq!(result, Some(analysis));

        Ok(())
    }

    #[tokio::test]
    async fn test_read_for_songs() -> Result<()> {
        let db = init_test_database().await?;

        let song1 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song2 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song3 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis1 = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };
        let analysis2 = Analysis {
            id: Analysis::generate_id(),
            features: [1.; 20],
        };

        // create the analyses
        let result = Analysis::create(&db, song1.id.clone(), analysis1.clone()).await?;
        assert_eq!(result, Some(analysis1.clone()));
        let result = Analysis::create(&db, song2.id.clone(), analysis2.clone()).await?;
        assert_eq!(result, Some(analysis2.clone()));

        // read the analyses for the songs
        let result = Analysis::read_for_songs(
            &db,
            vec![song1.id.clone(), song2.id.clone(), song3.id.clone()],
        )
        .await?;
        assert_eq!(result, vec![Some(analysis1), Some(analysis2), None]);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_song() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };

        // create the analysis
        let result = Analysis::create(&db, song.id.clone(), analysis.clone()).await?;
        assert_eq!(result, Some(analysis.clone()));

        // read the song for the analysis
        let result = Analysis::read_song(&db, analysis.id.clone()).await?;
        assert_eq!(result, song);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_songs() -> Result<()> {
        let db = init_test_database().await?;

        let song1 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song2 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis1 = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };
        let analysis2 = Analysis {
            id: Analysis::generate_id(),
            features: [1.; 20],
        };

        // create the analyses
        let result = Analysis::create(&db, song1.id.clone(), analysis1.clone()).await?;
        assert_eq!(result, Some(analysis1.clone()));
        let result = Analysis::create(&db, song2.id.clone(), analysis2.clone()).await?;
        assert_eq!(result, Some(analysis2.clone()));

        // read the songs for the analyses
        let result = Analysis::read_songs(
            &db,
            OneOrMany::Many(vec![analysis1.id.clone(), analysis2.id.clone()]),
        )
        .await?;
        assert_eq!(result, OneOrMany::Many(vec![song1, song2]));

        Ok(())
    }

    #[tokio::test]
    async fn test_read_songs_without_analysis() -> Result<()> {
        let db = init_test_database().await?;

        let song1 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song2 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        // read the songs without an analysis
        let result = Analysis::read_songs_without_analysis(&db).await?;
        assert_eq!(result.len(), 2);
        assert!(result.contains(&song1));
        assert!(result.contains(&song2));

        let analysis1 = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };
        let analysis2 = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };

        // create the analysis
        let result = Analysis::create(&db, song1.id.clone(), analysis1.clone()).await?;
        assert_eq!(result, Some(analysis1.clone()));

        // read the songs without an analysis
        let result = Analysis::read_songs_without_analysis(&db).await?;
        assert_eq!(result, vec![song2.clone()]);

        // create the analysis
        let result = Analysis::create(&db, song2.id.clone(), analysis2.clone()).await?;
        assert_eq!(result, Some(analysis2.clone()));

        // read the songs without an analysis
        let result = Analysis::read_songs_without_analysis(&db).await?;
        assert_eq!(result, vec![]);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };

        // create the analysis
        let result = Analysis::create(&db, song.id.clone(), analysis.clone()).await?;
        assert_eq!(result, Some(analysis.clone()));

        // delete the analysis
        let result = Analysis::delete(&db, analysis.id.clone()).await?;
        assert_eq!(result, Some(analysis.clone()));

        // if we try to read the analysis, we get None
        let result = Analysis::read(&db, analysis.id.clone()).await?;
        assert_eq!(result, None);

        // if we try to read the analysis for the song, we get None
        let result = Analysis::read_for_song(&db, song.id.clone()).await?;
        assert_eq!(result, None);

        Ok(())
    }

    #[tokio::test]
    async fn test_nearest_neighbors() -> Result<()> {
        let db = init_test_database().await?;

        let song1 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song2 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song3 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis1 = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };
        let analysis2 = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };
        let analysis3 = Analysis {
            id: Analysis::generate_id(),
            features: [1.; 20],
        };

        // create the analyses
        let result1 = Analysis::create(&db, song1.id.clone(), analysis1.clone()).await?;
        assert_eq!(result1, Some(analysis1.clone()));
        let result2 = Analysis::create(&db, song2.id.clone(), analysis2.clone()).await?;
        assert_eq!(result2, Some(analysis2.clone()));
        let result3 = Analysis::create(&db, song3.id.clone(), analysis3.clone()).await?;
        assert_eq!(result3, Some(analysis3.clone()));

        // find the nearest neighbor to analysis1
        let result = Analysis::nearest_neighbors(&db, analysis1.id.clone(), 1).await?;
        assert_eq!(result, vec![analysis2.clone()]);

        Ok(())
    }

    #[tokio::test]
    async fn test_analysis_deleted_when_song_deleted() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = Analysis {
            id: Analysis::generate_id(),
            features: [0.; 20],
        };

        // create the analysis
        let result = Analysis::create(&db, song.id.clone(), analysis.clone()).await?;
        assert_eq!(result, Some(analysis.clone()));

        // delete the song
        let result = Song::delete(&db, song.id.clone()).await?;
        assert_eq!(result, Some(song.clone()));

        // if we try to read the song, we get None
        let result = Song::read(&db, song.id.clone()).await?;
        assert_eq!(result, None);

        // if we try to read the analysis, we get None
        let result = Analysis::read(&db, analysis.id.clone()).await?;
        assert_eq!(result, None);

        // if we try to read the analysis for the song, we get None
        let result = Analysis::read_for_song(&db, song.id.clone()).await?;
        assert_eq!(result, None);

        // if we try to read the songs without an analysis, we get an empty list
        let result = Analysis::read_songs_without_analysis(&db).await?;
        assert_eq!(result, vec![]);

        // if we try to read the song for the analysis, we get an error
        let result = Analysis::read_song(&db, analysis.id.clone()).await;
        assert!(matches!(result, Err(Error::NotFound)));

        Ok(())
    }
}
