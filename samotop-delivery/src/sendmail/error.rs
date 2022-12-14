//! Error and result type for sendmail transport

use std::string::FromUtf8Error;

use async_std::io;

/// An enum of all error kinds.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Internal client error
    #[error("client error: {0}")]
    Client(String),
    /// Error parsing UTF8in response
    #[error("utf8 error: {0}")]
    Utf8Parsing(#[from] FromUtf8Error),
    /// IO error
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}
