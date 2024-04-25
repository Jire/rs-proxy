use std::net::SocketAddr;

use tokio_uring::net::TcpStream;

use crate::DEFAULT_READ_TIMEOUT;
use crate::js5::handle_js5;
use crate::proxy_io::ProxyTcpStream;
use crate::rs2::handle_rs2;
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn handle_proxy(
    version: u32,
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddr,
) {
    match ingress.read_proxy_header(DEFAULT_READ_TIMEOUT).await {
        Ok(info) => {
            match info.addr {
                Some(addr) => {
                    println!("Proxied connection from {} to {}", addr.source, addr.destination);

                    match ingress.read_u8(DEFAULT_READ_TIMEOUT).await {
                        Ok(opcode) => {
                            match opcode {
                                14 => handle_rs2(egress_addr, ingress, ingress_addr).await,
                                15 => handle_js5(version, egress_addr, ingress, ingress_addr).await,
                                _ => {
                                    //println!("Invalid opcode {} from {}", _opcode, client_addr);
                                }
                            }
                        }
                        Err(_e) => {}//eprintln!("failed to read from socket; err = {:?}", _e);*/
                    }
                }
                None => println!("Local connection (e.g. healthcheck)")
            }
        }
        Err(_) => {}
    }
}