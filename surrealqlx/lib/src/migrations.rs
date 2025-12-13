//! Migrations for `SurrealDB` databases.
//!
//! Sourced from [surrealdb-migrator](https://github.com/prabirshrestha/surrealdb-migrator/blob/6cfd1edeab43da19d616a98a13f3cf57261f0508/src/lib.rs) with modifications.
//!
//! surrealdb-migrator is licensed under the [Apache-2.0 license](https://github.com/prabirshrestha/surrealdb-migrator/blob/6cfd1edeab43da19d616a98a13f3cf57261f0508/LICENSE)
use std::{
    cmp::{self, Ordering},
    fmt,
    num::NonZeroUsize,
};

use surrealdb::{Connection, Surreal};
use surrealqlx_macros::surrql;
use tracing::{debug, error, info, trace, warn};

/// A typedef of the result returned by many methods.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Enum listing possible errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Surreldb error: {0}")]
    /// Surrealdb error without context
    SurrealdbError(#[from] surrealdb::Error),
    #[error("Surreldb error: ({context}) {err}")]
    /// Surrealdb error, query may indicate the attempted SQL query
    SurrealdbErrorWithContext {
        /// information about SQL query that caused the error
        context: String,
        /// Error returned by surrealdb
        err: surrealdb::Error,
    },
    /// A `SurrealDB` error that occurred during a migration
    #[error("error during migrations at statement {}/{number_of_statements} when migrating from v{current_version} to v{target_version} in scope {scope}: {err}", i + 1)]
    SurrealdbErrorDuringMigration {
        /// The scope of the migration
        scope: &'static str,
        /// The index of the statement that caused the error
        i: usize,
        /// The total number of statements in the transaction
        number_of_statements: usize,
        /// The current version before migration
        current_version: usize,
        /// The target version after migration
        target_version: usize,
        /// The underlying `SurrealDB` error
        err: surrealdb::Error,
    },
    #[error("Specified schema version error: {0}")]
    /// Error with the specified schema version
    SpecifiedSchemaVersion(SchemaVersionError),
    #[error("Migration definition error: {0}")]
    /// Something wrong with migration definitions
    MigrationDefinition(MigrationDefinitionError),
}

/// Errors related to schema versions
#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
#[non_exhaustive]
pub enum SchemaVersionError {
    /// Attempt to migrate to a version out of range for the supplied migrations
    #[error("Target version out of range: specified: specified={specified} highest={highest}")]
    TargetVersionOutOfRange {
        /// The attempt to migrate to this version caused the error
        specified: SchemaVersion,
        /// Highest version defined in the migration set
        highest: SchemaVersion,
    },
}

impl<S> From<(S, surrealdb::Error)> for Error
where
    S: ToString,
{
    fn from((context, err): (S, surrealdb::Error)) -> Self {
        Self::SurrealdbErrorWithContext {
            context: context.to_string(),
            err,
        }
    }
}

/// Errors related to schema versions
#[derive(thiserror::Error, Debug)]
pub enum MigrationDefinitionError {
    #[error("Down not defined error")]
    /// Migration has no down version
    DownNotDefined {
        /// Index of the migration that caused the error
        migration_index: usize,
    },
    #[error("No migration defined")]
    /// Attempt to migrate when no migrations are defined
    NoMigrationsDefined,
    #[error("Database too far ahead")]
    /// Attempt to migrate when the database is currently at a higher migration level
    DatabaseTooFarAhead,
}

/// One migration.
#[derive(Debug, Clone)]
pub struct M<'a> {
    up: &'a str,
    down: Option<&'a str>,
    comment: Option<&'a str>,
}

impl<'a> M<'a> {
    /// Create a schema update. The SQL command will be executed only when the migration has not been
    /// executed on the underlying database.
    ///
    /// # Example
    ///
    /// ```no_test
    /// use surrealdb_migration::M;
    ///
    /// M::up("DEFINE TABLE user; DEFINE FIELD username ON user TYPE string;");
    /// ```
    #[must_use]
    pub const fn up(sql: &'a str) -> Self {
        Self {
            up: sql,
            down: None,
            comment: None,
        }
    }

