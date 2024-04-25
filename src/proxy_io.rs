use std::net::{Ipv4Addr, SocketAddrV4};

use proxy_header::Error::{BufferTooShort, Invalid};
use proxy_header::ParseConfig;
use thiserror::Error;
use tokio::io;
use tokio_uring::buf::IoBufMut;
use tokio_uring::net::TcpStream;

use crate::timeout_io::TimeoutTcpStream;

pub const PROXY_PARSE_CONFIG: ParseConfig = ParseConfig {
    include_tlvs: false,

    allow_v1: false,
    allow_v2: true,
};

pub const PROXY_EXPECTED_HEADER_BYTES: usize = 28;

#[derive(Debug, Error)]
pub(crate) enum ProxyTcpStreamError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Proxy header error: {0}")]
    ProxyHeader(#[from] proxy_header::Error),
}

pub(crate) trait ProxyTcpStream {
    async fn read_proxy_header<T: IoBufMut>(&self, timeout: u64)
                                            -> Result<ProxiedInfo, ProxyTcpStreamError>;
}

const GREETING: &[u8] = b"\r\n\r\n\x00\r\nQUIT\n";
const AF_UNIX_ADDRS_LEN: usize = 216;

const BYTES: usize = 4;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ProxiedInfo {
    pub pos: usize,
    pub addr: Option<ProxiedAddress>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ProxiedAddress {
    /// Source address (this is the address of the actual client)
    pub source: SocketAddrV4,
    /// Destination address (this is the address of the proxy)
    pub destination: SocketAddrV4,
}

fn parse_addrs(
    buf: Vec<u8>,
    pos: &mut usize,
    rest: &mut usize,
) -> Result<ProxiedAddress, proxy_header::Error> {
    if buf.len() < *pos + BYTES * 2 + 4 {
        return Err(BufferTooShort);
    }
    if *rest < BYTES * 2 + 4 {
        return Err(Invalid);
    }

    let ret = ProxiedAddress {
        source: SocketAddrV4::new(
            Ipv4Addr::new(buf[*pos], buf[*pos + 1], buf[*pos + 2], buf[*pos + 3]),//Ipv4Addr::from(buf[*pos..*pos + BYTES]),
            u16::from_be_bytes([buf[*pos + BYTES * 2], buf[*pos + BYTES * 2 + 1]]),
        ),
        destination: SocketAddrV4::new(
            Ipv4Addr::new(buf[*pos + BYTES], buf[*pos + BYTES + 1], buf[*pos + BYTES + 2], buf[*pos + BYTES + 3]),//Ipv4Addr::from(buf[*pos + BYTES..*pos + BYTES * 2]),
            u16::from_be_bytes([buf[*pos + BYTES * 2 + 2], buf[*pos + BYTES * 2 + 3]]),
        ),
    };

    *rest -= BYTES * 2 + 4;
    *pos += BYTES * 2 + 4;

    Ok(ret)
}

impl ProxyTcpStream for TcpStream {
    async fn read_proxy_header<T: IoBufMut>(&self, timeout: u64)
                                            -> Result<ProxiedInfo, ProxyTcpStreamError> {

        // Read data into the buffer with a specified timeout
        let buf = TimeoutTcpStream::read_buf(self,
                                             vec![0u8; PROXY_EXPECTED_HEADER_BYTES],
                                             timeout)
            .await
            .map_err(ProxyTcpStreamError::Io)?;

        let mut pos = 0;

        if buf.len() < 4 + GREETING.len() {
            return Err(ProxyTcpStreamError::ProxyHeader(BufferTooShort));
        }
        if !buf.starts_with(GREETING) {
            return Err(ProxyTcpStreamError::ProxyHeader(Invalid));
        }
        pos += GREETING.len();

        let _is_local = match buf[pos] {
            0x20 => true,
            0x21 => false,
            _ => return Err(ProxyTcpStreamError::ProxyHeader(Invalid)),
        };
        let protocol = buf[pos + 1];
        let mut rest = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;

        if buf.len() < pos + rest {
            return Err(ProxyTcpStreamError::ProxyHeader(BufferTooShort));
        }

        let addr = match protocol {
            0x00 => None,
            0x11 => Some(parse_addrs(buf, &mut pos, &mut rest)?),
            //0x12 => Some(parse_addrs::<Ipv4Addr>(buf, &mut pos, &mut rest, Datagram)?),
            //0x21 => Some(parse_addrs::<Ipv6Addr>(buf, &mut pos, &mut rest, Stream)?),
            //0x22 => Some(parse_addrs::<Ipv6Addr>(buf, &mut pos, &mut rest, Datagram)?),
            0x31 | 0x32 => {
                // AF_UNIX - we do not parse this, but don't reject it either in case
                // someone needs the TLVs

                if rest < AF_UNIX_ADDRS_LEN {
                    return Err(ProxyTcpStreamError::ProxyHeader(Invalid));
                }
                rest -= AF_UNIX_ADDRS_LEN;
                pos += AF_UNIX_ADDRS_LEN;

                None
            }
            _ => return Err(ProxyTcpStreamError::ProxyHeader(Invalid)),
        };

        pos += rest;

        return Ok(ProxiedInfo {
            pos,
            addr,
        });
    }
}