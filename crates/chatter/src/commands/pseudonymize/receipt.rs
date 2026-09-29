//! Queryable private evidence coupled to one staged output, never default logs.

use sqlx::Connection;
use std::path::Path;
use talkbank_transform::pseudonymize::PseudonymizedDocument;

use database::Database;
use publication::PendingOutput;

mod database;
mod publication;
mod records;
mod refusal;

/// No SQL, OS or protected-content errors are chained into public diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum ReceiptError {
    #[error("cannot create a new private receipt destination")]
    Destination,
    #[error("cannot persist private receipt evidence")]
    Storage,
    #[error("private output publication failed; inspect the receipt")]
    Publication,
}

/// Publication authority owns BOTH the staged output and its committed receipt.
/// No receipt token can be detached and reused with a different document.
pub(super) struct PreparedPublication {
    output: PendingOutput,
    receipt: Database,
}

impl PreparedPublication {
    /// Stage admitted bytes, then commit their exact edits and report-only
    /// findings before returning publication authority. Existing receipts are
    /// never opened or overwritten. Failed initialization may leave an incomplete
    /// database, which cannot be interpreted as a prepared or written receipt.
    pub(super) async fn prepare(
        document: &PseudonymizedDocument<'_>,
        output: &Path,
        receipt: &Path,
        key: &str,
    ) -> Result<Self, ReceiptError> {
        let staged =
            PendingOutput::stage(document, output).map_err(|_| ReceiptError::Publication)?;
        let output_name = staged
            .destination()
            .to_str()
            .ok_or(ReceiptError::Destination)?;
        let receipt_name = receipt.file_name().ok_or(ReceiptError::Destination)?;
        let receipt_parent = receipt.parent().ok_or(ReceiptError::Destination)?;
        let receipt_parent = if receipt_parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            receipt_parent
        };
        let receipt = receipt_parent
            .canonicalize()
            .map_err(|_| ReceiptError::Destination)?
            .join(receipt_name);
        if receipt == staged.destination() {
            return Err(ReceiptError::Destination);
        }
        let mut database = Database::create(&receipt).await?;
        let mut transaction = database
            .connection
            .begin()
            .await
            .map_err(|_| ReceiptError::Storage)?;
        sqlx::query("INSERT INTO document (id, schema_version, map_key, output_path, source_blake3, output_blake3, status) VALUES (1, 1, ?, ?, ?, ?, 'prepared')")
            .bind(key).bind(output_name)
            .bind(blake3::hash(document.plan().source().as_bytes()).to_hex().as_str())
            .bind(blake3::hash(document.text().as_bytes()).to_hex().as_str())
            .execute(&mut *transaction).await.map_err(|_| ReceiptError::Storage)?;
        records::write(&mut transaction, document).await?;
        transaction
            .commit()
            .await
            .map_err(|_| ReceiptError::Storage)?;
        database.sync()?;
        Ok(Self {
            output: staged,
            receipt: database,
        })
    }

    /// A written receipt is issued only after no-clobber publication and file
    /// synchronization. If the final DB update fails, prepared remains an honest
    /// uncertain state, never an invented rollback of an already installed file.
    pub(super) async fn publish(mut self) -> Result<(), ReceiptError> {
        let output = match self.output.publish() {
            Ok(output) => output,
            Err(_) => {
                sqlx::query("UPDATE document SET status = 'publication_failed' WHERE id = 1")
                    .execute(&mut self.receipt.connection)
                    .await
                    .map_err(|_| ReceiptError::Storage)?;
                return Err(ReceiptError::Publication);
            }
        };
        output.sync().map_err(|_| ReceiptError::Publication)?;
        sqlx::query("UPDATE document SET status = 'written' WHERE id = 1 AND status = 'prepared'")
            .execute(&mut self.receipt.connection)
            .await
            .map_err(|_| ReceiptError::Storage)?;
        self.receipt
            .close()
            .await
            .map_err(|_| ReceiptError::Storage)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
