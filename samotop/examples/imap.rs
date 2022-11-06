/*!
Example of receiving IMAP commands

## Testing - https://gist.github.com/akpoff/53ac391037ae2f2d376214eac4a23634

 */

use async_std::{
    io::{
        self,
        prelude::{BufRead, WriteExt},
        stdout, Read, Write,
    },
    task::ready,
};
use log::*;
use samotop::{io::IoService, server::TcpServer};
use std::{fmt, future::Future, ops::Deref, str::FromStr};
use std::{
    ops::DerefMut,
    task::{Context, Poll},
};
use std::{ops::RangeBounds, pin::Pin};

#[async_std::main]
async fn main() -> Result<()> {
    env_logger::init();
    TcpServer::on("localhost:2525").serve(Imap).await
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct Imap;

impl IoService for Imap {
    fn handle(
        &self,
        io: samotop_core::common::Result<Box<dyn samotop::io::tls::MayBeTls>>,
        _connection: samotop::io::ConnectionInfo,
    ) -> samotop_core::common::S1Fut<'static, samotop_core::common::Result<()>> {
        Box::pin(async move {
            let mut io = MatchReader::new(io?);
            let io = &mut io;

            respond(io.deref_mut(), "*", "OK Server ready.").await?;

            #[derive(Debug, Default)]
            struct State {
                usr: String,
                pwd: String,
            }
            let mut state = State::default();

            loop {
                let tg: String = io.parse(not(ws().or(eol())).many()).await?;
                let ___ = io.matches(ws().many()).await?;

                if io.matches(tag("capability")).await? {
                    respond(io.deref_mut(), tg, "OK NONE").await?;
                } else if io.matches(tag("login")).await? {
                    state.usr = {
                        io.matches(ws().many()).await?;
                        io.parse(not(ws()).many()).await?
                    };
                    state.pwd = {
                        io.matches(ws().many()).await?;
                        io.parse(not(ws()).many()).await?
                    };
                    info!("USER {} PWD {}", state.usr, state.pwd.len());
                    respond(io.deref_mut(), tg, "OK authhhh").await?;
                } else if io.matches(tag("logout")).await? {
                    respond(io.deref_mut(), tg, "OK bye").await?;
                    break;
                } else if io.matches(tag("status")).await? {
                    let ___ = io.matches(ws().many()).await?;
                    let inbx = io.match_str(not(ws()).many()).await?;
                    let msg = format!("STATUS {} (MESSAGES 231)", inbx);
                    respond(io.deref_mut(), "*", msg).await?;
                    respond(io.deref_mut(), tg, "OK STATUS completed").await?;
                } else if io.matches(tag("select")).await? {
                    let ___ = io.matches(ws().many()).await?;
                    let inbx = io.match_str(not(ws()).many()).await?;
                    respond(io.deref_mut(), "*", "172 EXISTS").await?;
                    respond(io.deref_mut(), "*", "1 RECENT").await?;
                    respond(
                        io.deref_mut(),
                        "*",
                        "OK [UNSEEN 12] Message 12 is first unseen",
                    )
                    .await?;
                    respond(
                        io.deref_mut(),
                        "*",
                        "OK [UIDVALIDITY 3857529045] UIDs valid",
                    )
                    .await?;
                    respond(io.deref_mut(), "*", "OK [UIDNEXT 4392] Predicted next UID").await?;
                    respond(
                        io.deref_mut(),
                        "*",
                        "FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)",
                    )
                    .await?;
                    respond(
                        io.deref_mut(),
                        "*",
                        "OK [PERMANENTFLAGS (\\Deleted \\Seen \\*)] Limited",
                    )
                    .await?;
                    respond(io.deref_mut(), tg, "OK SELECT [READ-ONLY] completed").await?;
                } else {
                    respond(io.deref_mut(), tg, "NO unimplemented!").await?;
                }
                io.matches(not(eol()).repeat(..)).await?;
                io.matches(tag("\r\n")).await?;
            }
            async_std::io::copy(io, stdout()).await?;
            Ok(())
        })
    }
}

