use samotop_model::smtp::SmtpSessionCommand;

use crate::common::*;
use crate::smtp::{ReadControl, SmtpState, WriteControl};

#[pin_project(project=SessionProj)]
pub struct StatefulSession<I, S> {
    #[pin]
    input: I,
    state: State<S>,
}

enum State<T> {
    Ready(T),
    Pending(S2Fut<'static, T>),
    Taken,
}

impl<T> Default for State<T> {
    fn default() -> Self {
        Self::Taken
    }
}

impl<I, S> Stream for StatefulSession<I, S>
where
    I: Stream<Item = Result<ReadControl>>,
    S: SmtpState + 'static,
{
    type Item = Result<WriteControl>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        trace!("polling next");
        loop {
            if let Some(answer) = ready!(self.as_mut().poll_pop(cx)) {
                trace!("Answer is: {:?}", answer);
                break Poll::Ready(Some(Ok(answer)));
            }
            let proj = self.as_mut().project();
            break match std::mem::take(proj.state) {
                State::Taken => Poll::Ready(None),
                State::Pending(_) => unreachable!("handled by previous poll"),
                State::Ready(data) => match proj.input.poll_next(cx) {
                    Poll::Pending => {
                        *proj.state = State::Ready(data);
                        Poll::Pending
                    }
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Ready(Some(Ok(control))) => {
                        *proj.state = State::Pending(control.apply(data));
                        continue;
                    }
                    Poll::Ready(Some(Err(e))) => {
                        error!("reading SMTP input failed: {:?}", e);
                        Poll::Ready(Some(Ok(WriteControl::Shutdown(
                            samotop_model::smtp::SmtpReply::ProcesingError,
                        ))))
                    }
                },
            };
        }
    }
}

impl<I, S> StatefulSession<I, S>
where
    S: SmtpState + 'static,
{
    pub fn new(input: I, state: S) -> Self {
        Self {
            state: State::Ready(state),
            input,
        }
    }
    fn poll_pop(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<WriteControl>> {
        trace!("polling pending");
        let proj = self.project();
        match proj.state {
            State::Taken => Poll::Ready(None),
            State::Ready(ref mut data) => Poll::Ready(data.pop()),
            State::Pending(ref mut fut) => {
                let mut data = ready!(fut.as_mut().poll(cx));
                let pop = Poll::Ready(data.pop());
                *proj.state = State::Ready(data);
                pop
            }
        }
    }
}