    /// Define a down-migration. This SQL statement should exactly reverse the changes
    /// performed in `up()`.
    ///
    /// A call to this method is **not** required.
    ///
    /// # Example
    ///
    /// ```not_test
    /// use surrealdb_migration::M;
    ///
    /// M::up("DEFINE TABLE animal; DEFINE FIELD name FOR animal TYPE string;")
    ///     .down("REMOVE TABLE animal;");
    /// ```
    #[must_use]
    pub const fn down(mut self, sql: &'a str) -> Self {
        self.down = Some(sql);
        self
    }

    /// Add a comment to the schema update
    #[must_use]
    pub const fn comment(mut self, comment: &'a str) -> Self {
        self.comment = Some(comment);
        self
    }

    /// Generate a sha256 checksum based on the up sql
    #[must_use]
    pub fn checksum(&self) -> String {
        sha256::digest(format!("{}:{}", self.up, self.down.unwrap_or_default()))
    }
}

impl cmp::PartialOrd for SchemaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_usize: usize = self.into();
        let other_usize: usize = other.into();

        self_usize.partial_cmp(&other_usize)
    }
}

/// Set of migrations
#[derive(Debug)]
pub struct Migrations<'a> {
    scope: &'static str,
    ms: Vec<M<'a>>,
}

impl<'a> Migrations<'a> {
    /// Create a set of migrations.
    ///
    /// # Example
    ///
    /// ```no_test
    /// use surrealdb_migration::{Migrations, M};
    ///
    /// let migrations = Migrations::new("root", vec![
    ///     M::up("DEFINE TABLE user; DEFINE FIELD username ON user TYPE string;"),
    ///     M::up("DEFINE FIELD password ON user TYPE string;"),
    /// ]);
    /// ```
    #[must_use]
    pub const fn new(scope: &'static str, ms: Vec<M<'a>>) -> Self {
        Migrations { scope, ms }
    }

    /// Migrate the database to latest schema version. The migrations are applied atomically.
    ///
    /// # Example
    ///
    /// ```no_test
    /// use surrealdb_migration::{Migrations, M};
    ///
    /// let db = surrealdb::engine::any::connect("file://data.db");
    ///
    /// let migrations = Migrations::new(vec![
    ///     M::up("DEFINE TABLE user; DEFINE FIELD username ON user TYPE string;"),
    ///     M::up("DEFINE FIELD password ON user TYPE string;"),
    /// ]);
    ///
    /// // Go to the latest version
    /// migrations.to_latest(&db).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error::MigrationDefinition`] if no migration is defined.
    pub async fn to_latest<C: Connection>(&self, db: &Surreal<C>) -> Result<()> {
        self.ensure_migrations_table(db).await?;
        let v_max = self.max_schema_version();
        match v_max {
            SchemaVersion::NoneSet => {
                warn!("No migration defined");
                Err(Error::MigrationDefinition(
                    MigrationDefinitionError::NoMigrationsDefined,
                ))
            }
            SchemaVersion::Inside(v) => {
                info!("some migrations defined (version: {v}), try to migrate");
                self.goto(db, v_max.into()).await
            }
            SchemaVersion::Outside(_) => unreachable!(),
        }
    }

