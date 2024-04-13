use std::net::SocketAddr;

use tokio_uring::net::TcpStream;

use crate::proxying;

pub(crate) async fn handle_rs2(
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddr,
) {
    let (result, _) = ingress.write_all(vec![0u8]).await;
    match result {
        Ok(_) => {
            println!("Connecting RS2 {}", ingress_addr);
            return proxying::start_proxying(egress_addr, ingress, ingress_addr, 14).await;
        }
        Err(_e) => {
            //eprintln!("failed to write RS2 response to socket; err = {:?}", _e)
            return;
        }
    }
}