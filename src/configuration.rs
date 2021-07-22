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
use clap::{clap_app, ArgMatches};
use core::fmt;
use rand::Rng;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::process::exit;
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

impl BenchmarkConfig {
    pub fn from_command_line() -> io::Result<BenchmarkConfig> {
        let matches = clap_app!(myapp =>
            (name: "Performance Gauge")
            (version: "0.1.8")
            (author: "Eugene Retunsky")
            (about: "A tool for gauging performance of network services")
            (@arg CONCURRENCY: --concurrency -c +takes_value "Concurrent clients. Default `1`.")
            (@group duration =>
                (@arg NUMBER_OF_REQUESTS: --num_req -n +takes_value "Number of requests per client.")
                (@arg DURATION: --duration -d +takes_value "Duration of the test.")
            )
            (@arg TEST_CASE_NAME: --name -N +takes_value "Test case name. Optional. Can be used for tagging metrics.")
            (@arg RATE: --rate -r +takes_value "Request rate per second. E.g. 100 or 0.1. By default no limit.")
            (@arg RATE_STEP: --rate_step +takes_value "Rate increase step (until it reaches --rate_max).")
            (@arg RATE_MAX: --rate_max +takes_value "Max rate per second. Requires --rate-step")
            (@arg MAX_RATE_ITERATIONS: --max_iter -m +takes_value "The number of iterations with the max rate. By default `1`.")
            (@arg CONTINUOUS: --continuous "If it's a part of a continuous run. In this case metrics are not reset at the end to avoid saw-like plots.")
            (@arg PROMETHEUS_ADDR: --prometheus +takes_value "If you'd like to send metrics to Prometheus PushGateway, specify the server URL. E.g. 10.0.0.1:9091")
            (@arg PROMETHEUS_JOB: --prometheus_job +takes_value "Prometheus Job (by default `pushgateway`)")
            (@subcommand http =>
                (about: "Run in HTTP(S) mode")
                (version: "0.1.8")
                (@arg IGNORE_CERT: --ignore_cert "Allow self signed certificates.")
                (@arg CONN_REUSE: --conn_reuse "If connections should be re-used")
                (@arg HTTP2_ONLY: --http2_only "Enforce HTTP/2 only")
                (@arg TARGET: +required ... "Target, e.g. https://my-service.com:8443/8kb Can be multiple ones (with random choice balancing)")
                (@arg METHOD: --method -M +takes_value "Method. By default GET")
                (@arg HEADER: --header -H ... "Headers in \"Name:Value\" form. Can be provided multiple times.")
                (@arg BODY: --body -B  +takes_value "Body of the request. Could be either `random://[0-9]+`, `file://$filename` or `base64://${valid_base64}`. Optional.")
            )
        ).get_matches();

        let test_case_name = matches.value_of("TEST_CASE_NAME").map(|s| s.to_string());
        let concurrency = matches.value_of("CONCURRENCY").unwrap_or("1");
        let rate_per_second = matches.value_of("RATE");
        let rate_step = matches.value_of("RATE_STEP");
        let rate_max = matches.value_of("RATE_MAX");
        let max_rate_iterations = matches.value_of("MAX_RATE_ITERATIONS").unwrap_or("1");

        let duration = matches.value_of("DURATION").map(|d| {
            humantime::Duration::from_str(d)
                .expect("Illegal duration")
                .into()
        });

        let number_of_requests = matches
            .value_of("NUMBER_OF_REQUESTS")
            .map(|n| parse_num(n, "Illegal number for NUMBER_OF_REQUESTS"));

        let rate_ladder = if let Some(rate_max) = rate_max {
            let rate_per_second =
                rate_per_second.expect("RATE is required if RATE_MAX is specified");
            let rate_step = rate_step.expect("RATE_STEP is required if RATE_MAX is specified");
            RateLadderBuilder::default()
                .start(parse_num(rate_per_second, "Cannot parse RATE"))
                .end(parse_num(rate_max, "Cannot parse RATE_MAX"))
                .rate_increment(Some(parse_num(rate_step, "Cannot parse RATE_STEP")))
                .step_duration(duration)
                .step_requests(number_of_requests)
                .max_rate_iterations(parse_num(
                    max_rate_iterations,
                    "Cannot parse MAX_RATE_ITERATIONS",
                ))
                .build()
                .expect("RateLadderBuilder failed")
        } else {
            let rps = parse_num(rate_per_second.unwrap_or("0"), "Cannot parse RATE");
            RateLadderBuilder::default()
                .start(rps)
                .end(rps)
                .rate_increment(None)
                .step_duration(duration)
                .step_requests(number_of_requests)
                .max_rate_iterations(parse_num(
                    max_rate_iterations,
                    "Cannot parse MAX_RATE_ITERATIONS",
                ))
                .build()
                .expect("RateLadderBuilder failed")
        };

        Ok(BenchmarkConfigBuilder::default()
            .name(test_case_name.clone())
            .rate_ladder(rate_ladder)
            .concurrency(parse_num(concurrency, "Cannot parse CONCURRENCY"))
            .verbose(false)
            .continuous(matches.is_present("CONTINUOUS"))
            .mode(BenchmarkConfig::build_mode(&matches))
            .reporters(BenchmarkConfig::build_metric_destinations(
                test_case_name,
                matches,
            ))
            .build()
            .expect("BenchmarkConfig failed"))
    }

