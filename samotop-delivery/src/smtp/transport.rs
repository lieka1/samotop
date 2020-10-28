use crate::smtp::commands::*;
use crate::smtp::error::Error;
use crate::smtp::extension::{Extension, MailBodyParameter, MailParameter, ServerInfo};
use crate::smtp::net::{ConnectionConfiguration, Connector, MaybeTls};
use crate::smtp::smtp_client::ClientSecurity;
use crate::smtp::stream::SmtpDataStream;
use crate::smtp::util::SmtpProto;
use crate::{Envelope, Transport};
use log::{debug, info};
use pin_project::pin_project;
use potential::{Lease, Potential};
use samotop_async_trait::async_trait;
use std::pin::Pin;
use std::time::Duration;

/// Structure that implements the high level SMTP client
#[pin_project]
#[allow(missing_debug_implementations)]
pub struct SmtpTransport<Conf, Conn: Connector> {
    connector: Conn,
    configuration: Conf,
    /// inner state leased to produced futures
    inner: Potential<SmtpConnection<Conn::Stream>>,
}

impl<Conf, Conn: Connector> SmtpTransport<Conf, Conn>
where
    Conf: ConnectionConfiguration,
{
    pub fn new(configuration: Conf, connector: Conn) -> Self {
        SmtpTransport {
            inner: Potential::empty(),
            connector,
            configuration,
        }
    }

    async fn connect(
        configuration: &Conf,
        connector: &Conn,
    ) -> Result<SmtpConnection<Conn::Stream>, Error> {
        let mut stream = connector.connect(configuration).await?;
        let server_info = Self::setup(&configuration, &mut stream).await?;
        let reuse = configuration.max_reuse_count().saturating_add(1);
        Ok(SmtpConnection {
            stream,
            reuse,
            server_info,
        })
    }
    async fn setup(configuration: &Conf, stream: &mut Conn::Stream) -> Result<ServerInfo, Error> {
        let timeout = configuration.timeout();
        let address = configuration.address();
        let my_id = configuration.hello_name();
        let security = configuration.security();

        let mut client = SmtpProto::new(Pin::new(stream));
        let banner = client.read_banner(timeout).await?;

        // Log the connection
        debug!(
            "connection established to {}. Saying: {}",
            address, banner.code
        );

        let (_, mut server_info) = client.execute_ehlo(my_id.clone(), timeout).await?;

        if !client.stream().is_encrypted() {
            let can_encrypt =
                server_info.supports_feature(Extension::StartTls) && client.stream().can_encrypt();

            let encrypt = match security {
                ClientSecurity::Required => true,
                ClientSecurity::Opportunistic => can_encrypt,
                ClientSecurity::Wrapper | ClientSecurity::None => false,
            };
            if encrypt {
                client.execute_starttls(timeout).await?;
                client.stream_mut().encrypt()?;
                server_info = client.execute_ehlo(my_id, timeout).await?.1;
            }
        }

        Self::try_login(configuration, stream, &server_info, timeout).await?;

        Ok(server_info)
    }
    async fn try_login(
        configuration: &Conf,
        stream: &mut Conn::Stream,
        server_info: &ServerInfo,
        timeout: Duration,
    ) -> Result<(), Error> {
        if let Some(auth) = configuration.get_authentication(&server_info, stream.is_encrypted()) {
            let mut client = SmtpProto::new(Pin::new(stream));
            client.authenticate(auth, timeout).await?;
        } else {
            info!("No authentication mechanisms are available");
        }

        Ok(())
    }
    async fn prepare_mail(
        _configuration: &Conf,
        lease: &mut Lease<SmtpConnection<Conn::Stream>>,
        envelope: Envelope,
        timeout: Duration,
    ) -> Result<(), Error> {
        // Mail
        let mut mail_options = vec![];

        if lease.server_info.supports_feature(Extension::EightBitMime) {
            // FIXME: this needs to be gracefully degraded to 7bit if not available
            mail_options.push(MailParameter::Body(MailBodyParameter::EightBitMime));
        }

        if lease.server_info.supports_feature(Extension::SmtpUtfEight) {
            // FIXME: this needs to be gracefully degraded to 7bit if not available
            mail_options.push(MailParameter::SmtpUtfEight);
        }

        let mut client = SmtpProto::new(Pin::new(&mut lease.stream));

        // MAIL FROM:<reverse-path>
        client
            .execute_command(
                MailCommand::new(envelope.from().cloned(), mail_options),
                [250],
                timeout,
            )
            .await?;

        // RCPT TO:<forward-path>
        for to_address in envelope.to() {
            client
                .execute_command(RcptCommand::new(to_address.clone(), vec![]), [2], timeout)
                .await?;
            // Log the rcpt command
            debug!("{}: to=<{}>", envelope.message_id(), to_address);
        }

        // DATA
        client.execute_command(DataCommand, [354], timeout).await?;

        // Ready to stream data - responsibility of the outer
        Ok(())
    }
}

#[async_trait]
impl<Conf: ConnectionConfiguration, Conn: Connector> Transport for SmtpTransport<Conf, Conn> {
    type DataStream = SmtpDataStream<Conn::Stream>;

    #[future_is[Sync]]
    async fn send_stream(&self, envelope: Envelope) -> Result<Self::DataStream, Error> {
        let mut lease = match self.inner.lease().await {
            Ok(lease) => lease,
            Err(gone) => gone.set(Self::connect(&self.configuration, &self.connector).await?),
        };
        let timeout = self.configuration.timeout();

        if lease.reuse == 0 {
            // reuse countdown reached
            // close and refresh
            let mut client = SmtpProto::new(Pin::new(&mut lease.stream));
            client.execute_quit(timeout).await?;
            // new connection
            lease.replace(Self::connect(&self.configuration, &self.connector).await?);
        }

        lease.reuse = lease.reuse.saturating_sub(1);
        let message_id = envelope.message_id().to_owned();
        // prepare a mail
        Self::prepare_mail(&self.configuration, &mut lease, envelope, timeout).await?;
        // Return a data stream carying the lease away
        Ok(SmtpDataStream::new(lease, message_id, timeout))
    }
}

pub(crate) struct SmtpConnection<S> {
    pub stream: S,
    /// How many times can the stream be used
    reuse: u16,
    /// Information about the server
    /// Value is None before HELO/EHLO
    pub server_info: ServerInfo,
}