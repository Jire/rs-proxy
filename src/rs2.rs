use std::net::{SocketAddr, SocketAddrV4};

use tokio_uring::net::TcpStream;

use proxying::start_proxying;

use crate::{DEFAULT_WRITE_TIMEOUT, proxying};
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn handle_rs2(
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddrV4,
) {
    match ingress.write_u8(0, DEFAULT_WRITE_TIMEOUT).await {
        Ok(_) => {
            println!("Connecting RS2 {}", ingress_addr);
            return start_proxying(egress_addr, ingress, ingress_addr, 14).await;
        }
        Err(e) => {
            eprintln!("Failed to write RS2 response to socket; err = {:?}", e);
            return;
        }
    }
}