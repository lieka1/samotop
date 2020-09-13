//! Reference implementation of a mail service
//! simply delivering mail to single directory.
use crate::common::*;
use crate::model::mail::*;
use crate::model::smtp::*;
use crate::service::mail::composite::*;
use crate::service::mail::*;
use async_std::fs::{create_dir_all, rename, File};
use async_std::path::Path;
use futures::future::TryFutureExt;

#[derive(Clone, Debug)]
pub struct Config<D> {
    dir: D,
}

impl<D> Config<D>
where
    D: AsRef<Path>,
{
    pub fn new(dir: D) -> Self {
        Self { dir }
    }
}

#[derive(Clone, Debug)]
pub struct SimpleDirMail<D, S> {
    dir: D,
    inner: S,
}

impl<D, S> SimpleDirMail<D, S>
where
    D: AsRef<Path>,
{
    pub fn new(dir: D, inner: S) -> Self {
        Self { dir, inner }
    }
}

impl<D, ES, GS, DS> MailSetup<ES, GS, DS> for Config<D>
where
    D: AsRef<Path> + Send + Sync,
    ES: EsmtpService,
    GS: MailGuard,
    DS: MailDispatch,
{
    type Output = CompositeMailService<EnableEightBit<ES>, GS, SimpleDirMail<D, DS>>;
    fn setup(self, extend: ES, guard: GS, dispatch: DS) -> Self::Output {
        (
            EnableEightBit(extend),
            guard,
            SimpleDirMail::new(self.dir, dispatch),
        )
            .into()
    }
}

#[derive(Clone, Debug)]
pub struct EnableEightBit<T>(T);

impl<T> EsmtpService for EnableEightBit<T>
where
    T: EsmtpService,
{
    fn prepare_session(&self, session: &mut SessionInfo) {
        self.0.prepare_session(session);
        session.extensions.enable(&extension::EIGHTBITMIME);
    }
}

impl<D, S> MailDispatch for SimpleDirMail<D, S>
where
    D: AsRef<Path> + Send + Sync,
    S: MailDispatch,
{
    type Mail = MailFile;
    type MailFuture = CreateMailFile;

    fn send_mail(&self, transaction: Transaction) -> Self::MailFuture {
        CreateMailFile::new(&self.dir, transaction)
    }
}

#[pin_project]
pub struct CreateMailFile {
    stage2: Option<(
        BytesMut,
        String,
        Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + Sync + 'static>>,
    )>,
    file: Pin<Box<dyn Future<Output = std::io::Result<File>> + Send + Sync + 'static>>,
}

impl CreateMailFile {
    pub fn new<D: AsRef<Path>>(dir: D, transaction: Transaction) -> Self {
        let mut headers = BytesMut::new();
        headers.extend(format!("X-Samotop-Helo: {:?}\r\n", transaction.session.smtp_helo).bytes());
        headers.extend(
            format!(
                "X-Samotop-Peer: {:?}\r\n",
                transaction.session.connection.peer_addr
            )
            .bytes(),
        );
        headers.extend(format!("X-Samotop-From: {:?}\r\n", transaction.mail).bytes());
        headers.extend(format!("X-Samotop-To: {:?}\r\n", transaction.rcpts).bytes());

        let target_dir = dir.as_ref().join("new");
        let tmp_dir = dir.as_ref().join("tmp");
        let target_file = target_dir.join(transaction.id.as_str());
        let tmp_file = tmp_dir.join(transaction.id.as_str());
        let target = Box::pin(rename(tmp_file.clone(), target_file.clone()));
        let file = Box::pin(
            ensure_dir(tmp_dir)
                .and_then(move |_| ensure_dir(target_dir))
                .and_then(move |_| File::create(tmp_file)),
        );

        Self {
            stage2: Some((headers, transaction.id, target)),
            file,
        }
    }
}

async fn ensure_dir<P: AsRef<Path>>(dir: P) -> std::io::Result<()> {
    if !dir.as_ref().exists().await {
        create_dir_all(dir).await
    } else {
        Ok(())
    }
}

impl Future for CreateMailFile {
    type Output = DispatchResult<MailFile>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<DispatchResult<MailFile>> {
        match ready!(Pin::new(&mut self.file).poll(cx)) {
            Ok(file) => {
                if let Some((buffer, id, target)) = self.stage2.take() {
                    Poll::Ready(Ok(MailFile {
                        id,
                        file,
                        buffer,
                        target,
                    }))
                } else {
                    error!("No buffer/id. Perhaps the future has been polled after Poll::Ready");
                    Poll::Ready(Err(DispatchError::FailedTemporarily))
                }
            }
            Err(e) => {
                error!("Could not create mail file: {:?}", e);
                Poll::Ready(Err(DispatchError::FailedTemporarily))
            }
        }
    }
}

#[pin_project(project=MailFileProj)]
pub struct MailFile {
    id: String,
    file: File,
    buffer: BytesMut,
    target: Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + Sync + 'static>>,
}

impl Write for MailFile {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        println!("Mail data for {}: {} bytes", self.id, buf.len());
        if self.as_mut().buffer.len() > 10 * 1024 {
            ready!(self.as_mut().poll_flush(cx)?);
        }
        self.buffer.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let MailFileProj { file, buffer, .. } = self.project();
        let mut pending = &buffer[..];
        let mut file = Pin::new(file);
        trace!("Writing mail data: {} bytes", pending.len());
        while let Poll::Ready(len) = file.as_mut().poll_write(cx, pending)? {
            trace!("Wrote mail data: {} bytes", len);
            if len == 0 {
                break;
            }
            pending = &pending[len..];
        }
        // remove written bytes from the buffer
        let written = buffer.len() - pending.len();
        drop(buffer.split_to(written));
        trace!("Remaining {} bytes", buffer.len());
        if buffer.len() == 0 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        ready!(self.as_mut().poll_flush(cx))?;
        let MailFileProj { target, .. } = self.project();
        ready!(target.as_mut().poll(cx))?;
        Poll::Ready(Ok(()))
    }
}