async fn respond<IO: Write + Unpin>(
    mut io: impl DerefMut<Target = IO>,
    tag: impl fmt::Display,
    what: impl fmt::Display,
) -> io::Result<()> {
    let msg = format!("{} {}\r\n", tag, what);
    io.write_all(msg.as_bytes()).await?;
    io.flush().await?;
    debug!("Response: {:?}", msg);
    Ok(())
}

/*

pub fn not_x<'a, P>(mut predicate: P) -> Call<'a, bool>
where
    P: 'a + Predicate + Send,
{
    Call::new(format!("!{:?}", predicate), move |x| !predicate.accept(x))
}
pub fn ws_x() -> impl Predicate + Send {
    Call::new("ws", |b| b == &b' ' || b == &b'\t')
}
pub fn eol_x() -> impl Predicate + Send {
    Call::new("eol", |b| b == &b'\n' || b == &b'\r')
}
*/
pub fn not<'a, M>(mut predicate: M) -> Call<'a, MatchResult>
where
    M: 'a + Matcher + Send,
{
    Call::new(format!("!{:?}", predicate), move |x| {
        match predicate.matches(x) {
            MatchResult::Incomplete => MatchResult::Incomplete,
            MatchResult::Match => MatchResult::Mismatch,
            MatchResult::Complete => MatchResult::Mismatch,
            MatchResult::Mismatch => MatchResult::Match,
            MatchResult::Fail => MatchResult::Fail,
        }
    })
}
pub fn ws() -> Call<'static, MatchResult> {
    Call::new("ws", |b| match b == &b'\t' || b == &b' ' {
        true => MatchResult::Match,
        false => MatchResult::Mismatch,
    })
}
pub fn eol() -> Call<'static, MatchResult> {
    Call::new("eol", |b| match b == &b'\n' || b == &b'\r' {
        true => MatchResult::Match,
        false => MatchResult::Mismatch,
    })
}
pub struct Call<'a, O>(String, Box<dyn 'a + Send + FnMut(&u8) -> O>);
impl<'a, O> fmt::Debug for Call<'a, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}
impl<'a, O> Call<'a, O> {
    pub fn new(dbg: impl fmt::Display, inner: impl Send + FnMut(&u8) -> O + 'a) -> Self {
        Self(dbg.to_string(), Box::new(inner))
    }
}
impl<'a, O> Deref for Call<'a, O> {
    type Target = Box<dyn 'a + Send + FnMut(&u8) -> O>;

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}
impl<'a, O> DerefMut for Call<'a, O> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.1
    }
}

type Fut<'f, T> = Pin<Box<dyn Future<Output = T> + Send + 'f>>;

struct MatchReader<IO> {
    inner: Pin<Box<IO>>,
    buffer: Vec<u8>,
    max: usize,
    grow: usize,
    filling: usize,
    consumed: usize,
}
impl<IO> MatchReader<IO> {
    pub fn new(io: IO) -> Self {
        Self::with_capacity(io, 1024)
    }
    pub fn with_capacity(io: IO, max: usize) -> Self {
        // grow by 10 % but at most 1KB at least 1B
        let grow = usize::max(1, usize::min(max / 10, 1024));

        Self {
            inner: Box::pin(io),
            buffer: Vec::with_capacity(grow),
            max,
            grow,
            filling: 0,
            consumed: 0,
        }
    }
}
impl<IO> Deref for MatchReader<IO>
where
    IO: Unpin,
{
    type Target = IO;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}
impl<IO> DerefMut for MatchReader<IO>
where
    IO: Unpin,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}
