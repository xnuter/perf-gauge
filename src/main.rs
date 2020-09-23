/// Copyright 2020 Developers of the service-benchmark project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate derive_builder;

mod bench_session;
mod configuration;
mod http_bench_session;

use crate::bench_session::{BenchClient, BenchRun};
use crate::configuration::{BenchmarkConfig, BenchmarkMode};
use log::warn;
use std::time::Instant;
use tokio::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    init_logger();

    let benchmark_config = BenchmarkConfig::from_command_line().map_err(|e| {
        println!("Failed to process parameters. See ./log/application.log for details");
        e
    })?;

    if benchmark_config.verbose {
        println!("Starting with configuration {:?}", benchmark_config);
    }

    let mut threads = vec![];

    let mut requests = benchmark_config.number_of_requests;
    let mut chunk = benchmark_config.number_of_requests / benchmark_config.concurrency;

    for _ in 0..benchmark_config.concurrency {
        let benchmark_config_copy = benchmark_config.clone();
        requests -= chunk;
        if requests < chunk {
            chunk += requests;
        }
        threads.push(tokio::spawn(async move {
            match &benchmark_config_copy.mode {
                BenchmarkMode::HTTP(http_bench) => {
                    send_load(&benchmark_config_copy, http_bench, chunk).await
                }
            }
        }));
    }

    let mut bench_run = BenchRun::new();

    for t in threads.into_iter() {
        match t.await? {
            Ok(b) => bench_run.merge(&b),
            Err(e) => println!("Cannot run. Error: {}", e),
        };
    }

    println!("{}", bench_run);

    Ok(())
}

async fn send_load(
    benchmark_config: &BenchmarkConfig,
    bench_client: &impl BenchClient,
    request_count: usize,
) -> Result<BenchRun, String> {
    let mut bench_run = BenchRun::new();

    let client = bench_client.build_client()?;

    let mut passed_seconds = 0;
    for i in 0..request_count {
        benchmark_config
            .rate_limiter
            .acquire_one()
            .await
            .expect("Unexpected LeakyBucket.acquire error");

        let start = Instant::now();

        match bench_client.send_request(&client).await {
            Ok(stats) => {
                bench_run.increment(stats.status);
                bench_run.total_bytes += stats.bytes_processed;
                bench_run.total_requests += 1;
            }
            Err(message) => {
                bench_run.increment(message);
                bench_run.total_requests += 1;
            }
        };

        let after_response = Instant::now();
        let elapsed_us = after_response.duration_since(start).as_micros() as u64;

        let duration_since_start = after_response.duration_since(bench_run.bench_begin);
        if duration_since_start.as_secs() > passed_seconds {
            passed_seconds = duration_since_start.as_secs();
            println!("Sent {} requests. Elapsed: {:?}", i, duration_since_start);
        }

        match bench_run.report_latency(elapsed_us) {
            Ok(_) => {}
            Err(e) => {
                warn!("Cannot add histogram value {}. Error: {}", elapsed_us, e);
            }
        }
    }

    Ok(bench_run)
}

fn init_logger() {
    let logger_configuration = "./config/log4rs.yaml";
    if let Err(e) = log4rs::init_file(logger_configuration, Default::default()) {
        panic!(
            "Cannot initialize logger from {}. Aborting execution: {}",
            logger_configuration, e
        )
    }
}
