//! Create-new, owner-private receipt database; no existing database is opened.

use super::ReceiptError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::{Connection, SqliteConnection};
use std::path::Path;

pub(super) struct Database {
    pub(super) connection: SqliteConnection,
    file: std::fs::File,
}

impl Database {
    pub(super) async fn create(path: &Path) -> Result<Self, ReceiptError> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path).map_err(|_| ReceiptError::Destination)?;
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .journal_mode(SqliteJournalMode::Delete)
            .synchronous(SqliteSynchronous::Full)
            .foreign_keys(true);
        let mut connection = SqliteConnection::connect_with(&options)
            .await
            .map_err(|_| ReceiptError::Storage)?;
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&mut connection)
            .await
            .map_err(|_| ReceiptError::Storage)?;
        Ok(Self { connection, file })
    }

    pub(super) fn sync(&self) -> Result<(), ReceiptError> {
        self.file.sync_all().map_err(|_| ReceiptError::Storage)
    }

    pub(super) async fn close(self) -> Result<(), ReceiptError> {
        self.connection
            .close()
            .await
            .map_err(|_| ReceiptError::Storage)
    }
}