impl<IO> Read for MatchReader<IO>
where
    IO: Read + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        if self.filling == 0 {
            ready!(self.as_mut().poll_fill_buf(cx))?;
        }
        let end = usize::min(self.filling, buf.len());
        let start = usize::min(self.consumed, end);
        let len = end - start;
        buf[0..len].copy_from_slice(&self.buffer[start..end]);
        self.consume(len);
        Poll::Ready(Ok(len))
    }
}
impl<IO> BufRead for MatchReader<IO>
where
    IO: Read + Unpin,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let MatchReader {
            inner,
            buffer,
            filling,
            grow,
            max,
            consumed,
        } = self.get_mut();

        if *filling >= buffer.len() {
            if *consumed != 0 {
                // Something has been read, shift bytes, make space
                debug!("Cleaning up {}", *consumed);
                buffer.copy_within(*consumed..*filling, 0);
                *filling = *filling - *consumed;
                *consumed = 0;
            } else {
                let len = usize::min(*grow + buffer.len(), *max);
                if *filling >= len {
                    error!("Buffer max size reached: {}", len);
                    return Poll::Ready(Err(io::ErrorKind::Unsupported.into()));
                }
                debug!("Growing to {}", len);
                buffer.resize(len, 0);
            }
        }

        let len = ready!(inner.as_mut().poll_read(cx, &mut buffer[*filling..]))?;
        *filling += len;
        debug!(
            "Read {}, remaining capacity {}, total size {}",
            len,
            buffer.len() - *filling,
            buffer.len()
        );
        Poll::Ready(Ok(&buffer[0..*filling]))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.consumed += amt;
    }
}

