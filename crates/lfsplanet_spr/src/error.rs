use std::io::ErrorKind;

use thiserror::Error;

/// An error encountered while reading an SPR file.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    /// The replay stream contains both the test-drive player name and plate.
    #[error("test-drive replays are not accepted")]
    TestDrive,

    /// The file does not start with the `LFSSPR` signature.
    #[error("not an SPR file: expected LFSSPR, found {found:?}")]
    InvalidMagic { found: [u8; 6] },

    /// Reading the header or replay stream failed.
    #[error("I/O error: {kind}: {message}")]
    Io { kind: ErrorKind, message: String },

    /// A header field could not be decoded or contained an invalid value.
    #[error("invalid SPR header: {0}")]
    Decode(#[from] insim_core::DecodeError),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io {
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}