    /// Migrate the database to a given schema version. The migrations are applied atomically.
    ///
    /// # Specifying versions
    ///
    /// - Empty database (no migrations run yet) has version `0`.
    /// - The version increases after each migration, so after the first migration has run, the schema version is `1`. For instance, if there are 3 migrations, version `3` is after all migrations have run.
    ///
    /// *Note*: As a result, the version is the index in the migrations vector *starting from 1*.
    ///
    /// # Example
    ///
    /// ```no_test
    /// use surrealdb_migration::{Migrations, M};
    ///
    /// let db = surrealdb::engine::any::connect("file://data.db");
    /// let migrations = Migrations::new(vec![
    ///     // 0: version 0, before having run any migration
    ///     M::up("DEFINE TABLE animal; DEFINE FIELD name on animal TYPE string;").down("REMOVE TABLE animal;"),
    ///     // 1: version 1, after having created the “animals” table
    ///     M::up("DEFINE TABLE food; DEFINE FIELD name on food TYPE string;").down("REMOVE TABLE food;"),
    ///     // 2: version 2, after having created the food table
    /// ]);
    ///
    /// migrations.to_latest(&db).await?; // Create all tables
    ///
    /// // Go back to version 1, i.e. after running the first migration
    /// migrations.to_version(&db, 1).await?;
    /// db.query("INSERT INTO animal { name: 'dog' }").await?.check()?;
    /// db.query("INSERT INTO food { name: 'carrot' }").await?.check()?;
    ///
    /// // Go back to an empty database
    /// migrations.to_version(&db, 0).await?;
    /// db.query("INSERT INTO animal { name: 'cat' }").await?.check()?;
    /// db.query("INSERT INTO food { name: 'milk' }").await?.check()?;
    /// ```
    ///
    /// # Errors
    ///
    /// Attempts to migrate to a higher version than is supported will result in an error.
    ///
    /// When migrating downwards, all the reversed migrations must have a `.down()` variant,
    /// otherwise no migrations are run and the function returns an error.
    pub async fn to_version<C: Connection>(&self, db: &Surreal<C>, version: usize) -> Result<()> {
        let target_version: SchemaVersion = self.db_version_to_schema(version);
        let v_max = self.max_schema_version();
        match v_max {
            SchemaVersion::NoneSet => {
                warn!("no migrations defined");
                Err(Error::MigrationDefinition(
                    MigrationDefinitionError::NoMigrationsDefined,
                ))
            }
            SchemaVersion::Inside(v) => {
                debug!("some migrations defined (version: {v}), try to migrate");
                if target_version > v_max {
                    warn!("specified version is higher than the max supported version");
                    return Err(Error::SpecifiedSchemaVersion(
                        SchemaVersionError::TargetVersionOutOfRange {
                            specified: target_version,
                            highest: v_max,
                        },
                    ));
                }

                self.goto(db, target_version.into()).await
            }
            SchemaVersion::Outside(_) => unreachable!(),
        }
    }

    const fn db_version_to_schema(&self, db_version: usize) -> SchemaVersion {
        match db_version {
            0 => SchemaVersion::NoneSet,
            v if v > 0 && v <= self.ms.len() => SchemaVersion::Inside(
                NonZeroUsize::new(v).expect("schema version should not be equal to 0"),
            ),
            v => SchemaVersion::Outside(
                NonZeroUsize::new(v).expect("schema version should not be equal to 0"),
            ),
        }
    }

