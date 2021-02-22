/// Copyright 2020 Developers of the perf-gauge project.
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

mod bench_run;
mod bench_session;
mod configuration;
mod http_bench_session;
mod metrics;
mod prometheus_reporter;
mod rate_limiter;

use crate::configuration::BenchmarkConfig;
use crate::metrics::{BenchRunMetrics, ExternalMetricsServiceReporter};
use log::error;
use log::{info, LevelFilter};
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Root};
use log4rs::Config;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use tokio::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut benchmark_config = BenchmarkConfig::from_command_line().map_err(|e| {
        println!("Failed to process parameters. Exiting.");
        e
    })?;

    init_logger();

    info!("Starting with configuration {}", benchmark_config);

    let (reporter_task, batch_metric_sender) =
        create_async_metrics_channel(benchmark_config.reporters.clone());
    let bench_session = benchmark_config.new_bench_session();

    for batch in bench_session {
        info!("Running next batch {:?}", batch);
        let metrics = BenchRunMetrics::new();
        let batch_run_result = batch.run(metrics).await;
        match batch_run_result {
            Ok(stats) => {
                batch_metric_sender.send(stats).unwrap_or_default();
            }
            Err(e) => {
                error!("Unexpected error during batch run: {}", e);
            }
        }
    }

    shutdown(reporter_task, batch_metric_sender);

    Ok(())
}

fn shutdown(reporter_task: JoinHandle<()>, batch_metric_sender: Sender<BenchRunMetrics>) {
    // we need to drop it explicitly, to signal completion to the `mpsc` thread.
    drop(batch_metric_sender);
    reporter_task.join().unwrap_or_default();
    println!("Done gauging performance. Exiting.");
}

fn create_async_metrics_channel(
    metric_reporters: Vec<Arc<Box<dyn ExternalMetricsServiceReporter + Send + Sync + 'static>>>,
) -> (JoinHandle<()>, Sender<BenchRunMetrics>) {
    // We need to report metrics in a separate threads,
    // as at the moment of writing this code not all major metric client libraries
    // had `async` APIs.
    // We can replace it with `tokio::sync::mpsc` and `tokio::spawn` at any time
    let (sender, receiver) = std::sync::mpsc::channel();
    let reporter_task = thread::spawn(move || {
        while let Ok(stats) = receiver.recv() {
            // broadcast to all metrics reporters
            for reporter in &metric_reporters {
                if let Err(e) = reporter.report(&stats) {
                    error!("Error sending metrics: {}", e);
                }
            }
        }
        for reporter in &metric_reporters {
            reporter.shutdown();
        }
    });
    (reporter_task, sender)
}

fn init_logger() {
    let logger_configuration = "./config/log4rs.yaml";
    if log4rs::init_file(logger_configuration, Default::default()).is_err() {
        println!(
            "Cannot find logger configuration at {}. Logging to console.",
            logger_configuration
        );
        let config = Config::builder()
            .appender(
                Appender::builder()
                    .build("application", Box::new(ConsoleAppender::builder().build())),
            )
            .build(
                Root::builder()
                    .appender("application")
                    .build(LevelFilter::Warn),
            )
            .unwrap();
        log4rs::init_config(config).expect("Bug: bad default config");
    }
}
