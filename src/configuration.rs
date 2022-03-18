use crate::bench_session::{BenchSession, BenchSessionBuilder, RateLadder, RateLadderBuilder};
/// Copyright 2020 Developers of the perf-gauge project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use crate::http_bench_session::{
    HttpBenchAdapter, HttpBenchAdapterBuilder, HttpClientConfigBuilder, HttpRequestBuilder,
};
use crate::metrics::{DefaultConsoleReporter, ExternalMetricsServiceReporter};
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use core::fmt;
use rand::Rng;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io;

#[derive(Clone, Debug)]
pub enum BenchmarkMode {
    Http(HttpBenchAdapter),
}

#[derive(Clone, Builder)]
pub struct BenchmarkConfig {
    #[builder(default)]
    pub name: Option<String>,
    #[builder(default)]
    pub continuous: bool,
    #[builder(default)]
    pub verbose: bool,
    #[builder(default = "1")]
    pub concurrency: usize,
    pub rate_ladder: RateLadder,
    pub mode: BenchmarkMode,
    #[builder(default)]
    pub reporters: Vec<Arc<dyn ExternalMetricsServiceReporter + Send + Sync + 'static>>,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    /// Concurrent clients. Default `1`.
    #[clap(short, long, default_value_t = 1)]
    concurrency: usize,
    /// Duration of the test.
    #[clap(short, long)]
    duration: Option<String>,
    /// Number of requests per client.
    #[clap(short, long = "num_req")]
    num_req: Option<usize>,
    /// Test case name. Optional. Can be used for tagging metrics.
    #[clap(short = 'N', long)]
    name: Option<String>,
    /// Request rate per second. E.g. 100 or 0.1. By default no limit.
    #[clap(short, long)]
    rate: Option<f64>,
    /// Rate increase step (until it reaches --rate_max).
    #[clap(long = "rate_step")]
    rate_step: Option<f64>,
    /// Max rate per second. Requires --rate-step
    #[clap(long = "rate_max")]
    rate_max: Option<f64>,
    /// takes_value "The number of iterations with the max rate. By default `1`.
    #[clap(short, long = "max_iter", default_value_t = 1)]
    max_iter: usize,
    /// If it's a part of a continuous run. In this case metrics are not reset at the end to avoid saw-like plots.
    #[clap(long)]
    continuous: bool,
    /// If you'd like to send metrics to Prometheus PushGateway, specify the server URL. E.g. 10.0.0.1:9091
    #[clap(long)]
    prometheus: Option<String>,
    /// Prometheus Job (by default `pushgateway`)
    #[clap(long = "prometheus_job")]
    prometheus_job: Option<String>,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Http(HttpOptions),
}

#[derive(Args, Debug)]
#[clap(about = "Run in HTTP(S) mode", long_about = None)]
#[clap(author, version, long_about = None)]
#[clap(propagate_version = true)]
struct HttpOptions {
    /// Target, e.g. https://my-service.com:8443/8kb Can be multiple ones (with random choice balancing).
    #[clap()]
    target: Vec<String>,
    /// Headers in "Name:Value1" form. E.g. `-H "Authentication:Bearer token" -H "Date:2022-03-17"`
    /// It can contain multiple values, e.g. "Name:Value1:Value2:Value3". In this case a random one is chosen for each request.
    #[clap(short = 'H', long)]
    header: Vec<String>,
    /// Method. By default GET.
    #[clap(short = 'M', long)]
    method: Option<String>,
    /// Stop immediately on error codes. E.g. `-E 401 -E 403`
    #[clap(short = 'E', long = "error_stop")]
    error_stop: Vec<u16>,
    /// Body of the request. Could be either `random://[0-9]+`, `file://$filename` or `base64://${valid_base64}`. Optional.
    #[clap(short = 'B', long)]
    body: Option<String>,
    /// Allow self signed certificates.
    #[clap(long = "ignore_cert")]
    ignore_cert: bool,
    /// If connections should be re-used.
    #[clap(long = "conn_reuse")]
    conn_reuse: bool,
    /// Enforce HTTP/2 only.
    #[clap(long = "http2_only")]
    http2_only: bool,
}

impl BenchmarkConfig {
    pub fn from_command_line() -> io::Result<BenchmarkConfig> {
        let cli = Cli::parse();

        let concurrency = cli.concurrency;
        let rate_per_second = cli.rate;
        let rate_step = cli.rate_step;
        let rate_max = cli.rate_max;
        let max_rate_iterations = cli.max_iter;

        let duration = cli.duration.as_ref().map(|d| {
            humantime::Duration::from_str(d.as_str())
                .expect("Illegal duration")
                .into()
        });

        let number_of_requests = cli.num_req;

        if duration.is_none() && number_of_requests.is_none() {
            panic!("Either the number of requests or the test duration must be specified");
        }

        let rate_ladder = if let Some(rate_max) = rate_max {
            let rate_per_second =
                rate_per_second.expect("RATE is required if RATE_MAX is specified");
            let rate_step = rate_step.expect("RATE_STEP is required if RATE_MAX is specified");
            RateLadderBuilder::default()
                .start(rate_per_second)
                .end(rate_max)
                .rate_increment(Some(rate_step))
                .step_duration(duration)
                .step_requests(number_of_requests)
                .max_rate_iterations(max_rate_iterations)
                .build()
                .expect("RateLadderBuilder failed")
        } else {
            let rps = rate_per_second.unwrap_or(0.0);
            RateLadderBuilder::default()
                .start(rps)
                .end(rps)
                .rate_increment(None)
                .step_duration(duration)
                .step_requests(number_of_requests)
                .max_rate_iterations(max_rate_iterations)
                .build()
                .expect("RateLadderBuilder failed")
        };

        Ok(BenchmarkConfigBuilder::default()
            .name(cli.name.clone())
            .rate_ladder(rate_ladder)
            .concurrency(concurrency)
            .verbose(false)
            .continuous(cli.continuous)
            .mode(BenchmarkConfig::build_mode(&cli))
            .reporters(BenchmarkConfig::build_metric_destinations(
                cli.name.clone(),
                &cli,
            ))
            .build()
            .expect("BenchmarkConfig failed"))
    }