    async fn ensure_migrations_table<C: Connection>(&self, db: &Surreal<C>) -> Result<()> {
        info!("Ensuring _migrations table");

        db.query(
            surrql!("
    DEFINE TABLE IF NOT EXISTS _migrations SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS scope                      ON _migrations TYPE string;
    DEFINE FIELD IF NOT EXISTS version                    ON _migrations TYPE number;
    DEFINE FIELD IF NOT EXISTS comment                    ON _migrations TYPE string;
    DEFINE FIELD IF NOT EXISTS checksum                   ON _migrations TYPE string;
    DEFINE FIELD IF NOT EXISTS installed_on               ON _migrations TYPE datetime;
    DEFINE INDEX IF NOT EXISTS _migrations_version_idx    ON TABLE _migrations COLUMNS scope, version UNIQUE;"),
        )
        .await.and_then(surrealdb::Response::check).map_err(|e| Error::from(("while ensuring _migrations table exists", e)))?;

        info!("_migrations table defined");

        Ok(())
    }

    /// Go to a given db version
    async fn goto<C: Connection>(&self, db: &Surreal<C>, target_db_version: usize) -> Result<()> {
        self.ensure_migrations_table(db).await?;
        let current_version = get_current_version(db, self.scope).await.map_err(|e| {
            Error::from((
                format!("while getting the current version of scope {}", self.scope),
                e,
            ))
        })?;

        let res = match target_db_version.cmp(&current_version) {
            Ordering::Less => {
                if current_version > self.ms.len() {
                    return Err(Error::MigrationDefinition(
                        MigrationDefinitionError::DatabaseTooFarAhead,
                    ));
                }
                info!(
                    "rollback to older version requested, target_db_version: {}, current_version: {}",
                    target_db_version, current_version
                );
                self.goto_down(db, current_version, target_db_version).await
            }
            Ordering::Equal => {
                info!("no migration to run, db already up to date");
                return Ok(()); // return directly, so the migration message is not printed
            }
            Ordering::Greater => {
                info!(
                    "some migrations to run, target: {target_db_version}, current: {current_version}"
                );
                self.goto_up(db, current_version, target_db_version).await
            }
        };

        if res.is_ok() {
            info!("Database migrated to version {}", target_db_version);
        }

        res
    }

    /// Migrate upward methods. This is rolled back on error.
    /// On success, returns the number of update performed
    /// All versions are db versions
    async fn goto_up<C: Connection>(
        &self,
        db: &Surreal<C>,
        current_version: usize,
        target_version: usize,
    ) -> Result<()> {
        debug_assert!(current_version <= target_version);
        debug_assert!(target_version <= self.ms.len());

        trace!("start migration");

        let mut queries = db.query("BEGIN;");

        for v in current_version..target_version {
            let m = &self.ms[v];
            info!("Running: v{} {}", v + 1, m.comment.unwrap_or_default());
            debug!("{}", m.up);

            queries = queries
                .query(m.up)
                .query(format!(
                    r"
                INSERT INTO _migrations {{
                    scope: $scope,
                    version: $version_{v},
                    comment: $comment_{v},
                    checksum: $checksum_{v},
                    installed_on: time::now()
                }};
                "
                ))
                .bind(("scope", self.scope))
                .bind((format!("version_{v}"), v + 1))
                .bind((
                    format!("comment_{v}"),
                    m.comment.unwrap_or_default().to_owned(),
                ))
                .bind((format!("checksum_{v}"), m.checksum()));
        }

        let mut response = queries.query("COMMIT;").await?;
        let number_of_statements = response.num_statements();
        response.take_errors().into_iter().next().map_or_else(
            || {
                trace!("committed migration transaction");
                Ok(())
            },
            |(i, err)| {
                let err = Error::SurrealdbErrorDuringMigration {
                    scope: self.scope,
                    i,
                    number_of_statements,
                    current_version,
                    target_version,
                    err,
                };
                error!("{}", err.to_string());
                Err(err)
            },
        )
    }

    /// Migrate downward. This is rolled back on error.
    /// All versions are db versions
    async fn goto_down<C: Connection>(
        &self,
        db: &Surreal<C>,
        current_version: usize,
        target_version: usize,
    ) -> Result<()> {
        debug_assert!(current_version >= target_version);
        debug_assert!(target_version <= self.ms.len());

        // First, check if all the migrations have a "down" version
        if let Some((i, bad_m)) = self
            .ms
            .iter()
            .enumerate()
            .skip(target_version)
            .take(current_version - target_version)
            .find(|(_, m)| m.down.is_none())
        {
            warn!("Cannot revert: {bad_m:?}");
            return Err(Error::MigrationDefinition(
                MigrationDefinitionError::DownNotDefined { migration_index: i },
            ));
        }

        trace!("start migration transaction");

        let mut queries = db.query("BEGIN;");

        for v in (target_version..current_version).rev() {
            let m = &self.ms[v];
            if let Some(down) = m.down {
                info!("Running: v{} {}", v + 1, m.comment.unwrap_or_default());

                queries = queries
                    .query(down)
                    .query(format!(
                        r"DELETE _migrations WHERE scope=$scope AND version=$version_{v};"
                    ))
                    .bind(("scope", self.scope))
                    .bind((format!("version_{v}"), v + 1));
            } else {
                unreachable!();
            }
        }

        let mut response = queries.query("COMMIT;").await?;
        let number_of_statements = response.num_statements();
        response.take_errors().into_iter().next().map_or_else(
            || {
                trace!("committed migration transaction");
                Ok(())
            },
            |(i, err)| {
                let err = Error::SurrealdbErrorDuringMigration {
                    scope: self.scope,
                    i,
                    number_of_statements,
                    current_version,
                    target_version,
                    err,
                };
                error!("{}", err.to_string());
                Err(err)
            },
        )
    }

    /// Maximum version defined in the migration set
    const fn max_schema_version(&self) -> SchemaVersion {
        match self.ms.len() {
            0 => SchemaVersion::NoneSet,
            v => SchemaVersion::Inside(
                NonZeroUsize::new(v).expect("schema version should not be equal to 0"),
            ),
        }
    }
}

// Read user version field from the db
async fn get_current_version<C: Connection>(
    db: &Surreal<C>,
    scope: &'static str,
) -> Result<usize, surrealdb::Error> {
    let mut result = db
        .query(
            r"SELECT version FROM _migrations WHERE scope = $scope ORDER BY version DESC LIMIT 1",
        )
        .bind(("scope", scope))
        .await?
        .check()?;

    let query_result: Option<usize> = result.take((0, "version"))?;
    Ok(query_result.unwrap_or_default())
}

/// Schema version, in the context of Migrations
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SchemaVersion {
    /// No schema version set
    NoneSet,
    /// The current version in the database is inside the range of defined
    /// migrations
    Inside(NonZeroUsize),
    /// The current version in the database is outside any migration defined
    Outside(NonZeroUsize),
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoneSet => write!(f, "0 (no version set)"),
            Self::Inside(v) => write!(f, "{v} (inside)"),
            Self::Outside(v) => write!(f, "{v} (outside)"),
        }
    }
}

