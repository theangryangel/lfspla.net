use super::error::Error;
use crate::api::extractors::pubstat::{Compression, Format};
use axum::{
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use lfsplanet_pubstat::ser::xml::XmlRoot;
use std::io::Write;

pub(crate) fn respond(
    format: Format,
    compression: Compression,
    value: &impl XmlRoot,
) -> Result<Response, Error> {
    let (mut content_type, bytes) = match format {
        Format::Text => (
            "text/plain; charset=utf-8",
            lfsplanet_pubstat::ser::to_string(value)
                .map_err(|error| Error::internal(&error))?
                .into_bytes(),
        ),
        Format::Json => (
            "application/json",
            serde_json::to_vec(value).map_err(|error| Error::internal(&error))?,
        ),
        Format::Xml => (
            "text/xml; charset=utf-8",
            lfsplanet_pubstat::ser::xml::to_string(value)
                .map_err(|error| Error::internal(&error))?
                .into_bytes(),
        ),
        Format::Php => return Err(Error::unsupported("PHP output is not implemented")),
    };

    let body = match compression {
        Compression::None => bytes,
        Compression::Gzip => {
            let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder
                .write_all(&bytes)
                .map_err(|error| Error::internal(&error))?;
            encoder.finish().map_err(|error| Error::internal(&error))?
        }
        Compression::Zlib => {
            let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            encoder
                .write_all(&bytes)
                .map_err(|error| Error::internal(&error))?;
            encoder.finish().map_err(|error| Error::internal(&error))?
        }
        Compression::Deflate => {
            let mut encoder = DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            encoder
                .write_all(&bytes)
                .map_err(|error| Error::internal(&error))?;
            encoder.finish().map_err(|error| Error::internal(&error))?
        }
    };

    if matches!(compression, Compression::Zlib | Compression::Deflate) {
        // These are binary protocol payloads for the client to inflate, rather
        // than HTTP content codings. The existing compression predicate excludes
        // octet-stream responses, preventing a second negotiated encoding.
        content_type = "application/octet-stream";
    }
    let mut response = ([(header::CONTENT_TYPE, content_type)], body).into_response();
    if compression == Compression::Gzip {
        // Tower also skips bodies with an existing Content-Encoding header.
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    }
    Ok(response)
}
