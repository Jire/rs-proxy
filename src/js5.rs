use std::net::SocketAddr;

use tokio_uring::net::TcpStream;

use proxying::start_proxying;

use crate::proxying;
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn handle_js5(
    expected_version: u32,
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddr,
) {
    match ingress.read_buf(vec![0u8; 4], 30).await {
        Ok(buf) => {
            let version = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
            if expected_version != version {
                return;
            }

            match ingress.write_u8(0, 30).await {
                Ok(_) => {
                    println!("Connecting JS5 {}", ingress_addr);
                    return start_proxying(egress_addr, ingress, ingress_addr, 15).await;
                }
                Err(_e) => return
            }
        }
        Err(_e) => {
            //eprintln!("failed to read from JS5 socket; err = {:?}", _e)
            return;
        }
    }
}