impl From<&SchemaVersion> for usize {
    /// Translate schema version to db version
    fn from(schema_version: &SchemaVersion) -> Self {
        match schema_version {
            SchemaVersion::NoneSet => 0,
            SchemaVersion::Inside(v) | SchemaVersion::Outside(v) => From::from(*v),
        }
    }
}

impl From<SchemaVersion> for usize {
    fn from(schema_version: SchemaVersion) -> Self {
        From::from(&schema_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde::Deserialize;
    use serde_json::Value;
    use surrealdb::sql::{Datetime, Thing};

    #[derive(Debug, Deserialize)]
    #[allow(unused)]
    pub struct MigrationRow {
        pub id: Thing,
        pub scope: String,
        pub version: usize,
        pub comment: String,
        pub checksum: String,
        pub installed_on: Datetime,
    }

    #[tokio::test]
    async fn empty_db_should_have_version_0() -> Result<()> {
        let db = surrealdb::engine::any::connect("mem://").await?;
        db.use_ns("test").use_db("test").await?;
        let version = get_current_version(&db, "test_scope").await?;
        assert_eq!(version, 0);
        Ok(())
    }

    #[tokio::test]
    async fn fail_with_no_migrations_defined_when_no_migrations() -> Result<()> {
        let db = surrealdb::engine::any::connect("mem://").await?;
        db.use_ns("test").use_db("test").await?;
        let migrations = Migrations::new("test_scope", vec![]);
        let result = migrations.to_latest(&db).await;
        matches!(
            result,
            Err(Error::MigrationDefinition(
                MigrationDefinitionError::NoMigrationsDefined
            ))
        );
        Ok(())
    }

    #[tokio::test]
    async fn empty_migrations_table_is_created_when_run_migrations() -> Result<()> {
        let db = surrealdb::engine::any::connect("mem://").await?;
        db.use_ns("test").use_db("test").await?;
        let migrations = Migrations::new("test_scope", vec![]);
        let _ = migrations.to_latest(&db).await;
        let mut result = db.query("INFO FOR TABLE _migrations;").await?.check()?;
        let result: Vec<Value> = result.take((0, "fields"))?;
        assert_eq!(
            &result[0]["checksum"],
            "DEFINE FIELD checksum ON _migrations TYPE string PERMISSIONS FULL"
        );
        assert_eq!(
            &result[0]["comment"],
            "DEFINE FIELD comment ON _migrations TYPE string PERMISSIONS FULL"
        );
        assert_eq!(
            &result[0]["installed_on"],
            "DEFINE FIELD installed_on ON _migrations TYPE datetime PERMISSIONS FULL"
        );
        assert_eq!(
            &result[0]["version"],
            "DEFINE FIELD version ON _migrations TYPE number PERMISSIONS FULL"
        );

        let mut result = db.query("SELECT count() from _migrations").await?.check()?;
        let query_result: Option<u64> = result.take((0, "count"))?;
        assert_eq!(query_result.unwrap_or_default(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn run_to_latest() -> Result<()> {
        let db = surrealdb::engine::any::connect("mem://").await?;
        db.use_ns("test").use_db("test").await?;
        let migrations = Migrations::new("test_scope", vec![
            M::up("DEFINE TABLE animal SCHEMAFULL; DEFINE FIELD name ON animal TYPE string; DEFINE FIELD created_at ON animal TYPE datetime DEFAULT time::now()")
                .comment("Create animal table"),
            M::up("INSERT INTO animal { name: 'dog' };"),
            M::up("INSERT INTO animal { name: 'cat' };"),
        ]);
        migrations.to_latest(&db).await?;

        let mut result = db
            .query("SELECT * from _migrations ORDER BY version")
            .await?
            .check()?;
        let query_result: Vec<MigrationRow> = result.take(0)?;

        assert_eq!(query_result.len(), 3);

        assert_eq!(query_result[0].version, 1);
        assert_eq!(query_result[0].comment, "Create animal table");
        assert_eq!(
            query_result[0].checksum,
            "1bd55fce0e19a65fa868aba24e44a361713e6be5fe1a28d84fae0386a2781edd"
        );

        assert_eq!(query_result[1].version, 2);
        assert_eq!(query_result[1].comment, "");
        assert_eq!(
            query_result[1].checksum,
            "59da625a9fe83055fbb5cd18ba1fdf0e4beebf64e13233d3252bc1b617493abc"
        );

        assert_eq!(query_result[2].version, 3);
        assert_eq!(query_result[2].comment, "");
        assert_eq!(
            query_result[2].checksum,
            "33e6cf4b20d211292ac1dab30f199fe290c66221ce10f31fc03d026f965b9c35"
        );

        let mut result = db
            .query("SELECT name, created_at from animal ORDER BY created_at")
            .await?
            .check()?;
        let query_result: Vec<String> = result.take((0, "name"))?;

        assert_eq!(query_result.len(), 2);
        assert_eq!(query_result[0], "dog");
        assert_eq!(query_result[1], "cat");

        // run 2nd migration adding horse
        let migrations = Migrations::new(
            "test_scope",
            vec![
                M::up("DEFINE TABLE animal SCHEMAFULL; DEFINE FIELD name ON animal TYPE string;")
                    .comment("Create animal table"),
                M::up("INSERT INTO animal { name: 'dog' };"),
                M::up("INSERT INTO animal { name: 'cat' };"),
                M::up("INSERT INTO animal { name: 'horse' };"),
            ],
        );
        migrations.to_latest(&db).await?;

        let mut result = db
            .query("SELECT * from _migrations ORDER BY version")
            .await?
            .check()?;
        let query_result: Vec<MigrationRow> = result.take(0)?;

        assert_eq!(query_result.len(), 4);

        assert_eq!(query_result[0].version, 1);
        assert_eq!(query_result[0].comment, "Create animal table");
        assert_eq!(
            query_result[0].checksum,
            "1bd55fce0e19a65fa868aba24e44a361713e6be5fe1a28d84fae0386a2781edd"
        );

        assert_eq!(query_result[1].version, 2);
        assert_eq!(query_result[1].comment, "");
        assert_eq!(
            query_result[1].checksum,
            "59da625a9fe83055fbb5cd18ba1fdf0e4beebf64e13233d3252bc1b617493abc"
        );

        assert_eq!(query_result[2].version, 3);
        assert_eq!(query_result[2].comment, "");
        assert_eq!(
            query_result[2].checksum,
            "33e6cf4b20d211292ac1dab30f199fe290c66221ce10f31fc03d026f965b9c35"
        );

        assert_eq!(query_result[3].version, 4);
        assert_eq!(query_result[3].comment, "");
        assert_eq!(
            query_result[3].checksum,
            "3d7a82ff33fae0322040f40cc3b93fdc6539c4e04e7d6153da5377f9d3c3408a"
        );

        let mut result = db
            .query("SELECT name, created_at from animal ORDER BY created_at")
            .await?
            .check()?;
        let query_result: Vec<String> = result.take((0, "name"))?;

        assert_eq!(query_result.len(), 3);
        assert_eq!(query_result[0], "dog");
        assert_eq!(query_result[1], "cat");
        assert_eq!(query_result[2], "horse");

        Ok(())
    }

    #[tokio::test]
    async fn run_to_version() -> Result<()> {
        let db = surrealdb::engine::any::connect((
            "mem://",
            surrealdb::opt::Config::new()
                .set_strict(true)
                .capabilities(surrealdb::opt::capabilities::Capabilities::all()),
        ))
        .await?;

        db.query("DEFINE NAMESPACE test; USE NAMESPACE test; DEFINE DATABASE test;")
            .await?
            .check()?;

        db.use_ns("test").use_db("test").await?;
        let migrations = Migrations::new("test_scope", vec![
            // 0: version 0, before having run any migration
            M::up("DEFINE TABLE animal SCHEMAFULL; DEFINE FIELD name ON animal TYPE string; DEFINE FIELD created_at ON animal TYPE datetime DEFAULT time::now()")
                .down("REMOVE TABLE animal;")
                .comment("Create animal table"),

            // 1: version 1, after having created the “animals” table
            M::up("DEFINE TABLE food SCHEMAFULL; DEFINE FIELD name ON food TYPE string; DEFINE FIELD created_at ON food TYPE datetime DEFAULT time::now()")
                .down("REMOVE TABLE food;")
                .comment("Create food table"),
            // 2: version 2, after having created the food table
        ]);

        // create all tables
        migrations.to_latest(&db).await?;

        let version = get_current_version(&db, "test_scope").await?;
        assert_eq!(version, 2);

        // Go back to version 1, i.e. after running the first migration
        migrations.to_version(&db, 1).await?;

        let version = get_current_version(&db, "test_scope").await?;
        assert_eq!(version, 1);

        let mut result = db.query("SELECT count() from animal").await?.check()?;
        let query_result: Option<u64> = result.take((0, "count"))?;
        assert_eq!(query_result.unwrap_or_default(), 0);

        db.query("INSERT INTO animal { name: 'dog' }")
            .await?
            .check()?;

        let mut result = db.query("SELECT count() from animal").await?.check()?;
        let query_result: Option<u64> = result.take((0, "count"))?;
        assert_eq!(query_result.unwrap_or_default(), 1);

        let result = db
            .query("INSERT INTO food { name: 'carrot' }")
            .await?
            .check();

        match result {
            Err(surrealdb::Error::Db(surrealdb::error::Db::TbNotFound { name })) => {
                assert_eq!(name, "food")
            }
            _ => unreachable!(),
        }

        // Go back to an empty database
        migrations.to_version(&db, 0).await?;

        let version = get_current_version(&db, "test_scope").await?;
        assert_eq!(version, 0);

        let result = db
            .query("INSERT INTO animal { name: 'cat' }")
            .await?
            .check();

        match result {
            Err(surrealdb::Error::Db(surrealdb::error::Db::TbNotFound { name })) => {
                assert_eq!(name, "animal")
            }
            _ => unreachable!(),
        }

        let result = db.query("INSERT INTO food { name: 'milk' }").await?.check();

        match result {
            Err(surrealdb::Error::Db(surrealdb::error::Db::TbNotFound { name })) => {
                assert_eq!(name, "food")
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    #[tokio::test]
    async fn run_to_latest_when_table_already_exists() -> Result<()> {
        let db = surrealdb::engine::any::connect("mem://").await?;
        db.use_ns("test").use_db("test").await?;
        let migrations = Migrations::new("test_scope", vec![
            M::up("DEFINE TABLE OVERWRITE animal SCHEMAFULL; DEFINE FIELD name ON animal TYPE string; DEFINE FIELD created_at ON animal TYPE datetime DEFAULT time::now()")
                .comment("Create animal table"),
            M::up("INSERT INTO animal { name: 'dog' };"),
            M::up("INSERT INTO animal { name: 'cat' };"),
        ]);

        db.query("DEFINE TABLE animal;").await?.check()?;

        // First run
        migrations.to_latest(&db).await?;

        let mut result = db
            .query("SELECT * from _migrations ORDER BY version")
            .await?
            .check()?;
        let query_result: Vec<MigrationRow> = result.take(0)?;

        assert_eq!(query_result.len(), 3);

        Ok(())
    }
}
