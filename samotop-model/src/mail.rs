use crate::smtp::*;
use std::net::SocketAddr;

/// Mail envelope before sending mail data
#[derive(Debug, Clone)]
pub struct Envelope {
    /// Service name
    pub name: String,
    /// Local server endpoint
    pub local: Option<SocketAddr>,
    /// Remote peer endpoint
    pub peer: Option<SocketAddr>,
    /// The SMTP helo sent by peer
    pub helo: Option<SmtpHelo>,
    /// The SMTP mail from:path sent by peer
    pub mail: Option<SmtpMail>,
    /// unique mail request identifier
    pub id: String,
    /// A list of SMTP rcpt to:path sent by peer
    pub rcpts: Vec<SmtpPath>,
}

/// Request to check if mail is accepted for given recipient
#[derive(Debug, Clone)]
pub struct AcceptSenderRequest {
    /// Service name
    pub name: String,
    /// Local server endpoint
    pub local: Option<SocketAddr>,
    /// Remote peer endpoint
    pub peer: Option<SocketAddr>,
    /// The SMTP helo sent by peer
    pub helo: Option<SmtpHelo>,
    /// The SMTP mail from:path sent by peer
    pub mail: Option<SmtpMail>,
    /// unique mail request identifier
    pub id: String,
}
#[derive(Debug, Clone)]
pub enum AcceptSenderResult {
    Failed,
    Rejected,
    Accepted,
}

/// Request to check if mail is accepted for given recipient
#[derive(Debug, Clone)]
pub struct AcceptRecipientRequest {
    /// Service name
    pub name: String,
    /// Local server endpoint
    pub local: Option<SocketAddr>,
    /// Remote peer endpoint
    pub peer: Option<SocketAddr>,
    /// The SMTP helo sent by peer
    pub helo: Option<SmtpHelo>,
    /// The SMTP mail from:path sent by peer
    pub mail: Option<SmtpMail>,
    /// unique mail request identifier
    pub id: String,
    /// The SMTP rcpt to:path sent by peer we want to check
    pub rcpt: SmtpPath,
}

#[derive(Debug, Clone)]
pub enum AcceptRecipientResult {
    Failed,
    Rejected,
    RejectedWithNewPath(SmtpPath),
    AcceptedWithNewPath(SmtpPath),
    Accepted(SmtpPath),
}

pub type QueueResult = std::result::Result<(), QueueError>;

#[derive(Debug, Clone)]
pub enum QueueError {
    Refused,
    Failed,
}

impl std::error::Error for QueueError {}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            QueueError::Failed => write!(f, "Mail queue failed temporarily"),
            QueueError::Refused => write!(f, "Mail was refused by the server"),
        }
    }
}