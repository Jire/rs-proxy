use std::net::SocketAddr;

use tokio_uring::net::TcpStream;

use proxying::start_proxying;

use crate::{DEFAULT_WRITE_TIMEOUT, proxying};
use crate::proxy_io::ProxiedAddress;
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn handle_rs2(
    egress_addr: SocketAddr,
    ingress: TcpStream,
    proxied_addr: ProxiedAddress,
) {
    match ingress.write_u8(0, DEFAULT_WRITE_TIMEOUT).await {
        Ok(_) => {
            println!("Connecting RS2 {}", proxied_addr);
            return start_proxying(egress_addr, ingress, proxied_addr, 14).await;
        }
        Err(e) => {
            drop(ingress);
            eprintln!("Failed to write RS2 response to socket; err = {:?}", e);
            return;
        }
    }
}