//! Private filesystem boundaries for selective name pseudonymization.
//!
//! Command routing and persistent receipts are not exposed until the complete
//! publication protocol is wired. The transform itself remains SQL-free.

// Preserve and exercise the unwired protocol without shipping a dead command path.
#[cfg(test)]
mod receipt;
