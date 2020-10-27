//! Traits and impls to represent and establish network-like streams

use crate::smtp::authentication::Authentication;
use crate::smtp::extension::ClientId;
use crate::smtp::extension::ServerInfo;
use crate::smtp::tls::{DefaultTls, TlsProvider, TlsUpgrade};
use crate::ClientSecurity;
use async_std::io::{self, Read, Write};
use async_std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use async_std::pin::Pin;
use async_std::task::{Context, Poll};
use futures::{ready, Future};
use log::trace;
use pin_project::pin_project;
use samotop_async_trait::async_trait;
use std::fmt;
use std::time::Duration;

#[async_trait]
pub trait Connector: Sync + Send {
    type Stream: MaybeTls + Read + Write + Unpin + Sync + Send + 'static;
    /// This provider of connectivity takes care of resolving
    /// given address (which could be an IP, FQDN, URL...),
    /// establishing a connection and enabling (or not) TLS upgrade.
    #[future_is[Sync+Send]]
    async fn connect<C: ConnectionConfiguration>(
        &self,
        configuration: &C,
    ) -> io::Result<Self::Stream>;
}

pub trait ConnectionConfiguration: Sync + Send {
    fn address(&self) -> String;
    fn timeout(&self) -> Duration;
    fn security(&self) -> ClientSecurity;
    fn hello_name(&self) -> ClientId;
    fn max_reuse_count(&self) -> u16;
    fn get_authentication(
        &self,
        server_info: &ServerInfo,
        encrypted: bool,
    ) -> Option<Box<dyn Authentication>>;
}

/// A stream implementing this trait may be able to upgrade to TLS
/// But maybe not...
pub trait MaybeTls {
    /// Initiates the TLS negotiations.
    /// The stream must then block all reads/writes until the
    /// underlying TLS handshake is done.
    fn encrypt(&mut self) -> Result<(), io::Error>;
    /// Returns true only if calling encrypt would make sense:
    /// 1. required encryption setup information is available.
    /// 2. the stream is not encrypted yet.
    fn can_encrypt(&self) -> bool;
    /// Returns true if the stream is already encrypted.
    fn is_encrypted(&self) -> bool;
}

pub type DefaultConnector = TcpConnector<DefaultTls>;

#[derive(Debug)]
pub struct TcpConnector<TLS> {
    pub tls_mode: TlsMode,
    pub provider: TLS,
}

impl Default for TcpConnector<DefaultTls> {
    fn default() -> Self {
        Self {
            tls_mode: TlsMode::StartTls,
            provider: DefaultTls,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TlsMode {
    Tls,
    StartTls,
}

#[async_trait]
impl<TLS> Connector for TcpConnector<TLS>
where
    TLS: TlsProvider<TcpStream> + Sync + Send + 'static,
{
    type Stream =
        NetworkStream<TcpStream, <TLS::Upgrade as TlsUpgrade<TcpStream>>::Encrypted, TLS::Upgrade>;
    /// This provider of connectivity takes care of resolving
    /// given address (which could be an IP, FQDN, URL...),
    /// establishing a connection and enabling (or not) TLS upgrade.
    #[future_is[Sync + Send]]
    async fn connect<C: ConnectionConfiguration + Sync>(
        &self,
        configuration: &C,
    ) -> io::Result<Self::Stream> {
        // TODO: try alternative addresses on failure. Here we just pick the first one.
        let mut to = configuration.address();
        let timeout = configuration.timeout();
        let addr = to.to_socket_addrs().await?.next().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("No address resolved for {}", to),
            )
        })?;

        let tcp_stream = io::timeout(timeout, TcpStream::connect(addr)).await?;

        // remove port part, domain/host remains
        to.find(':').map(|i| to.split_off(i));
        let mut stream = NetworkStream {
            peer_addr: tcp_stream.peer_addr().ok(),
            peer_name: to,
            state: State::Plain(tcp_stream, self.provider.get()),
        };

        match self.tls_mode {
            TlsMode::Tls => stream.encrypt()?,
            TlsMode::StartTls => { /* ready! */ }
        }
        Ok(stream)
    }
}

#[pin_project(project = NetStreamProj)]
#[derive(Debug)]
pub struct NetworkStream<S, E, U> {
    state: State<S, E, U>,
    peer_addr: Option<SocketAddr>,
    peer_name: String,
}