    #[cfg(not(feature = "report-to-prometheus"))]
    fn build_metric_destinations(
        test_case_name: Option<String>,
        args: &Cli,
    ) -> Vec<Arc<dyn ExternalMetricsServiceReporter + Send + Sync>> {
        use std::process::exit;

        if args.prometheus.is_some() {
            println!("Prometheus is not supported in this configuration");
            exit(-1);
        }

        vec![Arc::new(DefaultConsoleReporter::new(test_case_name))]
    }

    #[cfg(feature = "report-to-prometheus")]
    fn build_metric_destinations(
        test_case_name: Option<String>,
        args: &Cli,
    ) -> Vec<Arc<dyn ExternalMetricsServiceReporter + Send + Sync>> {
        use crate::prometheus_reporter::PrometheusReporter;
        use std::net::SocketAddr;

        let mut metrics_destinations: Vec<
            Arc<dyn ExternalMetricsServiceReporter + Send + Sync + 'static>,
        > = vec![Arc::new(DefaultConsoleReporter::new(
            test_case_name.clone(),
        ))];

        if let Some(prometheus_addr) = &args.prometheus {
            if SocketAddr::from_str(prometheus_addr.as_str()).is_err() {
                panic!("Illegal Prometheus Gateway addr `{}`", prometheus_addr);
            }
            metrics_destinations.push(Arc::new(PrometheusReporter::new(
                test_case_name,
                prometheus_addr.to_string(),
                Some(
                    args.prometheus_job
                        .as_ref()
                        .unwrap_or(&"pushgateway".to_string())
                        .clone()
                        .as_str(),
                ),
            )));
        }

        metrics_destinations
    }

    fn build_mode(args: &Cli) -> BenchmarkMode {
        match &args.command {
            Commands::Http(config) => {
                #[cfg(feature = "tls-boring")]
                if config.ignore_cert {
                    use std::process::exit;

                    println!("--ignore_cert is not supported for BoringSSL");
                    exit(-1);
                }

                let http_config = HttpBenchAdapterBuilder::default()
                    .config(
                        HttpClientConfigBuilder::default()
                            .ignore_cert(config.ignore_cert)
                            .conn_reuse(config.conn_reuse)
                            .http2_only(config.http2_only)
                            .stop_on_errors(config.error_stop.clone())
                            .build()
                            .expect("HttpClientConfigBuilder failed"),
                    )
                    .request(
                        HttpRequestBuilder::default()
                            .url(config.target.clone())
                            .method(config.method.as_ref().unwrap_or(&"GET".to_string()).clone())
                            .headers(
                                config
                                    .header
                                    .iter()
                                    .map(|s| {
                                        let mut split = s.split(':');
                                        (
                                            split
                                                .next()
                                                .expect("Header name is missing")
                                                .to_string(),
                                            split.map(String::from).collect::<Vec<String>>(),
                                        )
                                    })
                                    .collect(),
                            )
                            .body(BenchmarkConfig::generate_body(config))
                            .build()
                            .expect("HttpRequestBuilder failed"),
                    )
                    .build()
                    .expect("BenchmarkModeBuilder failed");
                BenchmarkMode::Http(http_config)
            }
        }
    }

    fn generate_body(args: &HttpOptions) -> Vec<u8> {
        const RANDOM_PREFIX: &str = "random://";
        const BASE64_PREFIX: &str = "base64://";
        const FILE_PREFIX: &str = "file://";

        if let Some(body_value) = &args.body {
            if let Some(body_size) = body_value.strip_prefix(RANDOM_PREFIX) {
                BenchmarkConfig::generate_random_vec(body_size)
            } else if let Some(base64) = body_value.strip_prefix(BASE64_PREFIX) {
                base64::decode(base64).expect("Invalid base64")
            } else if let Some(filename) = body_value.strip_prefix(FILE_PREFIX) {
                BenchmarkConfig::read_file_as_vec(filename)
            } else {
                panic!("Unsupported format: {}", body_value);
            }
        } else {
            Vec::new()
        }
    }

    fn generate_random_vec(size: &str) -> Vec<u8> {
        let body_size = size
            .parse::<u32>()
            .expect("Body must have format 'RND:NUMBER', where NUMBER is a positive integer");
        let mut rng = rand::thread_rng();
        let random_data: Vec<u8> = (0..body_size).map(|_| rng.gen()).collect();
        random_data
    }

    fn read_file_as_vec(filename: &str) -> Vec<u8> {
        let mut f = File::open(&filename).expect("File not found");
        let metadata = fs::metadata(&filename).expect("Cannot get metadata");
        let mut buffer = vec![0; metadata.len() as usize];
        f.read_exact(&mut buffer)
            .map_err(|e| panic!("Error reading file {}: {}", filename, e))
            .unwrap();

        buffer
    }

    pub fn new_bench_session(&mut self) -> BenchSession {
        BenchSessionBuilder::default()
            .concurrency(self.concurrency)
            .rate_ladder(self.rate_ladder.clone())
            .mode(Arc::new(self.mode.clone()))
            .build()
            .expect("BenchSessionBuilder failed")
    }
}

impl fmt::Display for BenchmarkConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Mode={:?}, RateLadder={:?}, Concurrency={}",
            self.mode, self.rate_ladder, self.concurrency
        )
    }
}
