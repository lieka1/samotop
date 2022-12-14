#![allow(missing_docs)]

use async_std::io::{self, Cursor, Read, Write};
use async_std::pin::Pin;
use async_std::task::{Context, Poll};
use pin_project::pin_project;

pub type MockCursor = Cursor<Vec<u8>>;

#[pin_project]
#[derive(Clone, Debug)]
pub struct MockStream {
    #[pin]
    reader: MockCursor,
    #[pin]
    writer: MockCursor,
}

impl Default for MockStream {
    fn default() -> Self {
        Self::new()
    }
}

impl MockStream {
    pub fn new() -> MockStream {
        MockStream {
            reader: MockCursor::new(Vec::new()),
            writer: MockCursor::new(Vec::new()),
        }
    }

    pub fn with_vec(vec: Vec<u8>) -> MockStream {
        MockStream {
            reader: MockCursor::new(vec),
            writer: MockCursor::new(Vec::new()),
        }
    }

    pub fn take_vec(&mut self) -> Vec<u8> {
        let vec = self.writer.get_ref().to_vec();
        self.writer.set_position(0);
        self.writer.get_mut().clear();
        vec
    }

    pub fn next_vec(&mut self, vec: &[u8]) {
        let cursor = &mut self.reader;
        cursor.set_position(0);
        cursor.get_mut().clear();
        cursor.get_mut().extend_from_slice(vec);
    }

    pub fn swap(&mut self) {
        let cur_write = &mut self.writer;
        let cur_read = &mut self.reader;
        let vec_write = cur_write.get_ref().to_vec();
        let vec_read = cur_read.get_ref().to_vec();
        cur_write.set_position(0);
        cur_read.set_position(0);
        cur_write.get_mut().clear();
        cur_read.get_mut().clear();
        // swap cursors
        cur_read.get_mut().extend_from_slice(vec_write.as_slice());
        cur_write.get_mut().extend_from_slice(vec_read.as_slice());
    }
}

impl Write for MockStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.project();
        let _: Pin<&mut _> = this.writer;
        this.writer.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        let this = self.project();
        let _: Pin<&mut _> = this.writer;
        this.writer.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        let this = self.project();
        let _: Pin<&mut _> = this.writer;
        this.writer.poll_close(cx)
    }
}

impl Read for MockStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        let _: Pin<&mut _> = this.reader;
        this.reader.poll_read(cx, buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use async_std::prelude::*;

    #[async_attributes::test]
    async fn write_take_test() {
        let mut mock = MockStream::new();
        // write to mock stream
        mock.write_all(&[1, 2, 3]).await.expect("should succeed");
        assert_eq!(mock.take_vec(), vec![1, 2, 3]);
    }

    #[async_attributes::test]
    async fn read_with_vec_test() {
        let mut mock = MockStream::with_vec(vec![4, 5]);
        let mut vec = Vec::new();
        mock.read_to_end(&mut vec).await.expect("should succeed");
        assert_eq!(vec, vec![4, 5]);
    }

    #[async_attributes::test]
    async fn swap_test() {
        let mut mock = MockStream::new();
        let mut vec = Vec::new();
        mock.write_all(&[8, 9, 10]).await.expect("should succeed");
        mock.swap();
        mock.read_to_end(&mut vec).await.expect("should succeed");
        assert_eq!(vec, vec![8, 9, 10]);
    }
}
