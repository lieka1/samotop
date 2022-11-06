use crate::common::*;
use crate::io::tls::MayBeTls;
use crate::mail::{AcceptsInterpretter, AcceptsSessionService, MailSetup};
use crate::smtp::{Interpret, InterpretResult, ParseError, SessionService, SmtpContext};
use smol_timeout::TimeoutExt;
use std::time::{Duration, Instant};

/// Prevent bad SMTP behavior
#[derive(Debug, Default, Clone)]
pub struct Prudence {
    /// Monitor bad behavior of clients not waiting for a banner given time
    wait_for_banner_delay: Option<Duration>,
    /// Maximum read time
    read_timeout: Option<Duration>,
}

impl Prudence {
    /// Shut the session down if the client sends commands before the delayed banner
    pub fn with_banner_delay(mut self, delay: Duration) -> Self {
        self.wait_for_banner_delay = Some(delay);
        self
    }
    /// Shut the session down if the client takes too long to send a command
    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = Some(timeout);
        self
    }
}

impl<T> MailSetup<T> for Prudence
where
    T: AcceptsSessionService + AcceptsInterpretter,
{
    fn setup(self, config: &mut T) {
        config.wrap_interpretter(|inner| PrudentInterpretter {
            inner,
            timeout: self.read_timeout,
        });
        config.wrap_session_service(|others| PrudentService {
            config: self,
            others,
        });
    }
}

#[derive(Debug)]
struct PrudentService {
    config: Prudence,
    others: Box<dyn SessionService + Sync + Send>,
}

impl SessionService for PrudentService {
    fn prepare_session<'a, 'i, 's, 'f>(
        &'a self,
        io: &'i mut Box<dyn MayBeTls>,
        state: &'s mut SmtpContext,
    ) -> crate::common::S1Fut<'f, ()>
    where
        'a: 'f,
        'i: 'f,
        's: 'f,
    {
        Box::pin(async move {
            if let Some(delay) = self.config.wait_for_banner_delay {
                let mut buf = [0u8; 425];
                use async_std::io::ReadExt;
                match io.read(&mut buf[..]).timeout(delay).await {
                    Some(Ok(0)) => {
                        // this just looks like the client gave up and left
                        warn!("{} touch and go!", state.session.connection.peer_addr)
                    }
                    Some(Ok(len)) => {
                        state.session.input.extend_from_slice(&buf[0..len]);
                        state.session.say_shutdown_processing_err(
                            "Client sent commands before banner".into(),
                        );
                    }
                    Some(Err(e)) => {
                        state
                            .session
                            .say_shutdown_processing_err(format!("IO read failed {}", e));
                    }
                    None => {
                        // timeout is correct behavior, well done!
                    }
                }
            }

            *io = Box::new(PrudentIo::new(
                self.config.read_timeout,
                std::mem::replace(io, Box::new(Dummy)),
            ));

            self.others.prepare_session(io, state).await;
        })
    }
}

#[derive(Debug)]
struct PrudentState {
    pub last_command_at: Instant,
}

struct PrudentIo {
    expired: Pin<Box<dyn Future<Output = ()> + Sync + Send>>,
    timeout: Option<Duration>,
    io: Box<dyn MayBeTls>,
}

impl PrudentIo {
    pub fn new<IO: MayBeTls + 'static>(timeout: Option<Duration>, io: IO) -> Self {
        PrudentIo {
            expired: Box::pin(Self::expire(timeout)),
            timeout,
            io: Box::new(io),
        }
    }
    async fn expire(timeout: Option<Duration>) {
        if let Some(timeout) = timeout {
            pending::<()>().timeout(timeout).await;
        } else {
            pending::<()>().await;
        }
    }
}

impl MayBeTls for PrudentIo {
    fn enable_encryption(&mut self, upgrade: Box<dyn crate::io::tls::TlsUpgrade>, name: String) {
        self.io.enable_encryption(upgrade, name)
    }

    fn encrypt(mut self: Pin<&mut Self>) {
        Pin::new(&mut self.io).encrypt()
    }

    fn can_encrypt(&self) -> bool {
        self.io.can_encrypt()
    }

    fn is_encrypted(&self) -> bool {
        self.io.is_encrypted()
    }
}

impl io::Read for PrudentIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        if let Poll::Ready(()) = self.expired.as_mut().poll(cx) {
            return Poll::Ready(Err(io::ErrorKind::TimedOut.into()));
        }

        let res = Pin::new(&mut self.io).poll_read(cx, buf);

        if let Poll::Ready(Ok(_)) = res {
            self.expired = Box::pin(Self::expire(self.timeout))
        }

        res
    }
}
impl io::Write for PrudentIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_close(cx)
    }
}

/// Enforces the specified command timeout
#[derive(Debug)]
struct PrudentInterpretter {
    inner: Box<dyn Interpret + Sync + Send>,
    timeout: Option<Duration>,
}

impl Interpret for PrudentInterpretter {
    fn interpret<'a, 's, 'f>(&'a self, state: &'s mut SmtpContext) -> S1Fut<'f, InterpretResult>
    where
        'a: 'f,
        's: 'f,
    {
        Box::pin(self.interpret_inner(state))
    }
}

impl PrudentInterpretter {
    pub async fn interpret_inner(&self, state: &mut SmtpContext) -> InterpretResult {
        let res = self.inner.interpret(state).await;

        if let Some(timeout) = self.timeout {
            let mystate = state.get_or_insert(|| PrudentState {
                last_command_at: Instant::now(),
            });

            match res {
                Ok(Some(consumed)) if consumed != 0 => {
                    mystate.last_command_at = std::time::Instant::now();
                }
                Err(ParseError::Incomplete) => {
                    if Instant::now().saturating_duration_since(mystate.last_command_at) > timeout {
                        state.session.say_shutdown_timeout();
                        return Ok(None);
                    }
                }
                _ => {}
            }
        }

        res
    }
}
