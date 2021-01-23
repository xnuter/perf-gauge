- [No keep-alive](#no-keep-alive)
    * [High-performance (C, C++, Rust)](#high-performance-c-c-rust)
        + [Regular percentiles (p50,90,99)](#regular-percentiles-p509099)
        + [Tail latency (p99.9 and p99.99)](#tail-latency-p999-and-p9999)
        + [Trimmed mean and standard deviation](#trimmed-mean-and-standard-deviation)
        + [CPU](#cpu)
        + [Summary](#summary)
    * [Memory-safe languages (Rust, Golang, Java, Python)](#memory-safe-languages-rust-golang-java-python)
        + [Regular percentiles (p50,90,99)](#regular-percentiles-p509099-1)
        + [Tail latency (p99.9 and p99.99)](#tail-latency-p999-and-p9999-1)
        + [Trimmed mean and standard deviation](#trimmed-mean-and-standard-deviation-1)
        + [CPU and Memory consumption](#cpu-and-memory-consumption)
        + [Summary](#summary-1)
    * [Total summary](#total-summary)
    * [Conclusion](#conclusion)

### No keep-alive

This test benchmarks connection established and clean-up after it's no longer needed.
Only a single request is sent over each connection.

#### High-performance (C, C++, Rust)

##### Regular percentiles (p50,90,99)

HAProxy beats both C++ and Rust for handling new connections even at `p50` level.
C++ is faster than Rust:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/no-keepalive-baseline-c-cpp-rust-p50-99.png)

##### Tail latency (p99.9 and p99.99)

For the tail latency, Rust is better than both C++, but HAProxy is still the best out of these three:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/no-keepalive-baseline-c-cpp-rust-tail.png)

##### Trimmed mean and standard deviation

Same here, HAProxy is noticeably better, Rust is doing slightly worse than C++:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/no-keepalive-baseline-c-cpp-rust-mean.png)

##### CPU

HAProxy is again better than both C++/Rust (which are roughly equal here):

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/no-keepalive-baseline-c-cpp-rust-cpu.png)

##### Summary

| | p50  | p90  | p99 |  p99.9 |  p99.99 | max | tm99 | stddev |
|---|---|---|---|---|---|---|---|---|
| Baseline  | 597  | 882 | 1,230 | 1,620 | 2,140 | 6,260 | 621 | 205 |
| C (HAProxy) | 878  | 1,180 | 1,510 | 2,000 | 2,670 | 8,490 | 901 | 217 |
| C++ (draft-http-tunnel) | 948  | 1,480 | 2,060 | 2,660 | 3,820 | 7,660 | 1,010 | 344 |
| Rust (http-tunnel) | 1,310  | 1,680 | 1,970 | 2,250 | 3,620 | 10,590 | 1,310 | 297 |

#### Memory-safe languages (Rust, Golang, Java, Python)

While Rust and Golang were benchmarked at `3,500` connections per second (CPS), both Java and Python based
solutions were not as capable, and thus were benchmarked at `1,500 cps` level. 

##### Regular percentiles (p50,90,99)

Again, we can see, that at `p50`-`p90` level Golang is somewhat comparable to Rust,
and is even better at `p50` level.
But it quickly lags behind at `p99` level, adding `~1.5 ms`.

Java and Python exhibit substantially higher latencies, but Java `p99` latency is much worse than Python:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/no-keepalive-rust-golang-java-python-p50-99.png)

##### Tail latency (p99.9 and p99.99)

For tail latency Rust is doing substantially better than other, with a close second Golang.
Java latency is appalling:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/no-keepalive-rust-golang-java-python-tail.png)

##### Trimmed mean and standard deviation

In terms of the trimmed median Golang is slightly better but is much worse in terms of variance.
Java's latency is much worse:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/no-keepalive-rust-golang-java-python-mean.png)

##### CPU and Memory consumption

Rust is the leanest of all. Please note, that Java and Python are doing `~40%` of work compared to Rust/Golang,
but Java consumes even more CPU:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/no-keepalive-rust-golang-java-python-cpu.png)

##### Summary

| | p50  | p90  | p99 |  p99.9 |  p99.99 | max | tm99 | stddev |
|---|---|---|---|---|---|---|---|---|
| Rust (http-tunnel) | 1,310  | 1,680 | 1,970 | 2,250 | 3,620 | 10,590 | 1,310 | 297 |
| Tcp-Proxy (Golang) | 1,160  | 1,720 | 3,510  | 8,900 | 9,980 | 13,180 | 1,240 | 634 |
| NetCrusher (Java) | 4,000  | 7,900 | 52,400  | 65,600 | 85,600 | 93,900 | 5,600 | 7,800 |
| pproxy (Python) | 4,030  | 5,960 | 7,380  | 10,340 | 16,940 | 18,350 | 4,150 | 1,400 |

#### Total summary

| | p50  | p90  | p99 |  p99.9 |  p99.99 | max | tm99 | stddev |
|---|---|---|---|---|---|---|---|---|
| Baseline  | 597  | 882 | 1,230 | 1,620 | 2,140 | 6,260 | 621 | 205 |
| C (HAProxy) | 878  | 1,180 | 1,510 | 2,000 | 2,670 | 8,490 | 901 | 217 |
| C++ (draft-http-tunnel) | 948  | 1,480 | 2,060 | 2,660 | 3,820 | 7,660 | 1,010 | 344 |
| Rust (http-tunnel) | 1,310  | 1,680 | 1,970 | 2,250 | 3,620 | 10,590 | 1,310 | 297 |
| Tcp-Proxy (Golang) | 1,160  | 1,720 | 3,510  | 8,900 | 9,980 | 13,180 | 1,240 | 634 |
| NetCrusher (Java) | 4,000  | 7,900 | 52,400  | 65,600 | 85,600 | 93,900 | 5,600 | 7,800 |
| pproxy (Python) | 4,030  | 5,960 | 7,380  | 10,340 | 16,940 | 18,350 | 4,150 | 1,400 |
#### Conclusion

The Rust solution is slightly worse than C/C++ (with HAProxy being the best).
Golang is somewhat comparable to high performance languages, but the variance and tail latencies are substantially worse.

NetCrusher showed the worst performance of all, going into tens of milliseconds even for `p99`.
