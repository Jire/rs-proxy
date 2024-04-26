use std::net::SocketAddr;

use tokio_uring::net::TcpStream;

use proxying::start_proxying;

use crate::proxy_io::ProxiedAddress;
use crate::proxying;

pub(crate) async fn handle_ping(
    egress_addr: SocketAddr,
    ingress: TcpStream,
    proxied_addr: ProxiedAddress,
) {
    println!("Connecting ping {}", proxied_addr);
    return start_proxying(egress_addr, ingress, proxied_addr, 200).await;
}