![Clippy/Fmt](https://github.com/xnuter/http-tunnel/workflows/Clippy/Fmt/badge.svg)
![Tests](https://github.com/xnuter/http-tunnel/workflows/Tests/badge.svg)
[![Coverage Status](https://coveralls.io/repos/github/xnuter/service-benchmark/badge.svg?branch=master)](https://coveralls.io/github/xnuter/service-benchmark?branch=master)

Overview
========

Benchmarking tool for network services. Currently, limited to HTTP/1.1 only (over TCP or TLS).

Usage
======

Build with `cargo`
```
$ cargo build --release
$ ./target/release/service-benchmark help 

USAGE:
    service-benchmark [FLAGS] [OPTIONS] --num_req <NUMBER_OF_REQUESTS> [SUBCOMMAND]

FLAGS:
    -v, --verbose    Print debug information. Not recommended for `-n > 500`
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --concurrency <CONCURRENCY>       Concurrent threads. Default `1`.
    -n, --num_req <NUMBER_OF_REQUESTS>    Number of requests.
    -r, --rate <RATE_PER_SECOND>          Request rate per second. E.g. 100 or 0.1. By default no limit.

SUBCOMMANDS:
    help    Prints this message or the help of the given subcommand(s)
    http    Run in HTTP(S) mode

```

Help for the `http` command:

```
$./target/release/service-benchmark help http

USAGE:
    service-benchmark http [FLAGS] [OPTIONS] <TARGET>

ARGS:
    <TARGET>    Target, e.g. https://my-service.com:8443/8kb

FLAGS:
        --conn_reuse       If connections should be re-used
        --http2_only       Enforce HTTP/2 only
        --ignore_cert      Allow self signed certificates. Applies to the target (not proxy).
        --store_cookies    If cookies should be stored
    -h, --help             Prints help information
    -V, --version          Prints version information

OPTIONS:
        --tunnel <TUNNEL>    HTTP Tunnel used for connection, e.g. http://my-proxy.org


```

Example:

Test an endpoint:

```
./target/release/service-benchmark -c 4 -n 50000 http https://my-local-nginx.org/10kb --ignore_cert --conn_reuse

```

via an HTTP tunnel:

```
./target/release/service-benchmark -c 4 -n 50000 http https://my-local-nginx.org/10kb --tunnel http://localhost:8080 --ignore_cert --conn_reuse
```