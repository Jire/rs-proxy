use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::time::sleep;
use tokio_uring::net::TcpListener;

use js5::handle_js5;
use rs2::handle_rs2;

use crate::{js5, rs2};
use crate::timeout_io::TimeoutTcpStream;

pub(crate) async fn bind(
    version: u32,
    bind_ip: &str,
    local_port: i32,
    remote: String,
    timeout: u64) {
    let egress_addr: SocketAddr = remote.parse().unwrap();

    let num_cons = Arc::new(AtomicU64::new(0));

    check_activity(Arc::clone(&num_cons), timeout).await;

    // Listen on the specified IP and port
    let bind_addr = if !bind_ip.starts_with('[') && bind_ip.contains(':') {
        // Correctly format for IPv6 usage
        format!("[{}]:{}", bind_ip, local_port)
    } else {
        format!("{}:{}", bind_ip, local_port)
    };
    let bind_sock = bind_addr
        .parse::<SocketAddr>()
        .expect("Failed to parse bind address");

    let listener = TcpListener::bind(bind_sock).unwrap();
    println!("Listening on {} for RS version {}", listener.local_addr().unwrap(), version);

    while let Ok((ingress, ingress_addr)) = listener.accept().await {
        let num_cons = Arc::clone(&num_cons);

        tokio_uring::spawn(async move {
            num_cons.fetch_add(1, Ordering::SeqCst);

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

            num_cons.fetch_sub(1, Ordering::SeqCst);
        });
    }
}

async fn check_activity(
    num_cons: Arc<AtomicU64>,
    timeout: u64,
) {
    // We can still spawn stuff, but with tokio_uring's `spawn`. The future
    // we send doesn't have to be `Send`, since it's all single-threaded.
    tokio_uring::spawn({
        async move {
            let mut last_activity = Instant::now();

            loop {
                sleep(Duration::from_secs(timeout / 6)).await;

                let connections = num_cons.load(Ordering::SeqCst);
                if connections > 0 {
                    last_activity = Instant::now();
                    println!("{} active connections", connections);
                } else {
                    let idle_time = last_activity.elapsed();
                    println!("Idle for {idle_time:?}");
                    if idle_time > Duration::from_secs(timeout) {
                        println!("Stopping machine. Goodbye!");
                        std::process::exit(0)
                    }
                }
            }
        }
    });
}