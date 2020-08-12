//! Reference implementation of a mail service
//! simply delivering mail to server console log.
use crate::common::*;
use crate::model::io::Connection;
use crate::model::mail::*;
use crate::model::Error;
use crate::service::mail::*;

#[derive(Clone)]
pub struct DefaultMailService;

impl NamedService for DefaultMailService {
    fn name(&self) -> &str {
        "samotop"
    }
}

impl EsmtpService for DefaultMailService {
    fn extend(&self, connection: &mut Connection) {
        connection
            .extensions_mut()
            .enable(SmtpExtension::EIGHTBITMIME);
    }
}

impl MailGuard for DefaultMailService {
    type Future = futures::future::Ready<AcceptRecipientResult>;
    fn accept(&self, request: AcceptRecipientRequest) -> Self::Future {
        println!("Accepting recipient {:?}", request);
        future::ready(AcceptRecipientResult::Accepted(request.rcpt))
    }
}

impl MailQueue for DefaultMailService {
    type Mail = MailSink;
    type MailFuture = futures::future::Ready<Option<Self::Mail>>;

    fn mail(&self, envelope: Envelope) -> Self::MailFuture {
        match envelope {
            Envelope {
                ref name,
                peer: Some(ref peer),
                local: Some(ref local),
                helo: Some(ref helo),
                mail: Some(ref mail),
                ref id,
                ref rcpts,
            } if rcpts.len() != 0 => {
                println!(
                    "Mail from {} (helo: {}, mailid: {}) (peer: {}) for {} on {} ({} <- {})",
                    mail.from(),
                    helo.name(),
                    id,
                    peer,
                    rcpts
                        .iter()
                        .fold(String::new(), |s, r| s + format!("{}, ", r).as_ref()),
                    name,
                    local,
                    peer
                );
                future::ready(Some(MailSink { id: id.clone() }))
            }
            envelope => {
                warn!("Incomplete envelope: {:?}", envelope);
                future::ready(None)
            }
        }
    }
}

pub struct MailSink {
    id: String,
}

impl Sink<Bytes> for MailSink {
    type Error = Error;
    fn start_send(self: Pin<&mut Self>, bytes: Bytes) -> Result<()> {
        println!("Mail data for {}: {:?}", self.id, bytes);
        Ok(())
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.poll_ready(cx)
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.poll_flush(cx)
    }
    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}
