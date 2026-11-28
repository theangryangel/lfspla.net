//! Serde serialization to Pubstat plain text and XML.
//!
//! The Serde adapter validates the row structure; the text writer owns field
//! separators, line endings, scalar formatting, and string restrictions.
mod error;
mod serializer;
#[cfg(test)]
mod test_support;
mod text;
pub mod xml;

pub use error::Error;
use serde::Serialize;
use std::io::Write;

/// Serializes complete rows to a writer. Errors may leave partially written output.
pub fn to_writer<W: Write, T: Serialize + ?Sized>(writer: W, value: &T) -> Result<(), Error> {
    value.serialize(&mut serializer::Serializer::new(writer))
}

pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String, Error> {
    let mut bytes = Vec::new();
    to_writer(&mut bytes, value)?;
    String::from_utf8(bytes).map_err(|error| Error::Message(error.to_string()))
}
