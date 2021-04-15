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
use futures_util::StreamExt;
use log::error;
use rand::{thread_rng, Rng};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Method, Proxy, Request};
use std::str::FromStr;
use std::time::{Duration, Instant};

#[derive(Builder, Deserialize, Clone, Debug)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct HttpBenchAdapter {
    url: Vec<String>,
    #[builder(default)]
    tunnel: Option<String>,
    #[builder(default)]
    ignore_cert: bool,
    #[builder(default)]
    conn_reuse: bool,
    #[builder(default)]
    store_cookies: bool,
    #[builder(default)]
    verbose: bool,
    #[builder(default)]
    http2_only: bool,
    #[builder(default = "\"GET\".to_string()")]
    method: String,
    #[builder(default)]
    headers: Vec<(String, String)>,
    #[builder(default)]
    body: Vec<u8>,
}

#[async_trait]
impl BenchmarkProtocolAdapter for HttpBenchAdapter {
    type Client = reqwest::Client;

    async fn build_client(&self) -> Result<Self::Client, String> {
        let mut client_builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(self.ignore_cert)
            .user_agent("perf-gauge, v0.1.0")
            .connection_verbose(self.verbose)
            .tcp_nodelay(true)
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
            Ok(r) => {
                let mut status = r.status().to_string();
                if let Some(connection_header) = r.headers().get("connection") {
                    if let Ok(value) = connection_header.to_str() {
                        status.push_str(", Connection: ");
                        status.push_str(value);
                    }
                }
                let success = r.status().is_success();
                let mut stream = r.bytes_stream();
                let mut total_size = 0;
                while let Some(item) = stream.next().await {
                    if let Ok(bytes) = item {
                        total_size += bytes.len();
                    } else {
                        break;
                    }
                }
                RequestStatsBuilder::default()
                    .bytes_processed(total_size)
                    .status(status)
                    .is_success(success)
                    .duration(Instant::now().duration_since(start))
                    .build()
                    .expect("RequestStatsBuilder failed")
            }
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
        let method =
            Method::from_str(&self.method.clone()).expect("Method must be valid at this point");

        let mut request_builder = client.request(
            method,
            &self.url[thread_rng().gen_range(0..self.url.len())].clone(),
        );

        if !self.headers.is_empty() {
            let mut headers = HeaderMap::new();
            for (key, value) in self.headers.iter() {
                headers
                    .entry(
                        HeaderName::from_str(&key)
                            .expect("Header name must be valid at this point"),
                    )
                    .or_insert(
                        HeaderValue::from_str(&value)
                            .expect("Header value must be valid at this point"),
                    );
            }
            request_builder = request_builder.headers(headers);
        }

        if !self.body.is_empty() {
            request_builder = request_builder.body(self.body.clone());
        }

        request_builder.build().expect("Illegal request")
    }
}

impl HttpBenchAdapterBuilder {
    /// Validate request is going to be built from the given settings
    fn validate(&self) -> Result<(), String> {
        if let Some(ref m) = self.method {
            Method::from_str(m).map_err(|e| e.to_string()).map(|_| ())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bench_run::BenchmarkProtocolAdapter;
    use crate::http_bench_session::{HttpBenchAdapter, HttpBenchAdapterBuilder};
    use mockito::mock;
    use mockito::Matcher::Exact;
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
            .url(vec![format!("{}/1", url)])
            .build()
            .unwrap();

        let client = http_bench.build_client().await.expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{:?}", stats);
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("200 OK, Connection: close".to_string(), stats.status);
    }

    #[tokio::test]
    async fn test_success_put_request() {
        let body = "world";

        let _m = mock("PUT", "/1")
            .match_header("x-header", "value1")
            .match_header("x-another-header", "value2")
            .match_body(Exact("abcd".to_string()))
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .create();

        let url = mockito::server_url().to_string();
        println!("Url: {}", url);
        let http_bench = HttpBenchAdapterBuilder::default()
            .url(vec![format!("{}/1", url)])
            .method("PUT".to_string())
            .headers(vec![
                ("x-header".to_string(), "value1".to_string()),
                ("x-another-header".to_string(), "value2".to_string()),
            ])
            .body("abcd".as_bytes().to_vec())
            .build()
            .unwrap();

        let client = http_bench.build_client().await.expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{:?}", stats);
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("200 OK, Connection: close".to_string(), stats.status);
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
            .url(vec![format!("{}/1", url)])
            .build()
            .unwrap();

        let client = http_bench.build_client().await.expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{:?}", stats);
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!(
            "500 Internal Server Error, Connection: close".to_string(),
            stats.status
        );
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
            .url(vec![format!("{}/1", url)])
            .http2_only(true)
            .build()
            .unwrap();

        let client = http_bench.build_client().await.expect("Client is built");
        let result = timeout(Duration::from_secs(1), http_bench.send_request(&client)).await;

        assert!(
            result.is_err(),
            "Expected to fail as h2 is not supported by the endpoint"
        );
    }
}
