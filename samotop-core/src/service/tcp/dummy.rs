use crate::model::io::Connection;
use crate::model::Result;
use crate::service::tcp::TcpService;
use futures::prelude::*;

#[doc = "Dummy TCP service for testing samotop server"]
#[derive(Clone, Debug)]
pub struct DummyTcpService;

impl<IO> TcpService<IO> for DummyTcpService {
    type Future = future::Ready<()>;
    fn handle(self, _io: Result<IO>, conn: Connection) -> Self::Future {
        info!("Received connection {}", conn);
        future::ready(())
    }
}