    #[cfg(not(feature = "report-to-prometheus"))]
    fn build_metric_destinations(
        test_case_name: Option<String>,
        matches: ArgMatches,
    ) -> Vec<Arc<dyn ExternalMetricsServiceReporter + Send + Sync>> {
        if matches.value_of("PROMETHEUS_ADDR").is_some() {
            println!("Prometheus is not supported in this configuration");
            exit(-1);
        }

        vec![Arc::new(DefaultConsoleReporter::new(test_case_name))]
    }

    #[cfg(feature = "report-to-prometheus")]
    fn build_metric_destinations(
        test_case_name: Option<String>,
        matches: ArgMatches,
    ) -> Vec<Arc<dyn ExternalMetricsServiceReporter + Send + Sync>> {
        use crate::prometheus_reporter::PrometheusReporter;
        use std::net::SocketAddr;

        let mut metrics_destinations: Vec<
            Arc<dyn ExternalMetricsServiceReporter + Send + Sync + 'static>,
        > = vec![Arc::new(DefaultConsoleReporter::new(
            test_case_name.clone(),
        ))];

        if let Some(prometheus_addr) = matches.value_of("PROMETHEUS_ADDR") {
            if SocketAddr::from_str(prometheus_addr).is_err() {
                panic!("Illegal Prometheus Gateway addr `{}`", prometheus_addr);
            }
            metrics_destinations.push(Arc::new(PrometheusReporter::new(
                test_case_name,
                prometheus_addr.to_string(),
                matches.value_of("PROMETHEUS_JOB"),
            )));
        }

        metrics_destinations
    }

    fn build_mode(matches: &ArgMatches) -> BenchmarkMode {
        let mode = if let Some(config) = matches.subcommand_matches("http") {
            #[cfg(feature = "tls-boring")]
            if config.is_present("IGNORE_CERT") {
                println!("--ignore_cert is not supported for BoringSSL");
                exit(-1);
            }

            let http_config = HttpBenchAdapterBuilder::default()
                .config(
                    HttpClientConfigBuilder::default()
                        .ignore_cert(config.is_present("IGNORE_CERT"))
                        .conn_reuse(config.is_present("CONN_REUSE"))
                        .http2_only(config.is_present("HTTP2_ONLY"))
                        .build()
                        .expect("HttpClientConfigBuilder failed"),
                )
                .request(
                    HttpRequestBuilder::default()
                        .url(
                            config
                                .values_of("TARGET")
                                .expect("misconfiguration for TARGET")
                                .map(|s| s.to_string())
                                .collect(),
                        )
                        .method(config.value_of("METHOD").unwrap_or("GET").to_string())
                        .headers(BenchmarkConfig::get_multiple_values(config, "HEADER"))
                        .body(BenchmarkConfig::generate_body(config))
                        .build()
                        .expect("HttpRequestBuilder failed"),
                )
                .build()
                .expect("BenchmarkModeBuilder failed");
            BenchmarkMode::Http(http_config)
        } else {
            println!("Run `perf-gauge help` to see program options.");
            exit(1);
        };
        mode
    }

    fn generate_body(config: &ArgMatches) -> Vec<u8> {
        const RANDOM_PREFIX: &str = "random://";
        const BASE64_PREFIX: &str = "base64://";
        const FILE_PREFIX: &str = "file://";

        if let Some(body_value) = config.value_of("BODY") {
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

    fn get_multiple_values(config: &ArgMatches, id: &str) -> Vec<(String, String)> {
        config
            .values_of(id)
            .map(|v| {
                v.map(|s| {
                    let mut split = s.split(':');
                    (
                        split.next().expect("Header name is missing").to_string(),
                        split.collect::<Vec<&str>>().join(":"),
                    )
                })
                .collect()
            })
            .unwrap_or_else(Vec::new)
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

pub fn parse_num<F: FromStr>(s: &str, error_msg: &str) -> F {
    s.parse()
        .map_err(|_| {
            println!("{}", error_msg);
            panic!("Cannot start");
        })
        .unwrap()
}
