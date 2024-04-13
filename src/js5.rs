use std::net::SocketAddr;

use tokio_uring::net::TcpStream;

use proxying::start_proxying;

use crate::proxying;

pub(crate) async fn handle_js5(
    expected_version: u32,
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddr,
) {
    let (result, buf) = ingress.read(vec![0u8; 4]).await;
    match result {
        Ok(size) => {
            if size <= 0 {
                return;
            }

            let version = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
            if expected_version != version {
                return;
            }

            let (result, _) = ingress.write(vec![0u8]).await;
            match result {
                Ok(_) => {
                    println!("Connecting JS5 {}", ingress_addr);
                    return start_proxying(egress_addr, ingress, ingress_addr, 15).await;
                }
                Err(_e) => {
                    return;
                }
            }
        }
        Err(_e) => {
            //eprintln!("failed to read from JS5 socket; err = {:?}", _e)
            return;
        }
    }
}