pub trait Parse {
    fn parse<'a, 'm, M, R>(&'a mut self, matcher: M) -> Fut<'a, io::Result<R>>
    where
        'm: 'a,
        M: Matcher + Send + Unpin + 'm,
        R: FromStr,
        Self: Send,
    {
        Box::pin(async move {
            match self.parse_opt(matcher).await? {
                None => Err(io::ErrorKind::InvalidInput.into()),
                Some(res) => Ok(res),
            }
        })
    }
    fn parse_opt<'a, 'm, M, R>(&'a mut self, matcher: M) -> Fut<'a, io::Result<Option<R>>>
    where
        'm: 'a,
        M: Matcher + Send + Unpin + 'm,
        R: FromStr,
        Self: Send,
    {
        Box::pin(async move {
            match self.match_str_opt(matcher).await? {
                None => Ok(None),
                Some(res) => match res.parse() {
                    Ok(parsed) => Ok(Some(parsed)),
                    Err(_) => Ok(None),
                },
            }
        })
    }
    fn match_str<'a, 'm, M>(&'a mut self, matcher: M) -> Fut<'a, io::Result<&'a str>>
    where
        'm: 'a,
        M: Matcher + Send + Unpin + 'm,
        Self: Send,
    {
        Box::pin(async move {
            match self.match_str_opt(matcher).await? {
                None => Err(io::ErrorKind::InvalidInput.into()),
                Some(res) => Ok(res),
            }
        })
    }
    fn match_str_opt<'a, 'm, M>(&'a mut self, matcher: M) -> Fut<'a, io::Result<Option<&'a str>>>
    where
        'm: 'a,
        M: Matcher + Send + Unpin + 'm,
        Self: Send,
    {
        Box::pin(async move {
            match self.match_bytes_opt(matcher).await? {
                None => Ok(None),
                Some(res) => match std::str::from_utf8(res) {
                    Ok(str) => Ok(Some(str)),
                    Err(_) => Ok(None),
                },
            }
        })
    }
    fn matches<'a, 'm, M>(&'a mut self, matcher: M) -> Fut<'a, io::Result<bool>>
    where
        'm: 'a,
        M: Matcher + Send + Unpin + 'm,
        Self: Send,
    {
        Box::pin(async move {
            match self.match_bytes_opt(matcher).await? {
                None => Ok(false),
                Some(_) => Ok(true),
            }
        })
    }
    fn match_bytes_opt<'a, 'm, M>(
        &'a mut self,
        matcher: M,
    ) -> Fut<'a, io::Result<Option<&'a [u8]>>>
    where
        'm: 'a,
        M: Matcher + Send + Unpin + 'm;
}

impl<IO> Parse for MatchReader<IO>
where
    IO: Read + Send + Unpin,
{
    fn match_bytes_opt<'a, 'm, M>(
        &'a mut self,
        mut matcher: M,
    ) -> Fut<'a, io::Result<Option<&'a [u8]>>>
    where
        'm: 'a,
        M: Matcher + Send + Unpin + 'm,
    {
        struct Fill<'a, T>(&'a mut MatchReader<T>);
        impl<'a, T: Read + Unpin> Future for Fill<'a, T> {
            type Output = io::Result<()>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                ready!(Pin::new(&mut self.get_mut().0).poll_fill_buf(cx))?;
                Poll::Ready(Ok(()))
            }
        }

        Box::pin(async move {
            let mut matched = 0;
            let mut result = MatchResult::Incomplete;

            while result == MatchResult::Incomplete || result == MatchResult::Match {
                trace!(
                    "{:?} @{}/{} - {:?}",
                    result,
                    self.consumed + matched,
                    self.filling,
                    matcher
                );
                if self.consumed + matched >= self.filling {
                    debug!("Filling up");
                    Fill(self).await?;

                    if self.consumed + matched >= self.filling {
                        debug!("EOF");
                        break;
                    }
                }
                result = match matcher.matches(&self.buffer[self.consumed + matched]) {
                    MatchResult::Fail => return Err(io::ErrorKind::Unsupported.into()),
                    MatchResult::Mismatch => match result {
                        MatchResult::Match => {
                            // mismatch after a match => complete
                            break;
                        }
                        MatchResult::Incomplete => MatchResult::Mismatch,
                        _ => unreachable!("cannot be"),
                    },
                    r @ MatchResult::Incomplete
                    | r @ MatchResult::Match
                    | r @ MatchResult::Complete => r,
                };
                matched += 1;
            }

            let rng = self.consumed..self.consumed + matched;
            debug!(
                "{:?} {} @{}/{} - {:?}: {:?}",
                result,
                matched,
                self.consumed + matched,
                self.filling,
                matcher,
                String::from_utf8_lossy(self.buffer[rng.clone()].as_ref())
            );
            match result {
                MatchResult::Match | MatchResult::Complete => {
                    Pin::new(&mut *self).consume(matched);
                    Ok(Some(self.buffer[rng].as_ref()))
                }
                _ => Ok(None),
            }
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum MatchResult {
    Incomplete,
    Match,
    Complete,
    Mismatch,
    Fail,
}

pub fn tag<'a, T>(what: T) -> Call<'a, MatchResult>
where
    T: fmt::Debug,
    T: AsRef<[u8]>,
    T: 'a + Send,
{
    let mut pos = 0;
    Call::new(format!("{:?}", what), move |byte| {
        if pos >= what.as_ref().len() {
            MatchResult::Fail
        } else if byte == &what.as_ref()[pos] {
            pos += 1;
            if pos >= what.as_ref().len() {
                MatchResult::Complete
            } else {
                MatchResult::Incomplete
            }
        } else {
            pos = what.as_ref().len();
            MatchResult::Mismatch
        }
    })
}

pub trait Matcher: fmt::Debug + Send + Sized {
    fn matches(&mut self, byte: &u8) -> MatchResult;

    fn once<'a>(self) -> Call<'a, MatchResult>
    where
        Self: 'a,
    {
        self.exactly(1)
    }
    fn many<'a>(self) -> Call<'a, MatchResult>
    where
        Self: 'a,
    {
        self.repeat(1..)
    }
    fn exactly<'a>(self, repeat: usize) -> Call<'a, MatchResult>
    where
        Self: 'a,
    {
        self.repeat(repeat..repeat + 1)
    }
    fn repeat<'a, 'r, R>(mut self, range: R) -> Call<'a, MatchResult>
    where
        R: RangeBounds<usize> + Send + 'r,
        R: fmt::Debug,
        Self: 'a,
        'r: 'a,
    {
        let mut count = 0;
        let min = match range.start_bound() {
            std::ops::Bound::Unbounded => 0,
            std::ops::Bound::Included(min) => *min,
            std::ops::Bound::Excluded(min) => min.saturating_add(1),
        } as usize;
        let max = match range.end_bound() {
            std::ops::Bound::Unbounded => usize::MAX,
            std::ops::Bound::Included(max) => *max,
            std::ops::Bound::Excluded(max) => max.saturating_sub(1),
        } as usize;
        Call::new(format!("{:?}{{{:?}}}", self, range), move |b| {
            if count >= max {
                MatchResult::Fail
            } else {
                count += 1;
                match self.matches(b) {
                    MatchResult::Match => {
                        if count >= max {
                            MatchResult::Complete
                        } else if count < min {
                            MatchResult::Incomplete
                        } else {
                            MatchResult::Match
                        }
                    }
                    r @ MatchResult::Mismatch
                    | r @ MatchResult::Fail
                    | r @ MatchResult::Incomplete
                    | r @ MatchResult::Complete => r,
                }
            }
        })
    }

    fn or<'a, 'p, P>(mut self, mut otherwise: P) -> Call<'a, MatchResult>
    where
        P: Matcher + Send + 'p,
        Self: 'a,
        'p: 'a,
    {
        Call::new(format!("({:?} | {:?})", self, otherwise), move |b| {
            self.matches(b) | otherwise.matches(b)
        })
    }

    fn and<'a, 'p, P>(mut self, mut aswell: P) -> Call<'a, MatchResult>
    where
        P: Matcher + Send + 'p,
        Self: 'a,
        'p: 'a,
    {
        Call::new(format!("({:?} & {:?})", self, aswell), move |b| {
            self.matches(b) & aswell.matches(b)
        })
    }
}

impl std::ops::BitOr for MatchResult {
    type Output = MatchResult;

    fn bitor(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (r @ MatchResult::Match, _)
            | (_, r @ MatchResult::Match)
            | (r @ MatchResult::Complete, _)
            | (_, r @ MatchResult::Complete)
            | (r @ MatchResult::Incomplete, _)
            | (_, r @ MatchResult::Incomplete)
            | (r @ MatchResult::Fail, _)
            | (_, r @ MatchResult::Fail)
            | (r @ MatchResult::Mismatch, MatchResult::Mismatch) => r,
        }
    }
}
impl std::ops::BitAnd for MatchResult {
    type Output = MatchResult;

    fn bitand(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (r @ MatchResult::Fail, _)
            | (_, r @ MatchResult::Fail)
            | (r @ MatchResult::Mismatch, _)
            | (_, r @ MatchResult::Mismatch)
            | (r @ MatchResult::Incomplete, _)
            | (_, r @ MatchResult::Incomplete)
            | (r @ MatchResult::Complete, _)
            | (_, r @ MatchResult::Complete)
            | (MatchResult::Match, r @ MatchResult::Match) => r,
        }
    }
}

impl<T> Matcher for T
where
    T: DerefMut,
    T::Target: FnMut(&u8) -> MatchResult,
    T: Send + fmt::Debug,
{
    fn matches(&mut self, byte: &u8) -> MatchResult {
        self(byte)
    }
}
/*
pub trait Predicate: fmt::Debug + Sized + Send {
    fn accept(&mut self, byte: &u8) -> bool;

    fn once<'a>(self) -> Call<'a, MatchResult>
    where
        Self: 'a,
    {
        self.exactly(1)
    }
    fn many<'a>(self) -> Call<'a, MatchResult>
    where
        Self: 'a,
    {
        self.repeat(1..)
    }
    fn exactly<'a>(self, repeat: usize) -> Call<'a, MatchResult>
    where
        Self: 'a,
    {
        self.repeat(repeat..repeat + 1)
    }
    fn repeat<'a, 'r, R>(mut self, range: R) -> Call<'a, MatchResult>
    where
        R: RangeBounds<usize> + Send + 'r,
        R: fmt::Debug,
        Self: 'a,
        'r: 'a,
    {
        let mut count = 0;
        Call::new(format!("{:?}{{{:?}}}", self, range), move |b| {
            let min = match range.start_bound() {
                std::ops::Bound::Unbounded => 0,
                std::ops::Bound::Included(min) => *min,
                std::ops::Bound::Excluded(min) => min.saturating_add(1),
            } as usize;
            let max = match range.end_bound() {
                std::ops::Bound::Unbounded => usize::MAX,
                std::ops::Bound::Included(max) => *max,
                std::ops::Bound::Excluded(max) => max.saturating_sub(1),
            } as usize;

            if count >= max {
                MatchResult::Fail
            } else if self.accept(b) {
                count += 1;
                if count >= max {
                    MatchResult::Complete
                } else if count < min {
                    MatchResult::Incomplete
                } else {
                    MatchResult::Match
                }
            } else {
                MatchResult::Mismatch
            }
        })
    }

    fn or<'a, 'p, P>(mut self, mut otherwise: P) -> Call<'a, bool>
    where
        P: Predicate + Send + 'p,
        Self: 'a,
        'p: 'a,
    {
        Call::new(format!("({:?} || {:?})", self, otherwise), move |b| {
            self.accept(b) || otherwise.accept(b)
        })
    }

    fn and<'a, 'p, P>(mut self, mut aswell: P) -> Call<'a, bool>
    where
        P: Predicate + Send + 'p,
        Self: 'a,
        'p: 'a,
    {
        Call::new(format!("({:?} && {:?})", self, aswell), move |b| {
            self.accept(b) && aswell.accept(b)
        })
    }
}
impl<T> Predicate for T
where
    T: DerefMut,
    T::Target: FnMut(&u8) -> bool,
    T: fmt::Debug + Send,
{
    fn accept(&mut self, byte: &u8) -> bool {
        self(byte)
    }
}
*/

#[test]
fn repeat_combines_to_matcher() {
    let mut sut = not(ws()).repeat(2..3);
    assert_eq!(sut(&b'1'), MatchResult::Incomplete);
    assert_eq!(sut(&b'2'), MatchResult::Complete);
    assert_eq!(sut(&b'3'), MatchResult::Fail);
}

#[test]
fn tag_completes() {
    let mut sut = tag("123");
    assert_eq!(sut.matches(&b'1'), MatchResult::Incomplete);
    assert_eq!(sut.matches(&b'2'), MatchResult::Incomplete);
    assert_eq!(sut.matches(&b'3'), MatchResult::Complete);
    assert_eq!(sut.matches(&b'4'), MatchResult::Fail);
    assert_eq!(sut.matches(&b'5'), MatchResult::Fail);
}
#[test]
fn tag_mismatch() {
    let mut sut = tag("123".to_owned());
    assert_eq!(sut.matches(&b'1'), MatchResult::Incomplete);
    assert_eq!(sut.matches(&b'x'), MatchResult::Mismatch);
    assert_eq!(sut.matches(&b'3'), MatchResult::Fail);
    assert_eq!(sut.matches(&b'4'), MatchResult::Fail);
}
#[test]
fn tag_empty() {
    let mut sut = tag(b"");
    assert_eq!(sut.matches(&b'1'), MatchResult::Fail);
}

#[async_std::test]
async fn parse_tag_matches() {
    let mut inp = b"xyz".as_ref();
    let mut rdr = MatchReader::new(&mut inp);
    let rslt = rdr.match_str(tag(b"x")).await.expect("ok");
    assert_eq!(rslt, "x")
}

#[async_std::test]
async fn parse_tag_mismatches() {
    let mut inp = b"xyz".as_ref();
    let mut rdr = MatchReader::new(&mut inp);
    let rslt = rdr.match_str(tag("O")).await.err().expect("err");
    assert_eq!(rslt.kind(), io::ErrorKind::InvalidInput)
}

#[async_std::test]
async fn parse_tag_on_empty_rdr_mismatches() {
    let mut inp = b"".as_ref();
    let mut rdr = MatchReader::new(&mut inp);
    let rslt = rdr.match_str(tag("O")).await.err().expect("err");
    assert_eq!(rslt.kind(), io::ErrorKind::InvalidInput)
}
#[async_std::test]
async fn parse_empty_tag_is_unsupported() {
    let mut inp = b"xyz".as_ref();
    let mut rdr = MatchReader::new(&mut inp);
    let rslt = rdr.match_str(tag("")).await.err().expect("err");
    assert_eq!(rslt.kind(), io::ErrorKind::Unsupported)
}

#[test]
fn fail_ors_mismatch() {
    let mut sut = tag("123").or(tag(""));
    assert_eq!(sut.matches(&b'x'), MatchResult::Fail);

    let mut sut = tag("").or(tag("123"));
    assert_eq!(sut.matches(&b'x'), MatchResult::Fail);
}
#[test]
fn complete_ors_incomplete() {
    let mut sut = tag("123").or(tag("1"));
    assert_eq!(sut.matches(&b'1'), MatchResult::Complete);

    let mut sut = tag("1").or(tag("123"));
    assert_eq!(sut.matches(&b'1'), MatchResult::Complete);
}
#[test]
fn match_ors_incomplete() {
    let mut sut = tag("\r\n").or(eol());
    assert_eq!(sut.matches(&b'\r'), MatchResult::Match);

    let mut sut = eol().or(tag("\r\n"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Match);
}
#[test]
fn match_ors_complete() {
    let mut sut = tag("\r").or(eol());
    assert_eq!(sut.matches(&b'\r'), MatchResult::Match);

    let mut sut = eol().or(tag("\r"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Match);
}
#[test]
fn complete_ors_fail() {
    let mut sut = tag("\r").or(tag(""));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Complete);

    let mut sut = tag("").or(tag("\r"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Complete);
}
#[test]
fn complete_ors_mismatch() {
    let mut sut = tag("\r").or(tag("x"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Complete);

    let mut sut = tag("x").or(tag("\r"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Complete);
}





#[test]
fn fail_ands_mismatch() {
    let mut sut = tag("123").and(tag(""));
    assert_eq!(sut.matches(&b'x'), MatchResult::Fail);

    let mut sut = tag("").and(tag("123"));
    assert_eq!(sut.matches(&b'x'), MatchResult::Fail);
}
#[test]
fn incomplete_ands_complete() {
    let mut sut = tag("123").and(tag("1"));
    assert_eq!(sut.matches(&b'1'), MatchResult::Incomplete);

    let mut sut = tag("1").and(tag("123"));
    assert_eq!(sut.matches(&b'1'), MatchResult::Incomplete);
}
#[test]
fn incomplete_ands_match() {
    let mut sut = tag("\r\n").and(eol());
    assert_eq!(sut.matches(&b'\r'), MatchResult::Incomplete);

    let mut sut = eol().and(tag("\r\n"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Incomplete);
}
#[test]
fn complete_ands_match() {
    let mut sut = tag("\r").and(eol());
    assert_eq!(sut.matches(&b'\r'), MatchResult::Complete);

    let mut sut = eol().and(tag("\r"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Complete);
}
#[test]
fn fail_ands_complete() {
    let mut sut = tag("\r").and(tag(""));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Fail);

    let mut sut = tag("").and(tag("\r"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Fail);
}
#[test]
fn mismatch_ands_complete() {
    let mut sut = tag("\r").and(tag("x"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Mismatch);

    let mut sut = tag("x").and(tag("\r"));
    assert_eq!(sut.matches(&b'\r'), MatchResult::Mismatch);
}
