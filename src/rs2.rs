use std::net::SocketAddr;

use tokio_uring::net::TcpStream;

use proxying::start_proxying;

use crate::proxying;
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn handle_rs2(
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddr,
) {
    match ingress.write_u8(0, 30).await {
        Ok(_) => {
            println!("Connecting RS2 {}", ingress_addr);
            return start_proxying(egress_addr, ingress, ingress_addr, 14).await;
        }
        Err(_e) => {
            //eprintln!("failed to write RS2 response to socket; err = {:?}", _e)
            return;
        }
    }
}