/// Copyright 2020 Developers of the service-benchmark project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use crate::bench_session::{BenchClient, RequestStats, RequestStatsBuilder};
use async_trait::async_trait;
use log::error;
use reqwest::Proxy;
use std::time::Duration;

#[derive(Builder, Deserialize, Clone, Debug)]
pub struct HttpBenchmark {
    url: String,
    tunnel: Option<String>,
    ignore_cert: bool,
    conn_reuse: bool,
    store_cookies: bool,
    verbose: bool,
    http2_only: bool,
}

#[async_trait]
impl BenchClient for HttpBenchmark {
    type Client = reqwest::Client;

    fn build_client(&self) -> Result<Self::Client, String> {
        let mut client_builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.ignore_cert)
            .user_agent("service-benchmark, v0.1.0")
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

    async fn send_request(&self, client: &Self::Client) -> Result<RequestStats, String> {
        let request = client
            .get(&self.url.clone())
            .build()
            .expect("Error building request");

        let response = client.execute(request).await;

        match response {
            Ok(r) => Ok(RequestStatsBuilder::default()
                .bytes_processed(r.content_length().unwrap_or(0) as usize)
                .status(r.status().to_string())
                .build()
                .expect("RequestStatsBuilder failed")),
            Err(e) => {
                error!("Error sending request: {}", e);
                match e.status() {
                    None => Err(e.to_string()),
                    Some(code) => Err(code.to_string()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bench_session::BenchClient;
    use crate::http_bench_session::{HttpBenchmark, HttpBenchmarkBuilder};
    use mockito::mock;

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
        let http_bench: HttpBenchmark = HttpBenchmarkBuilder::default()
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
        let result = http_bench.send_request(&client).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
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
        let http_bench: HttpBenchmark = HttpBenchmarkBuilder::default()
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
        let result = http_bench.send_request(&client).await;

        assert!(result.is_ok());
        let stats = result.unwrap();
        println!("{:?}", stats);
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("500 Internal Server Error".to_string(), stats.status);
    }
}
