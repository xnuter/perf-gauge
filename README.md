[![Crate](https://img.shields.io/crates/v/perf-gauge.svg)](https://crates.io/crates/perf-gauge)
![Clippy/Fmt](https://github.com/xnuter/perf-gauge/workflows/Clippy/Fmt/badge.svg)
![Tests](https://github.com/xnuter/perf-gauge/workflows/Tests/badge.svg)
[![Coverage Status](https://coveralls.io/repos/github/xnuter/perf-gauge/badge.svg?branch=main)](https://coveralls.io/github/xnuter/perf-gauge?branch=main)

Overview
========

Benchmarking tool for network services. Currently, limited to HTTP only (H1 or H2, over TCP or TLS).
However, it's easily extendable to other protocols.

It works in the following modes:

1. `ab`-like mode. Just send traffic to an endpoint for a given duration, or a number of requests. 
   1. Unlimited request rate (to find the max throughput).
   1. Choose the request rate and concurrency level.
   1. Measurements are down to `µs`.
1. Increase the request rate linearly, e.g. by `1,000` every minute to see how your service scales with load.
1. It can report metrics to `Prometheus` via a `pushgateway`.

For instance: ![](./examples/prom/http-tunnel-rust-latency.png).

Emitted metrics are:
* `request_count` - counter for all requests
* `success_count` - counter for only successful requests
* `bytes_count` - total bytes transferred
* `response_codes` - counters for response codes (200, 400, etc.)
* `success_latency` - latency histogram of successful requests only
* `error_latency` - latency histogram of failed requests (if any)
* `latency` - latency histogram across all requests
* `latency_{statistic}` - `{statistic} = {min, mean, max, stddev, p50, p90, p99, p99_9, p99_99}` - gauges for latency statistics

You can [read more here](./examples).

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
    -c, --concurrency <CONCURRENCY>          Concurrent clients. Default `1`.
    -d, --duration <DURATION>                Duration of the test.
    -m, --max_iter <MAX_RATE_ITERATIONS>
            The number of iterations with the max rate. By default `1`.

    -n, --num_req <NUMBER_OF_REQUESTS>       Number of requests.
        --prometheus <PROMETHEUS_ADDR>
            If you'd like to send metrics to Prometheus PushGateway, specify the server URL. E.g.
            10.0.0.1:9091

        --prometheus_job <PROMETHEUS_JOB>    Prometheus Job (by default `pushgateway`)
    -r, --rate <RATE>
            Request rate per second. E.g. 100 or 0.1. By default no limit.

        --rate_max <RATE_MAX>                Max rate per second. Requires --rate-step
        --rate_step <RATE_STEP>              Rate increase step (until it reaches --rate_max).
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
    perf-gauge http [FLAGS] [OPTIONS] <TARGET>...

ARGS:
    <TARGET>...    Target, e.g. https://my-service.com:8443/8kb Can be multiple ones (with
                   random choice balancing)

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
$ perf-gauge --concurrency 10 \
               --duration 10s \
               http http://localhost/10kb --conn_reuse
```
  
Parameters:

* `--concurrency 10` - the number of clients generating load concurrently
* `--duration 60s` - step duration `60s`
* `http http://local-nginx.org/10kb --conn_reuse` - run in `http` mode to the given endpoint, reusing connections. 

Reporting performance metrics to Prometheus
===========================================

Another use case, is to increase request rate and see how the latency degrades. 

E.g. increase RPS each minute by 1,000: 

```
export PROMETHEUS_HOST=10.138.0.2

$ perf-gauge --concurrency 10 \
               --rate 1000 --rate_step 1000 --rate_max 25000 \
               --max_iter 15 \
               --duration 60s \
               --name nginx-direct \
               --prometheus $PROMETHEUS_HOST:9091 \
               http https://localhost/10kb --conn_reuse --ignore_cert
```

* `--concurrency 10` - the number of clients generating load concurrently
* `--rate 1000 --rate_step 1000 --rate_max 25000` - start with rate 1000 rps, then add 1000 rps after each step until it reaches 25k.
* `--duration 60s` - step duration `60s`
* `--max_iter 15` - perform `15` iterations at the max rate
* `--name nginx-direct` - the name of the test (used for reporting metrics to `prometheus`)
* `--prometheus $PROMETHEUS_HOST:9091` - push-gateway `host:port` to send metrics to Prometheus.
* `http http://local-nginx.org/10kb --conn_reuse` - run in `https` mode to the given endpoint, reusing connections and not checking the certificate. 
