use std::net::SocketAddr;

use tokio_uring::net::TcpStream;

use crate::DEFAULT_READ_TIMEOUT;
use crate::js5::handle_js5;
use crate::ping::handle_ping;
use crate::proxy_io::ProxyTcpStream;
use crate::rs2::handle_rs2;
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn handle_proxy(
    version: u32,
    egress_addr: SocketAddr,
    ingress: TcpStream,
    debug: bool,
) {
    match ingress.read_proxy_header(DEFAULT_READ_TIMEOUT).await {
        Ok(proxied_addresses) => {
            let proxied_address = proxied_addresses.source;
            if debug {
                println!("Proxied connection from {} to {}",
                         proxied_address,
                         proxied_addresses.destination);
            }

            match ingress.read_u8(DEFAULT_READ_TIMEOUT).await {
                Ok(opcode) => {
                    match opcode {
                        14 => handle_rs2(egress_addr, ingress, proxied_address).await,
                        15 => handle_js5(version, egress_addr, ingress, proxied_address).await,
                        200 => handle_ping(egress_addr, ingress, proxied_address).await,
                        _ => {
                            drop(ingress);
                            if debug {
                                println!("Invalid opcode {} from {}", opcode, proxied_address);
                            }
                        }
                    }
                }
                Err(e) => {
                    drop(ingress);
                    if debug {
                        eprintln!("Failed to read from socket; err = {:?}", e)
                    }
                }
            }
        }
        Err(e) => {
            drop(ingress);
            if debug {
                eprintln!("Failed to read proxy header from socket; err = {:?}", e);
            }
        }
    }
}