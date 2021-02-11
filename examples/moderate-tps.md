- [Moderate TPS](#moderate-tps)
    * [High-performance (C, C++, Rust)](#high-performance-c-c-rust)
        + [Regular percentiles (p50,90,99)](#regular-percentiles-p509099)
        + [Outliers (p99.9 and p99.99)](#outliers-latency-p999-and-p9999)
        + [Trimmed mean and standard deviation](#trimmed-mean-and-standard-deviation)
        + [CPU and Memory consumption](#cpu-and-memory-consumption)
        + [Summary](#summary)
    * [Memory-safe languages (Rust, Golang, Java, Python)](#memory-safe-languages-rust-golang-java-python)
        + [Regular percentiles (p50,90,99)](#regular-percentiles-p509099-1)
        + [Outliers (p99.9 and p99.99)](#outliers-latency-p999-and-p9999-1)
        + [Trimmed mean and standard deviation](#trimmed-mean-and-standard-deviation-1)
        + [Summary](#summary-1)
    * [Total summary](#total-summary)
    * [Conclusion](#conclusion)

### Moderate TPS

Payload starts at `10,000 rps` with `1,000` increment until it reaches `25,000 rps`.
Then stay there for 10 minutes.

#### High-performance (C, C++, Rust)

##### Regular percentiles (p50,90,99)

We can see that all three add `50-100µs` on top of the baseline, with slightly better latency for C++.

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/baseline-c-cpp-rust-p50-99.png)

##### Outliers (p99.9 and p99.99)

For outliers, the results are somewhat mixed. While for `p99.9` the best results are shown by C++,
for `p99.99` `C++' is the worst, and Rust is the best:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/baseline-c-cpp-rust-tail.png)

##### Trimmed mean and standard deviation

All three are nearly identical, adding `~50µs` to the mean, and `~15µs` to standard deviation:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/baseline-c-cpp-rust-mean.png)

##### CPU and Memory consumption

We can see that CPU utilization for all C, C++, and Rust are close, however,
C++ consumes slightly more than Rust, and Rust consumes slightly more than C.

![](https://raw.githubusercontent.com/perf-gauge/main/examples/prom/baseline-c-cpp-rust-cpu.png)

Memory consumption is on par for C++ and Rust, but HAProxy consumes more memory.
However, the difference is really negligible and on the order of 0.1% of the total memory.

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/baseline-c-cpp-rust-memory.png)

##### Summary

| | p50  | p90  | p99 |  p99.9 |  p99.99 | max | tm99 | stddev |
|---|---|---|---|---|---|---|---|---|
| Baseline  |  190 | 295 | 459 | 718 | 945 | 14,700 | 202 | 83 |
| C (HAProxy) |  240 | 359 | 591 | 987 | 1,460 | 15,880 | 258 | 98 |
| C++ (draft-http-tunnel) | 246  | 368 | 554 | 797 | 1,620 | 16,820 | 263 | 95 |
| Rust (http-tunnel) | 245  | 366 | 594 | 1,020 | 1,380 | 12,310 | 265 | 97 |

#### Memory-safe languages (Rust, Golang, Java, Python)

##### Regular percentiles (p50,90,99)

Please note that for Java and Python, the max TPS was somewhat lowered,
because of the inability of the TCP Proxies to handle more.

So the comparison was between:
* Rust - 25k rps
* Golang - 25k rps
* Java - 15k rps
* Python - 10k rps

Now we can see that at `p50`-`p90` level, Golang is somewhat comparable to Rust,
but quickly deviates at `p99` level, adding a whole millisecond.

Java and Python with lower RPS exhibit substantially higher latencies:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/rust-golang-java-python-p50-99.png)

##### Outliers (p99.9 and p99.99)

Outliers show an even larger difference with Rust, and for Java is the worst of all four:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/rust-golang-java-python-tail.png)

##### Trimmed mean and standard deviation

While the trimmed mean is not dramatically worse for Golang compared to Rust.
But the standard deviation is. Again, Java and Python are well behind both Rust and Golang:

![](https://raw.githubusercontent.com/xnuter/perf-gauge/main/examples/prom/rust-golang-java-python-mean.png)

##### Summary

| | p50  | p90  | p99 |  p99.9 |  p99.99 | max | tm99 | stddev |
|---|---|---|---|---|---|---|---|---|
| Rust (http-tunnel) | 245  | 366 | 594 | 1,020 | 1,380 | 12,310 | 265 | 97 |
| Tcp-Proxy (Golang) | 259 | 387 | 1,510  | 4,910 | 7,970 | 16,160 | 304 | 350 |
| NetCrusher (Java) | 287  | 665 | 1,700  | 8,200 | 35,100 | 44,800 | 377 | 718 |
| pproxy (Python) | 485  | 949 | 2,240  | 4,690 | 6,740 | 16,630 | 585 | 398 |

#### Total summary

| | p50  | p90  | p99 |  p99.9 |  p99.99 | max | tm99 | stddev |
|---|---|---|---|---|---|---|---|---|
| Baseline  |  190 | 295 | 459 | 718 | 945 | 14,700 | 202 | 83 |
| C (HAProxy) |  240 | 359 | 591 | 987 | 1,460 | 15,880 | 258 | 98 |
| C++ (draft-http-tunnel) | 246  | 368 | 554 | 797 | 1,620 | 16,820 | 263 | 95 |
| Rust (http-tunnel) | 245  | 366 | 594 | 1,020 | 1,380 | 12,310 | 265 | 97 |
| Tcp-Proxy (Golang) | 259 | 387 | 1,510  | 4,910 | 7,970 | 16,160 | 304 | 350 |
| NetCrusher (Java) | 287  | 665 | 1,700  | 8,200 | 35,100 | 44,800 | 377 | 718 |
| pproxy (Python) | 485  | 949 | 2,240  | 4,690 | 6,740 | 16,630 | 585 | 398 |

#### Conclusion

The Rust solution is on par with C/C++ solutions at all levels.
Golang is fine at `p50-90`, but the tail latencies and standard deviation are not as good.
NetCrusher and pproxy are unlikely suitable for latency-sensitive applications.
