pub use super::commands::*;
use super::ReadControl;
use crate::{common::*, smtp::session::SmtpState};
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

pub trait SmtpSessionCommand {
    fn verb(&self) -> &str;
    #[must_use = "apply must be awaited"]
    fn apply<'s, 'f, S>(self, state: S) -> S2Fut<'f, S>
    where
        S: SmtpState + 's,
        's: 'f;
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum SmtpCommand {
    StartTls,
    Helo(SmtpHelo),
    Mail(SmtpMail),
    Rcpt(SmtpPath),
    Expn(String),
    Vrfy(String),
    Help(Vec<String>),
    Noop(Vec<String>),
    Quit,
    Rset,
    Data,
    Turn,
    /// Command outside of the base implementation.
    /// First string is the command verb, next the parameters
    Other(String, Vec<String>),
}

impl SmtpSessionCommand for SmtpCommand {
    fn verb(&self) -> &str {
        use SmtpCommand as C;
        match self {
            C::Helo(helo) => helo.verb(),
            C::Mail(mail) => mail.verb(),
            C::Rcpt(_) => "RCPT",
            C::Data => SmtpData.verb(),
            C::Quit => SmtpQuit.verb(),
            C::Rset => SmtpRset.verb(),
            C::Noop(_) => SmtpNoop.verb(),
            C::StartTls => StartTls.verb(),
            C::Expn(_) => "EXPN",
            C::Vrfy(_) => "VRFY",
            C::Help(_) => "HELP",
            C::Turn => "TURN",
            C::Other(verb, _) => verb.as_str(),
        }
    }

    fn apply<'s, 'f, S>(self, data: S) -> S2Fut<'f, S>
    where
        S: SmtpState + 's,
        's: 'f,
    {
        use SmtpCommand as C;
        match self {
            C::Helo(helo) => helo.apply(data),
            C::Mail(mail) => mail.apply(data),
            C::Rcpt(path) => SmtpRcpt::from(path).apply(data),
            C::Data => SmtpData.apply(data),
            C::Quit => SmtpQuit.apply(data),
            C::Rset => SmtpRset.apply(data),
            C::Noop(_) => SmtpNoop.apply(data),
            C::StartTls => StartTls.apply(data),
            C::Expn(_) | C::Vrfy(_) | C::Help(_) | C::Turn | C::Other(_, _) => {
                SmtpUnknownCommand::default().apply(data)
            }
        }
    }
}

impl SmtpSessionCommand for ReadControl {
    fn verb(&self) -> &str {
        match self {
            ReadControl::PeerConnected(sess) => sess.verb(),
            ReadControl::PeerShutdown => SessionShutdown.verb(),
            ReadControl::Raw(_) => "",
            ReadControl::Command(cmd, _) => cmd.verb(),
            ReadControl::MailDataChunk(_) => "",
            ReadControl::EndOfMailData(_) => MailBodyEnd.verb(),
            ReadControl::Empty(_) => "",
            ReadControl::EscapeDot(_) => "",
        }
    }

    fn apply<'s, 'f, S>(self, state: S) -> S2Fut<'f, S>
    where
        S: SmtpState + 's,
        's: 'f,
    {
        match self {
            ReadControl::PeerConnected(sess) => sess.apply(state),
            ReadControl::PeerShutdown => SessionShutdown.apply(state),
            ReadControl::Raw(_) => SmtpInvalidCommand::default().apply(state),
            ReadControl::Command(cmd, _) => cmd.apply(state),
            ReadControl::MailDataChunk(bytes) => MailBodyChunk(bytes).apply(state),
            ReadControl::EndOfMailData(_) => MailBodyEnd.apply(state),
            ReadControl::Empty(_) => Box::pin(ready(state)),
            ReadControl::EscapeDot(_) => Box::pin(ready(state)),
        }
    }
}
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum SmtpHost {
    Domain(String),
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
    Invalid { label: String, literal: String },
    Other { label: String, literal: String },
}

impl SmtpHost {
    pub fn domain(&self) -> String {
        match self {
            SmtpHost::Domain(s) => s.clone(),
            SmtpHost::Ipv4(ip) => format!("{}", ip),
            SmtpHost::Ipv6(ip) => format!("{}", ip),
            SmtpHost::Invalid { label, literal } => format!("{}:{}", label, literal),
            SmtpHost::Other { label, literal } => format!("{}:{}", label, literal),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum SmtpPath {
    Direct(SmtpAddress),
    Relay(Vec<SmtpHost>, SmtpAddress),
    Postmaster,
    Null,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum SmtpAddress {
    Mailbox(String, SmtpHost),
}

impl SmtpPath {
    pub fn address(&self) -> String {
        match *self {
            SmtpPath::Direct(ref addr) => match addr {
                SmtpAddress::Mailbox(ref name, ref host) => format!("{}@{}", name, host),
            },
            SmtpPath::Null => String::new(),
            SmtpPath::Postmaster => "POSTMASTER".to_owned(),
            SmtpPath::Relay(_, ref addr) => match addr {
                SmtpAddress::Mailbox(ref name, ref host) => format!("{}@{}", name, host),
            },
        }
    }
}

impl fmt::Display for SmtpPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<{}>", self.address())
    }
}

impl fmt::Display for SmtpHost {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::SmtpHost::*;
        match *self {
            Domain(ref h) => f.write_str(h),
            Ipv4(ref h) => write!(f, "[{}]", h),
            Ipv6(ref h) => write!(f, "[IPv6:{}]", h),
            Invalid {
                ref label,
                ref literal,
            } => write!(f, "[{}:{}]", label, literal),
            Other {
                ref label,
                ref literal,
            } => write!(f, "[{}:{}]", label, literal),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct SmtpConnection {
    pub local_name: String,
    pub local_addr: Option<SocketAddr>,
    pub peer_addr: Option<SocketAddr>,
}
