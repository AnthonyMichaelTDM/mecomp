#[cfg(feature = "db")]
pub mod crud;
#[cfg(feature = "db")]
pub mod health;
#[cfg(feature = "db")]
pub(crate) mod queries;
pub mod schemas;

#[cfg(feature = "db")]
use surrealdb::{Surreal, engine::local::Db, opt::Config};

#[cfg(feature = "db")]
#[cfg(not(tarpaulin_include))]
static DB_DIR: once_cell::sync::OnceCell<std::path::PathBuf> = once_cell::sync::OnceCell::new();
#[cfg(feature = "db")]
#[cfg(not(tarpaulin_include))]
static TEMP_DB_DIR: once_cell::sync::Lazy<tempfile::TempDir> = once_cell::sync::Lazy::new(|| {
    tempfile::tempdir().expect("Failed to create temporary directory")
});

/// Set the path to the database.
///
/// # Errors
///
/// This function will return an error if the path cannot be set.
#[cfg(feature = "db")]
#[allow(clippy::missing_inline_in_public_items)]
pub fn set_database_path(path: std::path::PathBuf) -> Result<(), crate::errors::Error> {
    DB_DIR
        .set(path)
        .map_err(crate::errors::Error::DbPathSetError)?;
    log::info!("Primed database path");
    Ok(())
}

/// Initialize the database with the necessary tables.
///
/// # Errors
///
/// This function will return an error if the database cannot be initialized.
#[cfg(feature = "db")]
#[allow(clippy::missing_inline_in_public_items)]
pub async fn init_database() -> surrealqlx::Result<Surreal<Db>> {
    use surrealqlx::surrql;

    let config = Config::new().strict();
    let db_path = DB_DIR
    .get().cloned()
    .unwrap_or_else(|| {
        log::warn!("DB_DIR not set, defaulting to a temporary directory `{}`, this is likely a bug because `set_database_path` should be called before `init_database`", TEMP_DB_DIR.path().display());
        TEMP_DB_DIR.path()
        .to_path_buf()
    });
    let db = Surreal::new((db_path, config)).await?;

    db.query(surrql!("DEFINE NAMESPACE IF NOT EXISTS mecomp"))
        .await?;
    db.use_ns("mecomp").await?;
    db.query(surrql!("DEFINE DATABASE IF NOT EXISTS music"))
        .await?;
    db.use_db("music").await?;

    register_custom_analyzer(&db).await?;
    surrealqlx::register_tables!(
        &db,
        schemas::album::Album,
        schemas::artist::Artist,
        schemas::song::Song,
        schemas::collection::Collection,
        schemas::playlist::Playlist,
        schemas::dynamic::DynamicPlaylist
    )?;
    #[cfg(feature = "analysis")]
    surrealqlx::register_tables!(&db, schemas::analysis::Analysis)?;

    queries::relations::define_relation_tables(&db).await?;

    Ok(db)
}

#[cfg(feature = "db")]
pub(crate) async fn register_custom_analyzer<C>(db: &Surreal<C>) -> surrealqlx::Result<()>
where
    C: surrealdb::Connection,
{
    use surrealqlx::{
        migrations::{M, Migrations},
        surrql,
    };

    // NOTE: if you change this, you must go through the schemas and update the index analyzer names
    let analyzer_definition = surrql!(
        "DEFINE ANALYZER IF NOT EXISTS custom_analyzer
         TOKENIZERS class
         FILTERS ascii,lowercase,edgengram(1,10),snowball(English);"
    );

    let migrations = Migrations::new(
        "custom_analyzer",
        vec![
            M::up(analyzer_definition).down(surrql!("REMOVE ANALYZER IF EXISTS custom_analyzer;")),
        ],
    );

    migrations.to_latest(db).await?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::schemas::{
        album::Album, artist::Artist, collection::Collection, dynamic::DynamicPlaylist,
        playlist::Playlist, song::Song,
    };
    use super::*;

    use surrealdb::engine::local::Mem;
    use surrealqlx::traits::Table;

    #[tokio::test]
    async fn test_register_tables() -> anyhow::Result<()> {
        let config = Config::new().strict();
        // use an in-memory db for testing
        let db = Surreal::new::<Mem>(config).await?;

        db.query("DEFINE NAMESPACE IF NOT EXISTS test").await?;
        db.use_ns("test").await?;
        db.query("DEFINE DATABASE IF NOT EXISTS test").await?;
        db.use_db("test").await?;

        // register the custom analyzer
        register_custom_analyzer(&db).await?;

        // first we init all the table to ensure that the queries made by the macro work without error
        <Album as Table>::init_table(&db).await?;
        <Artist as Table>::init_table(&db).await?;
        <Song as Table>::init_table(&db).await?;
        <Collection as Table>::init_table(&db).await?;
        <Playlist as Table>::init_table(&db).await?;
        <DynamicPlaylist as Table>::init_table(&db).await?;

        // then we init the relation tables
        queries::relations::define_relation_tables(&db).await?;

        // then we try initializing one of the tables again to ensure that initialization won't mess with existing tables/data
        <Album as Table>::init_table(&db).await?;

        Ok(())
    }
}

