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

use crate::bench_session::BenchRun;
use crate::configuration::{BenchmarkConfig, BenchmarkMode};
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

    for i in 0..benchmark_config.concurrency {
        let benchmark_config_copy = benchmark_config.clone();
        requests -= chunk;
        if requests < chunk {
            chunk += requests;
        }
        threads.push(tokio::spawn(async move {
            match &benchmark_config_copy.mode {
                BenchmarkMode::HTTP(http_bench) => {
                    BenchRun::send_load(i + 1, &benchmark_config_copy, http_bench, chunk).await
                }
            }
        }));
    }

    let mut bench_run = BenchRun::new();

    // merge the results from the concurrent threads
    for t in threads.into_iter() {
        match t.await? {
            Ok(b) => bench_run.merge(&b),
            Err(e) => println!("Cannot run. Error: {}", e),
        };
    }

    println!("{}", bench_run);

    Ok(())
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
