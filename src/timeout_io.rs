use std::time::Duration;

use bytes::BytesMut;
use io::{Error, ErrorKind};
use tokio::{io, time};
use tokio_uring::buf::{IoBuf, IoBufMut};
use tokio_uring::net::TcpStream;

pub trait TimeoutTcpStream {
    async fn read_buf<T: IoBufMut>(&self, buf: T, timeout: u64) -> io::Result<T>;

    async fn read_u8(&self, timeout: u64) -> io::Result<u8>;

    async fn write_buf<T: IoBuf>(&self, buf: T, buf_len: usize, timeout: u64) -> io::Result<usize>;

    async fn write_u8(&self, value: u8, timeout: u64) -> io::Result<usize>;
}

impl TimeoutTcpStream for TcpStream {
    async fn read_buf<T: IoBufMut>(&self, buf: T, timeout: u64) -> io::Result<T> {
        match time::timeout(Duration::from_secs(timeout),
                            self.read(buf)).await {
            Ok((Ok(size), buf)) => {
                let expected_size = buf.bytes_init();

                if size < expected_size {
                    return Err(Error::new(ErrorKind::UnexpectedEof, "Not enough data left"));
                }

                return Ok(buf);
            }
            Err(_) => Err(Error::new(ErrorKind::TimedOut, "Read timed out")),
            _ => Err(Error::new(ErrorKind::Other, "Other issue occurred"))
        }
    }

    async fn read_u8(&self, timeout: u64) -> io::Result<u8> {
        match time::timeout(Duration::from_secs(timeout),
                            self.read(BytesMut::with_capacity(1))).await {
            Ok((Ok(size), buf)) => {
                if size < 1 {
                    return Err(Error::new(ErrorKind::UnexpectedEof, "No more data left"));
                }

                let u8 = buf[0];
                return Ok(u8);
            }
            Err(_) => Err(Error::new(ErrorKind::TimedOut, "Read timed out")),
            _ => Err(Error::new(ErrorKind::Other, "Other issue occurred"))
        }
    }

    async fn write_buf<T: IoBuf>(&self, buf: T, buf_len: usize, timeout: u64) -> io::Result<usize> {
        match time::timeout(Duration::from_secs(timeout),
                            self.write(buf)).await {
            Ok((Ok(size), _buf)) => {
                if size < buf_len {
                    return Err(Error::new(ErrorKind::UnexpectedEof, "Didn't write all the data"));
                }

                Ok(size)
            }
            Err(_) => Err(Error::new(ErrorKind::TimedOut, "Write timed out")),
            _ => Err(Error::new(ErrorKind::Other, "Other issue occurred"))
        }
    }

    async fn write_u8(&self, value: u8, timeout: u64) -> io::Result<usize> {
        match time::timeout(Duration::from_secs(timeout),
                            self.write(vec![value])).await {
            Ok((Ok(size), _buf)) => {
                if size < 1 {
                    return Err(Error::new(ErrorKind::UnexpectedEof, "Didn't write all the data"));
                }

                Ok(size)
            }
            Err(_) => Err(Error::new(ErrorKind::TimedOut, "Write timed out")),
            _ => Err(Error::new(ErrorKind::Other, "Other issue occurred"))
        }
    }
}