/// Represents the different types of underlying network streams
#[pin_project(project = StateProj)]
#[allow(missing_debug_implementations)]
enum State<S, E, U> {
    /// Plain TCP stream with name and potential TLS upgrade
    Plain(#[pin] S, U),
    /// Encrypted TCP stream
    Encrypted(#[pin] E),
    /// Pending TLS handshake
    Handshake(Pin<Box<dyn Future<Output = io::Result<E>> + Sync + Send>>),
    /// Transitional state to help take owned values from the enum
    /// Invalid outside of an &mut own method call/future
    None,
}

impl<S, U> MaybeTls for NetworkStream<S, U::Encrypted, U>
where
    U: TlsUpgrade<S>,
{
    /// Initiates the TLS negotiations.
    /// The stream must then block all reads/writes until the
    /// underlying TLS handshake is done.
    fn encrypt(&mut self) -> Result<(), io::Error> {
        match std::mem::replace(&mut self.state, State::None) {
            State::Plain(stream, upgrade) => {
                self.state = State::Handshake(Box::pin(
                    upgrade.upgrade_to_tls(stream, self.peer_name.clone()),
                ));
                Ok(())
            }
            otherwise => {
                self.state = otherwise;
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Invalid state to encrypt now",
                ))
            }
        }
    }
    /// Returns true only if calling encrypt would make sense:
    /// 1. required encryption setup information is available.
    /// 2. the stream is not encrypted yet.
    fn can_encrypt(&self) -> bool {
        match self.state {
            State::Plain(_, ref upgrade) => upgrade.is_enabled(),
            State::Encrypted(_) => false,
            State::Handshake(_) => false,
            State::None => false,
        }
    }
    /// Returns true if the stream is already encrypted (or hand shaking).
    fn is_encrypted(&self) -> bool {
        match self.state {
            State::Plain(_, _) => false,
            State::Encrypted(_) => true,
            State::Handshake(_) => true,
            State::None => false,
        }
    }
}

impl<S, E, U> NetworkStream<S, E, U> {
    /// Returns peer's address
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.peer_addr
            .ok_or_else(|| io::Error::from(io::ErrorKind::Other))
    }
}

impl<S, E, U> NetworkStream<S, E, U> {
    fn poll_tls(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        let proj = self.project();
        match std::mem::replace(proj.state, State::None) {
            State::Handshake(mut h) => match Pin::new(&mut h).poll(cx)? {
                Poll::Pending => {
                    *proj.state = State::Handshake(h);
                    Poll::Pending
                }
                Poll::Ready(encrypted) => {
                    *proj.state = State::Encrypted(encrypted);
                    Poll::Ready(Ok(()))
                }
            },
            otherwise => {
                *proj.state = otherwise;
                Poll::Ready(Ok(()))
            }
        }
    }
}

impl<S, E, U> Read for NetworkStream<S, E, U>
where
    S: Read + Unpin,
    E: Read + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        trace!("poll_read with {:?}", self.state);
        ready!(self.as_mut().poll_tls(cx))?;
        let result = match self.state {
            State::Plain(ref mut s, _) => Pin::new(s).poll_read(cx, buf),
            State::Encrypted(ref mut s) => Pin::new(s).poll_read(cx, buf),
            State::Handshake(_) => {
                unreachable!("Handshake is handled by poll_tls");
            }
            State::None => Poll::Ready(Err(broken())),
        };
        trace!("poll_read got {:?}", result);
        result
    }
}

impl<S, E, U> Write for NetworkStream<S, E, U>
where
    S: Write + Unpin,
    E: Write + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        ready!(self.as_mut().poll_tls(cx))?;
        match self.state {
            State::Plain(ref mut s, _) => Pin::new(s).poll_write(cx, buf),
            State::Encrypted(ref mut s) => Pin::new(s).poll_write(cx, buf),
            State::Handshake(_) => {
                unreachable!("Handshake is handled by poll_tls");
            }
            State::None => Poll::Ready(Err(broken())),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        ready!(self.as_mut().poll_tls(cx))?;
        match self.state {
            State::Plain(ref mut s, _) => Pin::new(s).poll_flush(cx),
            State::Encrypted(ref mut s) => Pin::new(s).poll_flush(cx),
            State::Handshake(_) => {
                unreachable!("Handshake is handled by poll_tls");
            }
            State::None => Poll::Ready(Err(broken())),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        ready!(self.as_mut().poll_tls(cx))?;
        match self.state {
            State::Plain(ref mut s, _) => Pin::new(s).poll_close(cx),
            State::Encrypted(ref mut s) => Pin::new(s).poll_close(cx),
            State::Handshake(_) => {
                unreachable!("Handshake is handled by poll_tls");
            }
            State::None => Poll::Ready(Err(broken())),
        }
    }
}

fn broken() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "Invalid network stream state")
}

impl<S, E, U> fmt::Debug for State<S, E, U> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use State::*;
        fmt.write_str(match self {
            Plain(_, _) => "Plain(stream, upgrade)",
            Encrypted(_) => "Encrypted(*)",
            Handshake(_) => "Handshake(*)",
            None => "None",
        })
    }
}
