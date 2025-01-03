# rs-proxy

_a simple, cross-platform, multi-client TCP proxy for Old-school RS2/JS5_

`rs-proxy` is a cross-platform, multi-client TCP proxy written in rust, that is designed for those "one-time" tasks
where you usually end up spending more time installing a proxy server and setting up the myriad configuration files and
options than you do actually using it.

`rs-proxy` is completely asynchronous and built on top of the `tokio` async runtime. It was written to serve as an
example of how bidirectional async networking code using rust futures and an async framework would look and is
intentionally kept easy to understand. The code is updated regularly to take advantage of new tokio features and best
practices (if/when they change).

## Usage

`rs-proxy` is a command-line application. One instance of `rs-proxy` should be started for each remote endpoint you wish
to proxy data to/from. All configuration is done via command-line arguments, in keeping with the spirit of this project.

```
rs-proxy VERSION REMOTE_HOST:PORT [-b BIND_ADDR] [-l LOCAL_PORT] [-t TIMEOUT]

Options:
    -b, --bind BIND_ADDR
                        The address on which to listen for incoming requests,
                        defaulting to localhost.
    -l, --local-port LOCAL_PORT
                        The local port to which tcpproxy should bind to
                        listening for requests, randomly chosen otherwise.
                        
    -t, --timeout
                        Sets the timeout in seconds to stop after no activity
```

Where possible, sane defaults for arguments are provided automatically.

## Installation

`rs-proxy` is available via `cargo`, the rust package manager. Installation is as follows:

```
cargo install rs-proxy
```