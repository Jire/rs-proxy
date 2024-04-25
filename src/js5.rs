use std::net::{SocketAddr, SocketAddrV4};

use tokio_uring::net::TcpStream;

use proxying::start_proxying;

use crate::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT, proxying};
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn handle_js5(
    expected_version: u32,
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddrV4,
) {
    match ingress.read_buf(vec![0u8; 4], DEFAULT_READ_TIMEOUT).await {
        Ok(buf) => {
            let version = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
            if expected_version != version {
                return;
            }

            match ingress.write_u8(0, DEFAULT_WRITE_TIMEOUT).await {
                Ok(_) => {
                    println!("Connecting JS5 {}", ingress_addr);
                    return start_proxying(egress_addr, ingress, ingress_addr, 15).await;
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