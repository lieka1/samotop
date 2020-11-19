pub mod dummy;
pub mod stateful;
use crate::common::*;
use crate::smtp::ReadControl;
use crate::smtp::WriteControl;

pub type InputStream = Box<dyn Stream<Item = Result<ReadControl>> + Unpin + Sync + Send>;
pub type OutputStream = Box<dyn Stream<Item = Result<WriteControl>> + Unpin + Sync + Send>;

/**
A session service handles the SMTP session.

For each connection a new handler is started with a call to `start()`.
This handler will only handle one session and then it will be dropped.

The handler will receive `ReadControl`s from the line and should produce
relevant `WriteControl`s to send down the line in response.
*/
pub trait SessionService {
    fn start(&self, input: InputStream) -> OutputStream;
}
