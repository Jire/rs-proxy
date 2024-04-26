use std::{env, thread};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use getopts::Options;
use tokio::time::sleep;

mod bind;
mod rs2;
mod js5;
mod proxying;
mod timeout_io;
mod proxy;
mod proxy_io;

type BoxedError = Box<dyn Error + Sync + Send + 'static>;

const DEFAULT_TIMEOUT: u64 = 30;

const DEFAULT_READ_TIMEOUT: u64 = 15;
const DEFAULT_WRITE_TIMEOUT: u64 = 15;

fn main() -> Result<(), BoxedError> {
    let args: Vec<String> = env::args().collect();

    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optopt(
        "v",
        "version",
        "the version of the game to proxy",
        "VERSION",
    );
    opts.optmulti(
        "b",
        "bind",
        "defines a bind to proxy, in the form of local_addr,remote_addr",
        "BIND",
    );
    opts.optopt(
        "t",
        "timeout",
        "sets the timeout in seconds to stop after no activity",
        "TIMEOUT",
    );
    opts.optflag("d", "debug", "enable debug mode");

    let matches = match opts.parse(&args[1..]) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("{}", e);
            print_usage(&program, opts);
            std::process::exit(-1);
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, opts);
        return Ok(());
    }

    let debug: bool = matches.opt_present("d");

    let version: u32 = matches.opt_str("v")
        .expect("Version must be present")
        .parse()
        .map_err(|e| format!("Failed to parse version as u32: {}", e))?;

    let timeout = match matches.opt_str("t") {
        Some(s) => s.parse::<u64>().unwrap_or_else(|_| DEFAULT_TIMEOUT),
        None => DEFAULT_TIMEOUT
    };

    let mut handles: Vec<JoinHandle<()>> = Vec::new();

    let num_cons = Arc::new(AtomicU64::new(0));

    {
        let num_cons = Arc::clone(&num_cons);
        let handle = thread::spawn(move || {
            tokio_uring::start(async move {
                check_activity(num_cons, timeout).await;
            });
        });
        handles.push(handle);
    }

    for bind in matches.opt_strs("b") {
        let (local_addr, remote_addr) =
            parse_socket_addrs(&bind).unwrap();

        let num_cons = Arc::clone(&num_cons);

        let handle = thread::spawn(move || {
            tokio_uring::builder()
                .entries(2 << 11)
                .uring_builder(
                    tokio_uring::uring_builder()
                        .setup_cqsize(2 << 12)
                        .setup_sqpoll(20) // backoff at 20ms interval to match client tick
                )
                .start(async move {
                    bind::bind(debug, num_cons, version, local_addr, remote_addr).await;
                });
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Ok(())
}

fn parse_socket_addrs(input: &str) -> Result<(SocketAddr, SocketAddr), String> {
    // Split the input string into two parts: local and remote
    let parts: Vec<&str> = input.split(",").collect();
    let parts_len = parts.len();
    if parts_len != 2 {
        return Err(format!("Input must contain exactly one '{}', but instead contained {}", ',', parts_len));
    }

    // Try to parse the local and remote parts into SocketAddr
    let local_addr = parts[0].parse::<SocketAddr>()
        .map_err(|_| format!("Failed to parse local address '{}'", parts[0]))?;

    let remote_addr = parts[1].parse::<SocketAddr>()
        .map_err(|_| format!("Failed to parse remote address '{}'", parts[1]))?;

    Ok((local_addr, remote_addr))
}

fn print_usage(program: &str, opts: Options) {
    let program_path = std::path::PathBuf::from(program);
    let program_name = program_path.file_stem().unwrap().to_string_lossy();
    let brief = format!(
        "Usage: {} -v VERSION -d LOCAL_HOST:LOCAL_PORT,REMOTE_HOST:REMOTE_PORT [-t TIMEOUT]",
        program_name
    );
    print!("{}", opts.usage(&brief));
}

async fn check_activity(
    num_cons: Arc<AtomicU64>,
    timeout: u64,
) {
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