use std::env;
use std::error::Error;

use getopts::Options;

mod bind;
mod rs2;
mod js5;
mod proxying;

type BoxedError = Box<dyn Error + Sync + Send + 'static>;

fn main() -> Result<(), BoxedError> {
    let args: Vec<String> = env::args().collect();

    let program = args[0].clone();

    let version_str = &args[1]; // Get the first user-supplied argument
    let version: u32 = version_str.parse().map_err(|e| format!("Failed to parse version as u32: {}", e))?;

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
        bind::listen(version, &bind_addr, local_port, remote, timeout).await;
    });

    Ok(())
}

fn print_usage(program: &str, opts: Options) {
    let program_path = std::path::PathBuf::from(program);
    let program_name = program_path.file_stem().unwrap().to_string_lossy();
    let brief = format!(
        "Usage: {} VERSION REMOTE_HOST:PORT [-b BIND_ADDR] [-l LOCAL_PORT] [-t TIMEOUT]",
        program_name
    );
    print!("{}", opts.usage(&brief));
}