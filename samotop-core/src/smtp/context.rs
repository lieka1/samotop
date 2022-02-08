use crate::{smtp::SmtpSession, config::Store};

#[derive(Debug)]
pub struct SmtpContext<'a> {
    /// Implementation-specific value store
    pub store: &'a mut Store,
    pub session: &'a mut SmtpSession,
}

impl<'a> SmtpContext<'a> {
    pub fn new(store: &'a mut Store, session: &'a mut SmtpSession) -> Self {
        SmtpContext { session, store }
    }
}

/// Represents the instructions for the client side of the stream.
#[derive(Clone, Eq, PartialEq)]
pub enum DriverControl {
    /// Write an SMTP response
    Response(Vec<u8>),
    /// Start TLS encryption
    StartTls,
    /// Shut the stream down
    Shutdown,
}

impl std::fmt::Debug for DriverControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        enum TextOrBytes<'a> {
            T(&'a str),
            B(&'a [u8]),
        }
        fn tb(inp: &[u8]) -> TextOrBytes {
            if let Ok(text) = std::str::from_utf8(inp) {
                TextOrBytes::T(text)
            } else {
                TextOrBytes::B(inp)
            }
        }
        match self {
            DriverControl::Response(r) => f.debug_tuple("Response").field(&tb(r)).finish(),
            DriverControl::StartTls => f.debug_tuple("StartTls").finish(),
            DriverControl::Shutdown => f.debug_tuple("Shutdown").finish(),
        }
    }
}
