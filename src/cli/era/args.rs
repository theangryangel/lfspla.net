//! Arguments for era policy operator commands.

use std::path::PathBuf;

use clap::Subcommand;

/// Era policy operation.
#[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum EraCommand {
    /// List configured eras.
    List,
    /// Write one era definition as YAML to stdout.
    Export {
        /// Stable era identifier.
        id: String,
    },
    /// Create or replace one or more eras from YAML; use - to read stdin.
    Apply {
        /// Era YAML paths, or - for stdin.
        #[arg(value_name = "FILE", required = true)]
        files: Vec<PathBuf>,
        /// Confirm replacement of an existing era.
        #[arg(long)]
        yes: bool,
    },
    /// Delete an era with no stored references.
    Delete {
        /// Stable era identifier.
        id: String,
        /// Confirm deletion.
        #[arg(long)]
        yes: bool,
    },
    /// Open an era for hotlap uploads.
    Open {
        /// Stable era identifier.
        id: String,
    },
    /// Close an era to new hotlap uploads.
    Close {
        /// Stable era identifier.
        id: String,
    },
}
