//! CRUD operations for the analysis table

use mecomp_analysis::{DIM_EMBEDDING, NUMBER_FEATURES};
use one_or_many::OneOrMany;
use surrealdb::{Connection, Surreal};
use surrealqlx::surrql;
use tracing::instrument;

use crate::{
    db::{
        queries::{
            analysis::{
                add_to_song, nearest_neighbors, nearest_neighbors_to_many, read_for_song,
                read_for_songs, read_song, read_songs, read_songs_without_analysis,
            },
            generic::read_many,
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
        let result: Option<Self> = db.create(analysis.id.clone()).content(analysis).await?;

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
        Ok(db.select(id).await?)
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

    /// Read the analyses for a list of songs
    #[instrument]
    pub async fn read_for_songs<C: Connection>(
        db: &Surreal<C>,
        song_ids: Vec<SongId>,
    ) -> StorageResult<Vec<AnalysisId>> {
        Ok(db
            .query(read_for_songs())
            .bind(("songs", song_ids))
            .await?
            .take(0)?)
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

    /// Read the songs of a list of analyses
    ///
    /// needed to convert a list of analyses (such as what we get from `nearest_neighbors`) into a list of songs
    #[instrument]
    pub async fn read_songs<C: Connection>(
        db: &Surreal<C>,
        ids: Vec<AnalysisId>,
    ) -> StorageResult<OneOrMany<Song>> {
        Ok(db
            .query(read_songs())
            .bind(("ids", ids.clone()))
            .await?
            .take(0)?)
    }

    /// Get all the songs that don't have an analysis
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or if the result cannot be deserialized.
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
        Ok(db.delete(id).await?)
    }

    /// Delete all analyses
    #[instrument]
    pub async fn delete_all<C: Connection>(db: &Surreal<C>) -> StorageResult<()> {
        // explicitly do not deserialize the result since this function might be used
        // in cases where the analysis table has malformed data
        db.query(surrql!("DELETE analysis;DELETE analysis_to_song;"))
            .await?;
        Ok(())
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
        // whether to use feature-based or embedding-based analysis
        use_embeddings: bool,
    ) -> StorageResult<Vec<Self>> {
        if ids.is_empty() || n == 0 {
            return Ok(vec![]);
        }

        // find the average "features" / "embeddings" of the given analyses
        let analyses: Vec<Self> = db
            .query(read_many())
            .bind(("ids", ids.clone()))
            .await?
            .take(0)?;

        let query = db
            .query(nearest_neighbors_to_many(n, use_embeddings))
            .bind(("ids", ids));

        #[allow(clippy::cast_precision_loss)]
        let num_analyses = analyses.len() as f64;

        let query = if use_embeddings {
            let avg_embedding = analyses
                .iter()
                .fold(vec![0.; DIM_EMBEDDING], |acc, analysis| {
                    acc.iter()
                        .zip(analysis.embedding.iter())
                        .map(|(a, b)| a + (b / num_analyses))
                        .collect::<Vec<_>>()
                });

            query.bind(("target", avg_embedding))
        } else {
            let avg_features = analyses
                .iter()
                .fold(vec![0.; NUMBER_FEATURES], |acc, analysis| {
                    acc.iter()
                        .zip(analysis.features.iter())
                        .map(|(a, b)| a + (b / num_analyses))
                        .collect::<Vec<_>>()
                });

            query.bind(("target", avg_features))
        };

        Ok(query.await?.take(0)?)
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
    use rstest::rstest;

    fn analysis_zeroes() -> Analysis {
        Analysis {
            id: Analysis::generate_id(),
            features: [0.; 23],
            embedding: [0.; 32],
        }
    }
    fn analysis_ones() -> Analysis {
        Analysis {
            id: Analysis::generate_id(),
            features: [1.; 23],
            embedding: [1.; 32],
        }
    }

    #[tokio::test]
    async fn test_create() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = analysis_zeroes();

        // create the analysis
        let result = Analysis::create(&db, song.id.clone(), analysis.clone()).await?;
        assert_eq!(result, Some(analysis.clone()));

        // if we try to create another analysis for the same song, we get Ok(None)
        let analysis = analysis_ones();

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
            features: [0.; 23],
            embedding: [0.; 32],
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

        let analysis = analysis_zeroes();

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

        let analysis = analysis_zeroes();

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

        let analysis1 = analysis_zeroes();
        let analysis2 = analysis_ones();

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
        assert_eq!(result, vec![analysis1.id, analysis2.id]);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_song() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = analysis_zeroes();

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

        let analysis1 = analysis_zeroes();
        let analysis2 = analysis_ones();

        // create the analyses
        let result = Analysis::create(&db, song1.id.clone(), analysis1.clone()).await?;
        assert_eq!(result, Some(analysis1.clone()));
        let result = Analysis::create(&db, song2.id.clone(), analysis2.clone()).await?;
        assert_eq!(result, Some(analysis2.clone()));

        // read the songs for the analyses
        let result =
            Analysis::read_songs(&db, vec![analysis1.id.clone(), analysis2.id.clone()]).await?;
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

        let analysis1 = analysis_zeroes();
        let analysis2 = analysis_ones();

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

        let analysis = analysis_zeroes();

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
    async fn test_analysis_delete_all_when_malformed_data_is_present() -> Result<()> {
        #[derive(Debug, serde::Serialize, serde::Deserialize)]
        struct MalformedAnalysis {
            id: AnalysisId,
            features: [f32; 10],
        }
        let config = surrealdb::opt::Config::new().strict();
        let db = Surreal::new::<surrealdb::engine::local::Mem>(config).await?;
        db.query("DEFINE NAMESPACE IF NOT EXISTS test").await?;
        db.use_ns("test").await?;
        db.query("DEFINE DATABASE IF NOT EXISTS test").await?;
        db.use_db("test").await?;
        // create the analysis table without specifying the schema
        db.query("DEFINE TABLE analysis").await?;

        let analysis = MalformedAnalysis {
            id: Analysis::generate_id(),
            features: [0.; 10],
        };
        // insert a malformed analysis directly
        let _: Option<MalformedAnalysis> = db.create(analysis.id.clone()).content(analysis).await?;
        // register a vector index that expects 23-dimensional vectors
        db.query(
            "DEFINE INDEX analysis_features_vector_index ON analysis FIELDS features MTREE DIMENSION 23;",
        )
        .await?;

        // delete all analyses
        Analysis::delete_all(&db).await?;
        // there should be no analyses left
        let result = Analysis::read_all(&db).await?;
        assert_eq!(result.len(), 0);
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

        let analysis1 = analysis_zeroes();
        let analysis2 = analysis_zeroes();
        let analysis3 = analysis_ones();

        // create the analyses
        let result1 = Analysis::create(&db, song1.id.clone(), analysis1.clone()).await?;
        assert_eq!(result1, Some(analysis1.clone()));
        let result2 = Analysis::create(&db, song2.id.clone(), analysis2.clone()).await?;
        assert_eq!(result2, Some(analysis2.clone()));
        let result3 = Analysis::create(&db, song3.id.clone(), analysis3.clone()).await?;
        assert_eq!(result3, Some(analysis3.clone()));

        // find the nearest neighbor to analysis1
        let result = Analysis::nearest_neighbors(&db, analysis1.id, 1).await?;
        assert_eq!(result, vec![analysis2.clone()]);

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_nearest_neighbors_to_many(
        #[values(false, true)] use_embeddings: bool,
    ) -> Result<()> {
        let db = init_test_database().await?;

        let song1 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song2 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song3 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;
        let song4 =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis1 = analysis_zeroes();
        let analysis2 = analysis_zeroes();
        let analysis3 = analysis_ones();
        let analysis4 = analysis_ones();

        // create the analyses
        let result1 = Analysis::create(&db, song1.id.clone(), analysis1.clone()).await?;
        assert_eq!(result1, Some(analysis1.clone()));
        let result2 = Analysis::create(&db, song2.id.clone(), analysis2.clone()).await?;
        assert_eq!(result2, Some(analysis2.clone()));
        let result3 = Analysis::create(&db, song3.id.clone(), analysis3.clone()).await?;
        assert_eq!(result3, Some(analysis3.clone()));
        let result4 = Analysis::create(&db, song4.id.clone(), analysis4.clone()).await?;
        assert_eq!(result4, Some(analysis4.clone()));

        // find the nearest neighbor to analysis1 and analysis2
        // with n = 0, we should get an empty list
        let result = Analysis::nearest_neighbors_to_many(
            &db,
            vec![analysis1.id.clone(), analysis2.id.clone()],
            0,
            use_embeddings,
        )
        .await?;
        assert_eq!(result.len(), 0);
        // with n = 1, we should get one of the two analyses
        let result = Analysis::nearest_neighbors_to_many(
            &db,
            vec![analysis1.id.clone(), analysis2.id.clone()],
            1,
            use_embeddings,
        )
        .await?;
        assert_eq!(result.len(), 1);
        assert!((result[0] == analysis3) || (result[0] == analysis4));
        // with n = 2, we should get both analyses
        let result = Analysis::nearest_neighbors_to_many(
            &db,
            vec![analysis1.id.clone(), analysis2.id.clone()],
            2,
            use_embeddings,
        )
        .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], analysis3);
        assert_eq!(result[1], analysis4);
        // with n > 2, we should get both analyses
        let result = Analysis::nearest_neighbors_to_many(
            &db,
            vec![analysis1.id.clone(), analysis2.id.clone()],
            3,
            use_embeddings,
        )
        .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], analysis3);
        assert_eq!(result[1], analysis4);

        // find the nearest neighbor to analysis3 and analysis4
        let result = Analysis::nearest_neighbors_to_many(
            &db,
            vec![analysis3.id.clone(), analysis4.id.clone()],
            3,
            use_embeddings,
        )
        .await?;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], analysis1);
        assert_eq!(result[1], analysis2);

        // if we pass an empty list, we should get an empty list
        let result = Analysis::nearest_neighbors_to_many(&db, vec![], 3, use_embeddings).await?;
        assert_eq!(result.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_analysis_deleted_when_song_deleted() -> Result<()> {
        let db = init_test_database().await?;

        let song =
            create_song_with_overrides(&db, arb_song_case()(), SongChangeSet::default()).await?;

        let analysis = analysis_zeroes();

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
