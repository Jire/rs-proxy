use std::{env, rc::Rc, sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};
use std::error::Error;
use std::net::SocketAddr;

use getopts::Options;
use tokio::{io, time};
use tokio::time::sleep;
use tokio_uring::{buf::IoBuf, net::{TcpListener, TcpStream}};

type BoxedError = Box<dyn Error + Sync + Send + 'static>;

fn print_usage(program: &str, opts: Options) {
    let program_path = std::path::PathBuf::from(program);
    let program_name = program_path.file_stem().unwrap().to_string_lossy();
    let brief = format!(
        "Usage: {} VERSION REMOTE_HOST:PORT [-b BIND_ADDR] [-l LOCAL_PORT] [-t TIMEOUT]",
        program_name
    );
    print!("{}", opts.usage(&brief));
}

fn main() -> Result<(), BoxedError> {
    let args: Vec<String> = env::args().collect();

    let program = args[0].clone();

    let version_str = &args[1]; // Get the first user-supplied argument
    let version: u32 = version_str.parse().map_err(|e| format!("Failed to parse version as u32: {}", e))?;

    println!("Version: {}", version);

    let mut opts = Options::new();
    opts.optopt(
        "b",
        "bind",
        "The address on which to listen for incoming requests, defaulting to localhost",
        "BIND_ADDR",
    );
    opts.optopt(
        "l",
        "local-port",
        "The local port to which rs-proxy should bind to, randomly chosen otherwise",
        "LOCAL_PORT",
    );
    opts.optopt(
        "t",
        "timeout",
        "Sets the timeout in seconds to stop after no activity",
        "TIMEOUT",
    );

    let matches = match opts.parse(&args[2..]) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("{}", e);
            print_usage(&program, opts);
            std::process::exit(-1);
        }
    };

    let remote = match matches.free.len() {
        1 => matches.free[0].clone(),
        _ => {
            print_usage(&program, opts);
            std::process::exit(-1);
        }
    };

    if !remote.contains(':') {
        eprintln!("A remote port is required (REMOTE_ADDR:PORT)");
        std::process::exit(-1);
    }

    // let local_port: i32 = matches.opt_str("l").unwrap_or("0".to_string()).parse()?;
    let local_port: i32 = matches.opt_str("l").map(|s| s.parse()).unwrap_or(Ok(0))?;

    let bind_addr = matches.opt_str("b").unwrap_or_else(|| "127.0.0.1".to_owned());

    let timeout = match matches.opt_str("t") {
        Some(s) => s.parse::<u64>().unwrap_or_else(|_| 30),
        None => 30
    };

    tokio_uring::start(async move {
        forward(version, &bind_addr, local_port, remote, timeout).await;
    });

    Ok(())
}

async fn forward(
    version: u32,
    bind_ip: &str,
    local_port: i32,
    remote: String,
    timeout: u64) {
    let egress_addr: SocketAddr = remote.parse().unwrap();

    let num_conns: Rc<AtomicU64> = Default::default();

    // We can still spawn stuff, but with tokio_uring's `spawn`. The future
    // we send doesn't have to be `Send`, since it's all single-threaded.
    tokio_uring::spawn({
        let num_conns = num_conns.clone();
        let mut last_activity = Instant::now();

        async move {
            loop {
                if num_conns.load(Ordering::SeqCst) > 0 {
                    last_activity = Instant::now();
                } else {
                    let idle_time = last_activity.elapsed();
                    println!("Idle for {idle_time:?}");
                    if idle_time > Duration::from_secs(timeout) {
                        println!("Stopping machine. Goodbye!");
                        std::process::exit(0)
                    }
                }
                sleep(Duration::from_secs(5)).await;
            }
        }
    });

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
    println!("Listening on {}", listener.local_addr().unwrap());

    while let Ok((ingress, ingress_addr)) = listener.accept().await {
        println!("Accepted connection");

        let num_conns = num_conns.clone();

        tokio_uring::spawn(async move {
            num_conns.fetch_add(1, Ordering::SeqCst);

            let (result, buf) = ingress.read(vec![0u8]).await;
            match result {
                Ok(size) => {
                    if size > 0 {
                        let opcode = buf[0];
                        match opcode {
                            14 => handle_rs2(egress_addr, ingress, ingress_addr).await,
                            15 => handle_js5(version, egress_addr, ingress, ingress_addr).await,
                            _ => {
                                //println!("Invalid opcode {} from {}", _opcode, client_addr);
                            }
                        }
                    }
                }
                Err(_e) => {} //eprintln!("failed to read from socket; err = {:?}", _e);*/
            }

            num_conns.fetch_sub(1, Ordering::SeqCst);
        });
    }
}

async fn copy(from: Rc<TcpStream>, to: Rc<TcpStream>) -> Result<(), io::Error> {
    let mut buf = vec![0u8; 1024];
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

async fn connect_with_timeout(
    egress_addr: SocketAddr,
    secs: u64) -> io::Result<TcpStream> {
    match time::timeout(Duration::from_secs(secs), TcpStream::connect(egress_addr)).await {
        Ok(Ok(egress)) => Ok(egress),
        Ok(Err(e)) => Err(e),  // Error from TcpStream::connect
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "Connection timed out")),
    }
}

async fn handle_rs2(
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddr,
) {
    let (result, _) = ingress.write_all(vec![0u8]).await;
    match result {
        Ok(_) => {
            println!("Connecting RS2 {}", ingress_addr);
            return start_proxying(egress_addr, ingress, ingress_addr, 14).await;
        }
        Err(_e) => {
            //eprintln!("failed to write RS2 response to socket; err = {:?}", _e)
            return;
        }
    }
}

async fn handle_js5(
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

            let (result, _) = ingress.write_all(vec![0u8]).await;
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

async fn start_proxying(
    egress_addr: SocketAddr,
    ingress: TcpStream,
    ingress_addr: SocketAddr,
    opcode: u8,
) {
    // same deal, we need to parse first. if you're puzzled why there's
    // no mention of `SocketAddr` anywhere, it's inferred from what
    // `TcpStream::connect` wants.
    let egress = connect_with_timeout(egress_addr, 30).await.unwrap();

    let (result, _) = egress.write_all(vec![opcode]).await;
    match result {
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