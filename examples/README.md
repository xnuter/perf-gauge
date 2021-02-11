- [Methodology](#methodology)
  * [Types of load](#types-of-load)
  * [Compared metrics](#compared-metrics)
    + [Trimmed mean vs median](#trimmed-mean-vs-median)
  * [Compared configurations](#compared-configurations)
- [Testbed](#testbed)
  * [Prometheus](#prometheus)
  * [Configurations](#configurations)
- [Benchmarks](#benchmarks)

<small><i><a href='http://ecotrust-canada.github.io/markdown-toc/'>Table of contents generated with markdown-toc</a></i></small>

## Methodology

TL;DR; you can jump right to [Benchmarks](#benchmarks) and look into methodology later.

### Types of load

There are three types of load to compare different aspects of TCP proxies:

* `moderate load` - `25k RPS` (requests per second). Connections are being re-used for `50` requests.
  * In this mode, we benchmark handling traffic over persisted connections.
  * Moderate request rate is chosen to benchmark proxies under _normal_ conditions.
* `max load` - sending as many requests as the server can handle.
  * The intent is to test the proxies under stress conditions.
  * Also, we find the max throughput of the service (the saturation point).
* `no-keepalive` - using each connection for a single request
  * So we can compare the performance characteristics of establishing new connections.
  * Establishing a connection is an expensive operation.
  * It involves resource allocation and dispatching tasks between worker threads.
  * As well as clean-up operations once a connection is closed.

### Compared metrics

To compare different solutions, we use the following set of metrics:

* Latency (in microseconds, or `µs`)
  * `p50` (median) - a value that is greater than 50% of observed latency samples
  * `p90` - 90th percentile, or a value that is better than 9 out of 10 latency samples. Usually a good proxy for a perceivable latency by humans.
  * Tail latency `p99` - 99th percentile, the threshold for the worst 1% of samples.
  * Outliers: `p99.9` and `p99.99` - may be important for systems with multiple network hops or large fan-outs (e.g., a request gathers data from tens or hundreds of microservices)
  * `max` - the worst-case.
  * `tm99.9` - trimmed mean, or the mean value of all samples without the best and worst 0.1%. It is more useful than the traditional mean, as it removes a potentially disproportionate influence of outliers: https://en.wikipedia.org/wiki/Truncated_mean
  * `stddev` - the standard deviation of the latency. The lower, the better: https://en.wikipedia.org/wiki/Standard_deviation
* Throughput `rps` (requests per second)
* CPU utilization
* Memory utilization

We primarily focus on the latency and keep an eye on the cost of that latency in terms of CPU/Memory.
For the `max load,` we also assess the maximum possible throughput of the system.

#### Trimmed mean vs median

Why do we need to observe trimmed mean if we already have median (i.e. `p50`)?
`p50` (or percentiles in general) may not necessarily capture performance regressions. For instance:

* `1,2,3,4,5,6,7,8,9,10` - `p50` is `5`, `trimmed mean` is `5`
* `5,5,5,5,5,6,7,8,9,10` - `p50` is still `5`, however the `trimmed mean` is `6.25`.

The same applies to any other percentile. If the team only uses `p90` or `p99` to monitor their system's performance, they may miss dramatic regressions without being aware of that.

Of course, we may use multiple `fences` (`p10`, `p25`, etc.) - but why, if we can use a single metric?
In contrast, the traditional mean is susceptible to noise and outliers and not as good for capturing the general tendency.

### Compared configurations

These benchmarks compare TCP proxies written in different languages, which use Non-blocking I/O.
Why TCP proxies? This is the simplest application dealing with the network I/O. All it does is connection establishment and forward traffic.
Why Non-blocking I/O? You can read [this post](https://medium.com/swlh/distributed-systems-and-asynchronous-i-o-ef0f27655ce5), which tries to demonstrate why
Non-blocking I/O is a much better option for network applications.

Let's say you're building a network service. TCP proxy benchmarks are the lower boundary for the request latency it may have.
Everything else is added on top of that (e.g., parsing, validating, packing, traversing, construction of data, etc.).

So the following solutions are being compared:

* Baseline (`perf-gauge <-> nginx`) - direct communication to Nginx to establish the baseline: https://nginx.org/en/
* HAProxy (`perf-gauge <-> HAProxy <-> nginx`) - HAProxy in TCP-proxy mode. To compare to a mature solution written in `C`: http://www.haproxy.org/
* `draft-http-tunnel` - a simple C++ solution with very basic functionality (asio) (running in TCP mode): https://github.com/cmello/draft-http-tunnel/
* `http-tunnel` - a simple HTTPTunnel written in Rust (tokio) (running in TCP mode): https://github.com/xnuter/http-tunnel/
* `tcp-proxy` - a Golang solution: https://github.com/ickerwx/tcpproxy/
* `NetCrusher` - a Java solution (Java NIO): https://github.com/NetCrusherOrg/NetCrusher-java/
* `pproxy` - a Python solution based on `asyncio` (running in TCP Proxy mode): https://pypi.org/project/pproxy/

Thanks to [Cesar Mello](https://github.com/cmello/) who coded the TCP proxy in C++ to make this benchmark possible.

## Testbed

Benchmarking network services is tricky, especially if we need to measure differences down to microseconds granularity.
To rule out network delays/noise, we can try to employ one of the options:

* use co-located servers, e.g., VMs on the same physical machine or in the same rack.
* use a single VM, but assign CPU cores to different components to avoid overlap

Both are not ideal, but the latter seems to be an easier way. We need to make sure that the instance type is CPU optimized
and won't suffer from noisy-neighbor issues. In other words, it must have exclusive access to all cores as we're going to drive CPU utilization close to 100%.

E.g., if we use an 8-core machine, we can use the following assignment scheme:

* Cores 0-1: Nginx (serves `10kb` of payload per request)
* Cores 2-3: TCP proxy
* Cores 4-7: [perf-gauge](https://github.com/xnuter/perf-gauge) - the load-generator.

This can be achieved by using [cpu sets](https://codywu2010.wordpress.com/2015/09/27/cpuset-by-example/):

```
apt-get install cgroup-tools
```

Then we can create non-overlapping CPU-sets and run different components without competing for CPU and ruling out any network noise.

### Prometheus

`perf-gauge` can emit metrics to `Prometheus.` To launch a stack, you can use https://github.com/xnuter/prom-stack.
I just forked `prom-stack` and removed anything but `prometheus,` `push-gateway` and `grafana.` You can clone the stack and launch `make.`

Then set the variable with the host, for instance:

```
export PROMETHEUS_HOST=10.138.0.2
```

### Configurations

Please note that we disable logging for all configurations to minimize the number of variables and the level of noise.

* [Perf-gauge](./perf-gauge-setup.md)
* [Nginx](nginx-config.md)
* TCP Proxies
  * [HAProxy - C](haproxy-config.md)
  * [draft-http-tunnel - C++](cpp-config.md)
  * [http-tunnel - Rust](rust-config.md)
  * [tcp-proxy - Golang](golang-config.md)
  * [NetCrusher - Java](java-config.md)
  * [pproxy - Python](python-config.md)

## Benchmarks

Okay, we finally got to benchmark results. All benchmark results are split into two batches:

* Baseline, C, C++, Rust - comparing high-performance solutions
* Rust, Golang, Java, Python - comparing memory-safe languages

Yep, Rust belongs to both worlds.

* [Moderate RPS](./moderate-tps.md)
* [Max RPS](./max-tps.md)
* [No keep-alive](./no-keepalive.md)