use crate::common::*;
use crate::io::tls::{Io, MayBeTls, TlsCapable};
use crate::io::*;
use async_std::stream::StreamExt;
use async_std::task;
use futures_util::stream::FuturesUnordered;

use async_std::{os::unix::net::UnixListener, path::PathBuf as SocketAddr};

/// `UnixServer` takes care of accepting Unix socket connections and passing them to an `IoService` to `handle()`.
#[derive(Default)]
pub struct UnixServer<'a> {
    ports: Vec<S1Fut<'a, Result<Vec<SocketAddr>>>>,
}

impl<'a> UnixServer<'a> {
    /// Listen on this socket - usually a path to socket file. You can call this multiple times to listen on multiple sockets.
    pub fn on<N>(ports: N) -> Self
    where
        N: Into<SocketAddr> + 'a,
    {
        Self::default().and(ports)
    }
    /// Listen on this socket - usually a path to a socket file. You can call this multiple times to listen on multiple sockets.
    pub fn and<N>(mut self, ports: N) -> Self
    where
        N: Into<SocketAddr> + 'a,
    {
        self.ports.push(Box::pin(Self::map_ports(ports)));
        self
    }
    /// Listen on multiple sockets - usually a list of path items
    pub fn on_all<I, N>(ports: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<SocketAddr> + 'a,
    {
        Self::default().and_all(ports)
    }
    /// Listen on multiple sockets - usually a list of path items
    pub fn and_all<I, N>(mut self, ports: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<SocketAddr> + 'a,
    {
        for port in ports.into_iter() {
            self = self.and(port);
        }
        self
    }
    fn map_ports(addrs: impl Into<SocketAddr>) -> impl Future<Output = Result<Vec<SocketAddr>>> {
        // todo: check if file exists and is a socket here?
        ready(Ok(vec![addrs.into()]))
    }
    async fn resolve_ports(&mut self) -> Result<Vec<SocketAddr>> {
        let mut result = vec![];
        for port in self.ports.iter_mut() {
            let port = port.await?;
            result.extend_from_slice(&port[..]);
        }
        Ok(result)
    }
    /// Serve the given IoService on configured sockets
    pub async fn serve<S>(mut self, service: S) -> Result<()>
    where
        S: IoService + Send + Sync,
    {
        Self::serve_ports(service, self.resolve_ports().await?).await
    }
    async fn serve_ports<S>(service: S, addrs: impl IntoIterator<Item = SocketAddr>) -> Result<()>
    where
        S: IoService + Send + Sync,
    {
        let svc = Arc::new(service);

        addrs
            .into_iter()
            .map(|a| Self::serve_port(svc.clone(), a))
            .collect::<FuturesUnordered<_>>()
            .skip_while(|r| r.is_ok())
            .take(1)
            .fold(Ok(()), |acc, cur| match cur {
                Err(e) => Err(e),
                Ok(()) => acc,
            })
            .await
    }
    async fn serve_port<S>(service: S, addr: SocketAddr) -> Result<()>
    where
        S: IoService + Clone,
    {
        trace!("Binding on {:?}", addr);
        let listener = UnixListener::bind(addr.clone())
            .await
            .map_err(|e| format!("Unable to bind {:?}: {}", addr, e))?;
        let mut incoming = listener.incoming();
        info!("Listening on {:?}", listener.local_addr());
        while let Some(stream) = incoming.next().await {
            let conn = if let Ok(ref stream) = stream {
                ConnectionInfo::new(
                    stream
                        .local_addr()
                        .ok()
                        .and_then(|s| {
                            s.as_pathname()
                                .and_then(|p| p.to_str())
                                .map(|s| s.to_owned())
                        })
                        .unwrap_or_default(),
                    stream
                        .peer_addr()
                        .ok()
                        .and_then(|s| {
                            s.as_pathname()
                                .and_then(|p| p.to_str())
                                .map(|s| s.to_owned())
                        })
                        .unwrap_or_default(),
                )
            } else {
                ConnectionInfo::default()
            };
            let stream = match stream {
                Ok(s) => {
                    let s: Box<dyn Io> = Box::new(s);
                    let s: Box<dyn MayBeTls> = Box::new(TlsCapable::plaintext(s));
                    Ok(s)
                }
                Err(e) => (Err(e.into())),
            };
            let service = service.clone();
            spawn_task_and_swallow_log_errors(
                format!("Unix connection {}", conn.id),
                service.handle(stream, conn),
            );
        }
        Ok(())
    }
}

fn spawn_task_and_swallow_log_errors<F>(task_name: String, fut: F) -> task::JoinHandle<()>
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    task::spawn(async move { log_errors(task_name, fut).await.unwrap_or_default() })
}

async fn log_errors<F, T, E>(task_name: String, fut: F) -> Option<T>
where
    F: Future<Output = std::result::Result<T, E>>,
    E: std::fmt::Display,
{
    match fut.await {
        Err(e) => {
            error!("Error in {}: {}", task_name, e);
            None
        }
        Ok(r) => {
            info!("{} completed successfully.", task_name);
            Some(r)
        }
    }
}
