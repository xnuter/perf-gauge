[![Crate](https://img.shields.io/crates/v/perf-gauge.svg)](https://crates.io/crates/perf-gauge)
![Clippy/Fmt](https://github.com/xnuter/perf-gauge/workflows/Clippy/Fmt/badge.svg)
![Tests](https://github.com/xnuter/perf-gauge/workflows/Tests/badge.svg)
[![Coverage Status](https://coveralls.io/repos/github/xnuter/perf-gauge/badge.svg?branch=master)](https://coveralls.io/github/xnuter/perf-gauge?branch=master)

Overview
========

Benchmarking tool for network services. Currently, limited to HTTP only (H1 or H2, over TCP or TLS).
However, it's easily extendable to other protocols.

Usage
======

Install with `cargo`
```
$ cargo install perf-gauage
$ perf-gauge help 

A tool for gauging performance of network services

USAGE:
    perf-gauge [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
    -v, --verbose    Print debug information. Not recommended for `-n > 500`
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --concurrency <CONCURRENCY>                 Concurrent threads. Default `1`.
    -d, --duration <DURATION>                       Duration of the test.
    -n, --num_req <NUMBER_OF_REQUESTS>              Number of requests.
        --prometheus <PROMETHEUS_ADDR>
            If you'd like to send metrics to Prometheus PushGateway, specify the server URL. E.g.
            10.0.0.1:9091

        --prometheus_job <PROMETHEUS_JOB>           Prometheus Job (by default `pushgateway`)
        --prometheus_label <PROMETHEUS_LABEL>...
            Label for prometheus metrics (absent by default). Format: `key:value`. Multiple labels
            are supported. E.g. `--prometheus_label type:plain-nginx --prometheus_label linear-rate`

    -r, --rate <RATE>
            Request rate per second. E.g. 100 or 0.1. By default no limit.

        --rate_max <RATE_MAX>                       Max rate per second. Requires --rate-step
        --rate_step <RATE_STEP>
            Rate increase step (until it reaches --rate_max).


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
      --prometheus localhost:9091 --prometheus_labels type=plain_nginx \
      http https://my-local-nginx.org/10kb \
      --conn_reuse --ignore_cert \
      --tunnel http://localhost:8080

```

