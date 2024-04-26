use std::net::SocketAddr;

use bytes::{Buf, BytesMut};
use tokio_uring::net::TcpStream;

use proxying::start_proxying;

use crate::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, proxying};
use crate::proxy_io::ProxiedAddress;
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn handle_js5(
    expected_version: u32,
    egress_addr: SocketAddr,
    ingress: TcpStream,
    proxied_addr: ProxiedAddress,
) {
    match ingress.read_buf(BytesMut::with_capacity(4), DEFAULT_READ_TIMEOUT).await {
        Ok(mut buf) => {
            let version = buf.get_u32();
            if expected_version != version {
                println!("Invalid JS5 version {} (expected {}) from {}",
                         version, expected_version, proxied_addr);
                return;
            }

            match ingress.write_u8(0, DEFAULT_WRITE_TIMEOUT).await {
                Ok(_) => {
                    println!("Connecting JS5 {}", proxied_addr);
                    return start_proxying(egress_addr, ingress, proxied_addr, 15).await;
                }
                Err(_e) => return
            }
        }
        Err(e) => {
            eprintln!("Failed to read from JS5 socket; err = {:?}", e);
            return;
        }
    }
}