#[cfg(test)]
mod minimal_reproduction {
    //! This module contains minimal reproductions of issues from MECOMPs past.
    //! They exist to ensure that the issues are indeed fixed.
    use serde::{Deserialize, Serialize};
    use surrealdb::{RecordId, Surreal, engine::local::Mem, method::Stats};
    use surrealqlx::surrql;

    use crate::db::queries::generic::{Count, count};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct User {
        id: RecordId,
        name: String,
        age: u64,
        favorite_numbers: [u64; 7],
    }

    static SCHEMA_SQL: &str = r"
    BEGIN;
    DEFINE TABLE users SCHEMAFULL;
    COMMIT;
    BEGIN;
    DEFINE FIELD id ON users TYPE record;
    DEFINE FIELD name ON users TYPE string;
    DEFINE FIELD age ON users TYPE int;
    DEFINE FIELD favorite_numbers ON users TYPE array<int>;
    COMMIT;
    BEGIN;
    DEFINE INDEX users_name_unique_index ON users FIELDS name UNIQUE;
    DEFINE INDEX users_age_normal_index ON users FIELDS age;
    DEFINE INDEX users_favorite_numbers_vector_index ON users FIELDS favorite_numbers MTREE DIMENSION 7;
    ";
    const NUMBER_OF_USERS: u64 = 100;

    #[tokio::test]
    async fn minimal_reproduction() {
        let db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();

        db.query(SCHEMA_SQL).await.unwrap();

        let cnt: Option<Count> = db
            // new syntax
            .query(count("users"))
            .await
            .unwrap()
            .take(0)
            .unwrap();

        assert_eq!(cnt, Some(Count::new(0)));

        let john_id = RecordId::from(("users", "0"));
        let john = User {
            id: john_id.clone(),
            name: "John".to_string(),
            age: 42,
            favorite_numbers: [1, 2, 3, 4, 5, 6, 7],
        };

        let sally_id = RecordId::from(("users", "1"));
        let sally = User {
            id: sally_id.clone(),
            name: "Sally".to_string(),
            age: 24,
            favorite_numbers: [8, 9, 10, 11, 12, 13, 14],
        };

        let result: Option<User> = db
            .create(john_id.clone())
            .content(john.clone())
            .await
            .unwrap();

        assert_eq!(result, Some(john.clone()));

        let result: Option<User> = db
            .create(sally_id.clone())
            .content(sally.clone())
            .await
            .unwrap();

        assert_eq!(result, Some(sally.clone()));

        let result: Option<User> = db.select(john_id).await.unwrap();

        assert_eq!(result, Some(john.clone()));

        // create like 100 more users
        for i in 2..NUMBER_OF_USERS {
            let user_id = RecordId::from(("users", i.to_string()));
            let user = User {
                id: user_id.clone(),
                name: format!("User {i}"),
                age: i,
                favorite_numbers: [i; 7],
            };
            let _: Option<User> = db.create(user_id.clone()).content(user).await.unwrap();
        }

        let mut resp_new = db
            // new syntax
            .query(surrql!("SELECT count() FROM users GROUP ALL"))
            .with_stats()
            .await
            .unwrap();
        dbg!(&resp_new);
        let res = resp_new.take(0).unwrap();
        let cnt: Option<Count> = res.1.unwrap();
        assert_eq!(cnt, Some(Count::new(NUMBER_OF_USERS)));
        let stats_new: Stats = res.0;

        let mut resp_old = db
            // old syntax
            .query(surrql!("RETURN array::len((SELECT * FROM users))"))
            .with_stats()
            .await
            .unwrap();
        dbg!(&resp_old);
        let res = resp_old.take(0).unwrap();
        let cnt: Option<u64> = res.1.unwrap();
        assert_eq!(cnt, Some(NUMBER_OF_USERS));
        let stats_old: Stats = res.0;

        // just a check to ensure the new syntax is faster
        assert!(stats_new.execution_time.unwrap() < stats_old.execution_time.unwrap());

        let result: Vec<User> = db.delete("users").await.unwrap();

        assert_eq!(result.len() as u64, NUMBER_OF_USERS);
        assert!(result.contains(&john), "Result does not contain 'john'");
        assert!(result.contains(&sally), "Result does not contain 'sally'");
    }
}
