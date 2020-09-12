//! Reference implementation of a mail service
//! simply delivering mail to server console log.
use crate::common::*;
use crate::model::mail::*;
use crate::service::mail::*;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct DefaultMailService;

impl NamedService for DefaultMailService {
    fn name(&self) -> &str {
        "samotop"
    }
}

impl EsmtpService for DefaultMailService {
    fn extend(&self, _connection: &mut SessionInfo) {}
}

impl MailGuard for DefaultMailService {
    type RecipientFuture = futures::future::Ready<AddRecipientResult>;
    type SenderFuture = futures::future::Ready<StartMailResult>;
    fn add_recipient(&self, request: AddRecipientRequest) -> Self::RecipientFuture {
        let AddRecipientRequest { mut envelope, rcpt } = request;
        envelope.rcpts.push(rcpt);
        future::ready(AddRecipientResult::Accepted(envelope))
    }
    fn start_mail(&self, mut request: StartMailRequest) -> Self::SenderFuture {
        if request.id.is_empty() {
            request.id = Uuid::new_v4().to_string();
        }
        future::ready(StartMailResult::Accepted(request))
    }
}

impl MailQueue for DefaultMailService {
    type Mail = MailSink;
    type MailFuture = futures::future::Ready<Option<Self::Mail>>;

    fn mail(&self, envelope: Envelope) -> Self::MailFuture {
        let Envelope {
            ref session,
            ref mail,
            ref id,
            ref rcpts,
        } = envelope;
        println!(
            "Mail from {:?} for {} (mailid: {:?}). {}",
            mail.as_ref()
                .map(|m| m.from().to_string())
                .unwrap_or("nobody".to_owned()),
            rcpts
                .iter()
                .fold(String::new(), |s, r| s + format!("{:?}, ", r.to_string())
                    .as_ref()),
            id,
            session
        );
        future::ready(Some(MailSink { id: id.clone() }))
    }
}

pub struct MailSink {
    id: String,
}

impl Write for MailSink {
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        println!("Mail data for {}: {:?}", self.id, buf);
        Poll::Ready(Ok(buf.len()))
    }
}
