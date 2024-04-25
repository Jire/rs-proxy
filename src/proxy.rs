use std::net::SocketAddr;

use proxy_header::ProxyHeader;
use tokio_uring::net::TcpStream;

use crate::js5::handle_js5;
use crate::proxy_io::{PROXY_EXPECTED_HEADER_BYTES, PROXY_PARSE_CONFIG};
use crate::rs2::handle_rs2;
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn handle_proxy(
    version: u32,
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddr,
) {
    match ingress.read_buf(vec![0u8; PROXY_EXPECTED_HEADER_BYTES], 15).await {
        Ok(buf) => {
            match ProxyHeader::parse(&buf, PROXY_PARSE_CONFIG) {
                Ok((header, len)) => {
                    match header.proxied_address() {
                        Some(addr) => {
                            println!("Proxied connection from {} to {}", addr.source, addr.destination);

                            let client_buf = &buf[len..];
                            println!("Client sent: {:?}", client_buf);

                            match ingress.read_u8(15).await {
                                Ok(opcode) => {
                                    match opcode {
                                        14 => handle_rs2(egress_addr, ingress, ingress_addr).await,
                                        15 => handle_js5(version, egress_addr, ingress, ingress_addr).await,
                                        _ => {
                                            //println!("Invalid opcode {} from {}", _opcode, client_addr);
                                        }
                                    }
                                }
                                Err(_e) => {} //eprintln!("failed to read from socket; err = {:?}", _e);*/
                            }
                        }
                        None => {
                            println!("Local connection (e.g. healthcheck)");
                        }
                    }
                }
                Err(_) => {}
            };
        }
        Err(_e) => {
            //eprintln!("failed to read from JS5 socket; err = {:?}", _e)
            return;
        }
    }
}