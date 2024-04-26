use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio_uring::net::TcpListener;

use crate::proxy;

pub(crate) async fn bind(
    num_cons: Arc<AtomicU64>,
    version: u32,
    local_addr: SocketAddr,
    remote_addr: SocketAddr) {
    let listener = TcpListener::bind(local_addr).unwrap();
    println!("Listening on {} for RS version {}", listener.local_addr().unwrap(), version);

    while let Ok((ingress, _)) = listener.accept().await {
        let num_cons = Arc::clone(&num_cons);

        tokio_uring::spawn(async move {
            num_cons.fetch_add(1, Ordering::SeqCst);

            proxy::handle_proxy(version, remote_addr, ingress).await;

            num_cons.fetch_sub(1, Ordering::SeqCst);
        });
    }
}