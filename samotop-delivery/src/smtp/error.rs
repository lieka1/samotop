//! Error and result type for SMTP clients

use self::Error::*;
use crate::smtp::response::{Response, Severity};
use base64::DecodeError;
use std::io;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

/// An enum of all error kinds.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Transient SMTP error, 4xx reply code
    ///
    /// [RFC 5321, section 4.2.1](https://tools.ietf.org/html/rfc5321#section-4.2.1)
    #[error("transient: {}", .0.first_line().unwrap_or("undetailed error during SMTP transaction"))]
    Transient(Response),
    /// Permanent SMTP error, 5xx reply code
    ///
    /// [RFC 5321, section 4.2.1](https://tools.ietf.org/html/rfc5321#section-4.2.1)
    #[error("permanent: {}", .0.first_line().unwrap_or("undetailed error during SMTP transaction"))]
    Permanent(Response),
    /// Error parsing a response
    #[error("{0}")]
    ResponseParsing(&'static str),
    /// Error parsing a base64 string in response
    #[error("challenge parsing: {0}")]
    ChallengeParsing(#[from] DecodeError),
    /// Error parsing UTF8in response
    #[error("utf8-string: {0}")]
    Utf8Parsing(#[from] FromUtf8Error),
    /// Error parsing UTF8in response
    #[error("utf8-slice: {0}")]
    Utf8(#[from] Utf8Error),
    /// Internal client error
    #[error("client: {0}")]
    Client(&'static str),
    /// DNS resolution error
    #[error("could not resolve hostname")]
    Resolution,
    /// IO error
    #[error("io: {0}")]
    Io(#[from] io::Error),
    /// Native TLS error
    #[cfg(feature = "native-tls")]
    #[error("tls: {0}")]
    NativeTls(#[from] async_native_tls::Error),
    /// Parsing error
    #[error("parsing: {0:?}")]
    Parsing(nom::error::ErrorKind),
    #[error("timeout: {0}")]
    Timeout(#[from] async_std::future::TimeoutError),
    #[error("no stream")]
    NoStream,
    #[error("no server info")]
    NoServerInfo,
}

impl From<nom::Err<nom::error::Error<&str>>> for Error {
    fn from(err: nom::Err<nom::error::Error<&str>>) -> Error {
        Parsing(match err {
            nom::Err::Incomplete(_) => nom::error::ErrorKind::Complete,
            nom::Err::Failure(e) => e.code,
            nom::Err::Error(e) => e.code,
        })
    }
}

impl From<Response> for Error {
    fn from(response: Response) -> Error {
        match response.code.severity {
            Severity::TransientNegativeCompletion => Transient(response),
            Severity::PermanentNegativeCompletion => Permanent(response),
            _ => {
                warn!("Unknown response code: {}, {:?}", response.code, response);
                Error::Transient(response)
            }
        }
    }
}

impl From<&'static str> for Error {
    fn from(string: &'static str) -> Error {
        Client(string)
    }
}

/// SMTP result type
pub type SmtpResult = Result<Response, Error>;
