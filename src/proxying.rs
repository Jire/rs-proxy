use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

use tokio::{io, time};
use tokio_uring::buf::IoBuf;
use tokio_uring::net::TcpStream;

use crate::{DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT};
use crate::timeout_io::TimeoutTcpStream;

const BUFFER_SIZE: usize = 1024;

pub(crate) async fn start_proxying(
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddr,
    opcode: u8,
) {
    let egress = match connect_with_timeout(egress_addr, DEFAULT_READ_TIMEOUT).await {
        Ok(stream) => stream,
        Err(_e) => {
            eprintln!("Failed to connect to {}; err = {:?}", egress_addr, _e);
            return;
        }
    };

    match egress.write_u8(opcode, DEFAULT_WRITE_TIMEOUT).await {
        Ok(_) => {
            println!("Connected {} with opcode {}", ingress_addr, opcode)
        }
        Err(_) => {
            //eprintln!("Failed to write opcode {} response to {}; err = {:?}", opcode, client_addr, _e);
            return;
        }
    }

    // `read` and `write` take owned buffers (more on that later), and
    // there's no "per-socket" buffer, so they actually take `&self`.
    // which means we don't need to split them into a read half and a
    // write half like we'd normally do with "regular tokio". Instead,
    // we can send a reference-counted version of it. also, since a
    // tokio-uring runtime is single-threaded, we can use `Rc` instead of
    // `Arc`.
    let egress = Rc::new(egress);
    let ingress = Rc::new(ingress);

    // We need to copy in both directions...
    let mut from_ingress = tokio_uring::spawn(
        copy(ingress.clone(), egress.clone())
    );
    let mut from_egress = tokio_uring::spawn(
        copy(egress.clone(), ingress.clone())
    );

    // Stop as soon as one of them errors
    let res = tokio::try_join!(&mut from_ingress, &mut from_egress);
    if let Err(e) = res {
        println!("Connection error: {}", e);
    }

    // Make sure the reference count drops to zero and the socket is
    // freed by aborting both tasks (which both hold a `Rc<TcpStream>`
    // for each direction)
    from_ingress.abort();
    from_egress.abort();
}

async fn connect_with_timeout(
    egress_addr: SocketAddr,
    secs: u64) -> io::Result<TcpStream> {
    match time::timeout(Duration::from_secs(secs), TcpStream::connect(egress_addr)).await {
        Ok(Ok(egress)) => Ok(egress),
        Ok(Err(e)) => Err(e),  // Error from TcpStream::connect
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "Connection timed out")),
    }
}

async fn copy(from: Rc<TcpStream>, to: Rc<TcpStream>) -> Result<(), io::Error> {
    let mut buf = vec![0u8; BUFFER_SIZE];
    loop {
        // things look weird: we pass ownership of the buffer to `read`, and we get
        // it back, _even if there was an error_. There's a whole trait for that,
        // which `Vec<u8>` implements!
        let (res, buf_read) = from.read(buf).await;
        // Propagate errors, see how many bytes we read
        let n = res?;
        if n == 0 {
            // A read of size zero signals EOF (end of file), finish gracefully
            return Ok(());
        }

        // The `slice` method here is implemented in an extension trait: it
        // returns an owned slice of our `Vec<u8>`, which we later turn back
        // into the full `Vec<u8>`
        let (res, buf_write) = to.write(buf_read.slice(..n)).await;
        res?;

        // Later is now, we want our full buffer back.
        // That's why we declared our binding `mut` way back at the start of `copy`,
        // even though we moved it into the very first `TcpStream::read` call.
        buf = buf_write.into_inner();
    }
}