use crate::bench_run::BenchmarkProtocolAdapter;
/// Copyright 2020 Developers of the perf-gauge project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use crate::metrics::{RequestStats, RequestStatsBuilder};
use async_trait::async_trait;
use log::error;
use reqwest::{Proxy, Request};
use std::time::{Duration, Instant};

#[derive(Builder, Deserialize, Clone, Debug)]
pub struct HttpBenchAdapter {
    url: String,
    tunnel: Option<String>,
    ignore_cert: bool,
    conn_reuse: bool,
    store_cookies: bool,
    verbose: bool,
    http2_only: bool,
}

#[async_trait]
impl BenchmarkProtocolAdapter for HttpBenchAdapter {
    type Client = reqwest::Client;

    fn build_client(&self) -> Result<Self::Client, String> {
        let mut client_builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.ignore_cert)
            .user_agent("perf-gauge, v0.1.0")
            .connection_verbose(self.verbose)
            .connect_timeout(Duration::from_secs(10));

        if self.http2_only {
            client_builder = client_builder.http2_prior_knowledge();
        }

        if !self.conn_reuse {
            client_builder = client_builder.pool_max_idle_per_host(0);
        }

        if let Some(tunnel) = &self.tunnel {
            let proxy = Proxy::all(tunnel).map_err(|e| e.to_string())?;
            client_builder = client_builder.proxy(proxy);
        }

        client_builder.build().map_err(|e| e.to_string())
    }

    async fn send_request(&self, client: &Self::Client) -> RequestStats {
        let start = Instant::now();

        let request = self.build_request(client);

        let response = client.execute(request).await;

        match response {
            Ok(r) => RequestStatsBuilder::default()
                .bytes_processed(r.content_length().unwrap_or(0) as usize)
                .status(r.status().to_string())
                .is_success(r.status().is_success())
                .duration(Instant::now().duration_since(start))
                .build()
                .expect("RequestStatsBuilder failed"),
            Err(e) => {
                error!("Error sending request: {}", e);
                let status = match e.status() {
                    None => e.to_string(),
                    Some(code) => code.to_string(),
                };
                RequestStatsBuilder::default()
                    .bytes_processed(0)
                    .status(status)
                    .is_success(false)
                    .duration(Instant::now().duration_since(start))
                    .build()
                    .expect("RequestStatsBuilder failed")
            }
        }
    }
}

impl HttpBenchAdapter {
    fn build_request(
        &self,
        client: &<HttpBenchAdapter as BenchmarkProtocolAdapter>::Client,
    ) -> Request {
        // so far simple GET for a given URL.
        // easy to extend with any method + headers + body
        client
            .get(&self.url.clone())
            .build()
            .expect("Error building request")
    }
}

#[cfg(test)]
mod tests {
    use crate::bench_run::BenchmarkProtocolAdapter;
    use crate::http_bench_session::{HttpBenchAdapter, HttpBenchAdapterBuilder};
    use mockito::mock;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_success_request() {
        let body = "world";

        let _m = mock("GET", "/1")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .create();

        let url = mockito::server_url().to_string();
        println!("Url: {}", url);
        let http_bench = HttpBenchAdapterBuilder::default()
            .url(format!("{}/1", url))
            .tunnel(None)
            .ignore_cert(true)
            .conn_reuse(true)
            .store_cookies(true)
            .http2_only(false)
            .verbose(false)
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{:?}", stats);
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("200 OK".to_string(), stats.status);
    }

    #[tokio::test]
    async fn test_failed_request() {
        let body = "world";

        let _m = mock("GET", "/1")
            .with_status(500)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .create();

        let url = mockito::server_url().to_string();
        println!("Url: {}", url);
        let http_bench = HttpBenchAdapterBuilder::default()
            .url(format!("{}/1", url))
            .tunnel(None)
            .ignore_cert(false)
            .conn_reuse(false)
            .store_cookies(false)
            .http2_only(false)
            .verbose(true)
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{:?}", stats);
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("500 Internal Server Error".to_string(), stats.status);
    }

    #[tokio::test]
    async fn test_only_http2() {
        let body = "world";

        let _m = mock("GET", "/1")
            .with_status(500)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .create();

        let url = mockito::server_url().to_string();
        println!("Url: {}", url);
        let http_bench: HttpBenchAdapter = HttpBenchAdapterBuilder::default()
            .url(format!("{}/1", url))
            .tunnel(None)
            .ignore_cert(false)
            .conn_reuse(false)
            .store_cookies(false)
            .http2_only(true)
            .verbose(true)
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");
        let result = timeout(Duration::from_secs(1), http_bench.send_request(&client)).await;

        assert!(
            result.is_err(),
            "Expected to fail as h2 is not supported by the endpoint"
        );
    }
}
