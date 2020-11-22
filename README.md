[![Crate](https://img.shields.io/crates/v/perf-gauge.svg)](https://crates.io/crates/perf-gauge)
![Clippy/Fmt](https://github.com/xnuter/perf-gauge/workflows/Clippy/Fmt/badge.svg)
![Tests](https://github.com/xnuter/perf-gauge/workflows/Tests/badge.svg)
[![Coverage Status](https://coveralls.io/repos/github/xnuter/perf-gauge/badge.svg?branch=master)](https://coveralls.io/github/xnuter/perf-gauge?branch=master)

Overview
========

Benchmarking tool for network services. Currently, limited to HTTP only (H1 or H2, over TCP or TLS).
However, it's easily extendable to other protocols.

It works in the following modes:

1. `ab`-like mode. Just send traffic to an endpoint for a given duration or a number of requests. 
   1. Unlimited request rate (to find the max throughput).
   1. Choose the request rate and concurrency level.
   1. Measurements are down to `µs`.
1. Increase the request rate linearly, e.g. by `1,000` every minute to see how your service scales with load.
1. It can report metrics to `Prometheus` via a `pushgateway`.

E.g. ![](https://raw.githubusercontent.com/xnuter/http-tunnel/main/misc/benchmarks/http-tunnel-rust.png).

You can [read more here](https://github.com/xnuter/http-tunnel/wiki/Benchmarking-the-HTTP-Tunnel-vs-Chisel-(Golang)).

Usage
======


Install cargo - follow these [instructions](https://doc.rust-lang.org/cargo/getting-started/installation.html).

On Debian to fix [OpenSSL build issue](https://docs.rs/openssl/0.10.30/openssl/). E.g. on Debian:

```
sudo apt-get install pkg-config libssl-dev
```

on `Red-Hat`:
```
sudo dnf install pkg-config openssl-devel
# or
sudo yum install pkg-config openssl-devel
```

Then:
```
$ cargo install perf-gauage
$ perf-gauge help 

A tool for gauging performance of network services

USAGE:
    perf-gauge [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --concurrency <CONCURRENCY>            Concurrent threads. Default `1`.
    -d, --duration <DURATION>                  Duration of the test.
    -m, --max_iter <MAX_RATE_ITERATIONS>
            The number of iterations with the max rate. By default `1`. Requires --rate-step

        --noise_threshold <NOISE_THRESHOLD>
            Noise threshold (in standard deviations) - a positive integer. By default it's `6`,
            which means latency deviated more than 6 stddev from the mean are ignored

    -n, --num_req <NUMBER_OF_REQUESTS>         Number of requests.
        --prometheus <PROMETHEUS_ADDR>
            If you'd like to send metrics to Prometheus PushGateway, specify the server URL. E.g.
            10.0.0.1:9091

        --prometheus_job <PROMETHEUS_JOB>      Prometheus Job (by default `pushgateway`)
    -r, --rate <RATE>
            Request rate per second. E.g. 100 or 0.1. By default no limit.

        --rate_max <RATE_MAX>                  Max rate per second. Requires --rate-step
        --rate_step <RATE_STEP>                Rate increase step (until it reaches --rate_max).
    -N, --name <TEST_CASE_NAME>
            Test case name. Optional. Can be used for tagging metrics.


SUBCOMMANDS:
    help    Prints this message or the help of the given subcommand(s)
    http    Run in HTTP(S) mode
```

Help for the `http` command:

```
$ perf-gauge help http

Run in HTTP(S) mode

USAGE:
    perf-gauge http [FLAGS] [OPTIONS] <TARGET>

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
    -B, --body <BODY>           Body of the request in base64. Optional.
    -H, --header <HEADER>...    Headers in "Name:Value" form. Can be provided multiple times.
    -M, --method <METHOD>       Method. By default GET
        --tunnel <TUNNEL>       HTTP Tunnel used for connection, e.g. http://my-proxy.org
```

For example, test an endpoint using a single run, 5 seconds (max possible request rate):

```
$ perf-gauge -c 4 -d 5s \
              http https://my-local-nginx.org/10kb --ignore_cert --conn_reuse
Duration 5.005778798s 
Requests: 99565 
Request rate: 19890.012 per second
Total bytes: 995.6 MB 
Bitrate: 1591.201 Mbps

Summary:
200 OK: 99565

Latency:
Min    :   137µs
p50    :   191µs
p90    :   243µs
p99    :   353µs
p99.9  :   546µs
p99.99 :  1769µs
Max    : 15655µs
Avg    :   201µs
StdDev :   110µs
```

Reporting performance metrics to Prometheus
===========================================

Another use case, is to increase request rate and see how the latency degrades. 

E.g. increase RPS each minute by 1,000: 

```
perf-gauge -c 2 --rate 1000 --rate_step 1000 --rate_max 20000 \
      -d 60s  \
      -N http-tunnel
      --prometheus localhost:9091 \
      http https://my-local-nginx.org/10kb \
      --conn_reuse --ignore_cert \
      --tunnel http://localhost:8080

```

For example, running the same test in parallel to compare different use-cases:

```bash
#!/usr/bin/env bash

perf-gauge -c 1 --rate 1 --rate_step 1000 --rate_max 5000 \
    -m 5 -d 60s -N http-tunnel --prometheus localhost:9091 \
    http https://my-local-nginx.xnuter.org/10kb \
    --conn_reuse --ignore_cert --tunnel http://localhost:8080 &
    
perf-gauge -c 1 --rate 1 --rate_step 1000 --rate_max 5000 \
    -m 5 -d 60s -N nginx-direct --prometheus localhost:9091 \
    http https://my-local-nginx.xnuter.org/10kb --conn_reuse --ignore_cert &

```
