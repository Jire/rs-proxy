use std::fmt;

use bytes::BytesMut;
use proxy_header::Error::{BufferTooShort, Invalid};
use thiserror::Error;
use tokio::io;
use tokio_uring::net::TcpStream;

use crate::timeout_io::TimeoutTcpStream;

pub const PROXY_EXPECTED_HEADER_BYTES: usize = 28;

#[derive(Debug, Error)]
pub(crate) enum ProxyTcpStreamError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Proxy header error: {0}")]
    ProxyHeader(#[from] proxy_header::Error),
}

pub(crate) trait ProxyTcpStream {
    async fn read_proxy_header(&self, timeout: u64)
                               -> Result<ProxiedAddresses, ProxyTcpStreamError>;
}

const GREETING: &[u8] = b"\r\n\r\n\x00\r\nQUIT\n";
const GREETING_LEN: usize = GREETING.len();

const BYTES: usize = 4;

pub(crate) struct ProxiedAddress(pub(crate) [u8; 4]);

impl fmt::Display for ProxiedAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

pub struct ProxiedAddresses {
    /// Source address (this is the address of the actual client)
    pub source: ProxiedAddress,

    /// Destination address (this is the address of the proxy)
    pub destination: ProxiedAddress,
}

fn parse_addrs(
    buf: BytesMut,
    pos: &mut usize,
    rest: &mut usize,
) -> Result<ProxiedAddresses, proxy_header::Error> {
    if buf.len() < *pos + BYTES * 2 + 4 {
        return Err(BufferTooShort);
    }
    if *rest < BYTES * 2 + 4 {
        return Err(Invalid);
    }

    let ret = ProxiedAddresses {
        source: ProxiedAddress(buf[*pos..*pos + BYTES].try_into().unwrap()),
        destination: ProxiedAddress(buf[*pos + BYTES..*pos + BYTES * 2].try_into().unwrap()),
    };

    *rest -= BYTES * 2 + 4;
    *pos += BYTES * 2 + 4;

    Ok(ret)
}

impl ProxyTcpStream for TcpStream {
    async fn read_proxy_header(&self, timeout: u64)
                               -> Result<ProxiedAddresses, ProxyTcpStreamError> {

        // Read data into the buffer with a specified timeout
        let buf = TimeoutTcpStream::read_buf(self,
                                             BytesMut::with_capacity(PROXY_EXPECTED_HEADER_BYTES),
                                             timeout)
            .await
            .map_err(ProxyTcpStreamError::Io)?;

        let mut pos = 0;

        if buf.len() < 4 + GREETING_LEN {
            return Err(ProxyTcpStreamError::ProxyHeader(BufferTooShort));
        }
        if !buf.starts_with(GREETING) {
            return Err(ProxyTcpStreamError::ProxyHeader(Invalid));
        }
        pos += GREETING_LEN;

        match buf[pos] {
            0x20 => return Err(ProxyTcpStreamError::ProxyHeader(Invalid)), // local not allowed
            0x21 => false,
            _ => return Err(ProxyTcpStreamError::ProxyHeader(Invalid)),
        };

        let protocol = buf[pos + 1];

        let mut rest = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;

        if buf.len() < pos + rest {
            return Err(ProxyTcpStreamError::ProxyHeader(BufferTooShort));
        }

        if protocol == 0x11 {
            Ok(parse_addrs(buf, &mut pos, &mut rest)?)
        } else {
            Err(ProxyTcpStreamError::ProxyHeader(Invalid))
        }
    }
}