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
#[cfg(feature = "tls-boring")]
use boring::ssl::{SslConnector, SslMethod};
use futures_util::StreamExt;
use hyper::client::HttpConnector;
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Body, Method, Request};
#[cfg(feature = "tls-boring")]
use hyper_boring::HttpsConnector;
#[cfg(feature = "tls-native")]
use hyper_tls::HttpsConnector;
use log::error;
use rand::{thread_rng, Rng};
use std::str::FromStr;
use std::time::Duration;
use std::time::Instant;
#[cfg(feature = "tls-native")]
use tokio_native_tls::TlsConnector;

#[derive(Builder, Deserialize, Clone, Debug)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct HttpBenchAdapter {
    url: Vec<String>,
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

#[cfg(feature = "tls")]
type ProtocolConnector = HttpsConnector<HttpConnector>;
#[cfg(not(feature = "tls"))]
type ProtocolConnector = HttpConnector;

impl HttpBenchAdapter {
    #[cfg(not(feature = "tls"))]
    fn build_connector(&self) -> ProtocolConnector {
        self.build_http_connector()
    }

    #[cfg(feature = "tls-native")]
    fn build_connector(&self) -> ProtocolConnector {
        HttpsConnector::from((self.build_http_connector(), self.build_tls_connector()))
    }

    #[cfg(feature = "tls-boring")]
    fn build_connector(&self) -> ProtocolConnector {
        let builder =
            SslConnector::builder(SslMethod::tls()).expect("Cannot build BoringSSL builder");
        hyper_boring::HttpsConnector::with_connector(self.build_http_connector(), builder)
            .expect("Cannot build Boring HttpsConnector")
    }

    #[cfg(feature = "tls-native")]
    fn build_tls_connector(&self) -> TlsConnector {
        let mut native_tls_builder = native_tls::TlsConnector::builder();
        if self.ignore_cert {
            native_tls_builder.danger_accept_invalid_certs(true);
        }

        TlsConnector::from(
            native_tls_builder
                .build()
                .expect("Cannot build TlsConnector"),
        )
    }

    fn build_http_connector(&self) -> HttpConnector {
        let mut connector = HttpConnector::new();
        connector.set_connect_timeout(Some(Duration::from_secs(10)));
        connector.set_nodelay(true);
        #[cfg(feature = "tls")]
        connector.enforce_http(false);
        connector
    }
}

#[async_trait]
impl BenchmarkProtocolAdapter for HttpBenchAdapter {
    type Client = hyper::Client<ProtocolConnector>;

    fn build_client(&self) -> Result<Self::Client, String> {
        let mut client_builder = hyper::Client::builder();

        if self.http2_only {
            client_builder.http2_only(true);
        }

        if !self.conn_reuse {
            client_builder.pool_idle_timeout(None);
        }

        Ok(client_builder.build(self.build_connector()))
    }

    async fn send_request(&self, client: &Self::Client) -> RequestStats {
        let start = Instant::now();

        let request = self.build_request();

        let response = client.request(request).await;

        match response {
            Ok(r) => {
                let status = r.status().to_string();
                let success = r.status().is_success();
                let mut stream = r.into_body();
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
                let status = e.to_string();
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
    fn build_request(&self) -> Request<Body> {
        let method =
            Method::from_str(&self.method.clone()).expect("Method must be valid at this point");

        let mut request_builder = Request::builder()
            .method(method)
            .uri(&self.url[thread_rng().gen_range(0..self.url.len())].clone());

        if !self.headers.is_empty() {
            for (key, value) in self.headers.iter() {
                request_builder = request_builder.header(
                    HeaderName::from_str(key).expect("Header name must be valid at this point"),
                    HeaderValue::from_str(value).expect("Header value must be valid at this point"),
                );
            }
        }

        if !self.body.is_empty() {
            request_builder
                .body(Body::from(self.body.clone()))
                .expect("Error building Request")
        } else {
            request_builder
                .body(Body::empty())
                .expect("Error building Request")
        }
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
            .match_header("x-header", "value1")
            .match_header("x-another-header", "value2")
            .with_body(body)
            .create();

        let url = mockito::server_url().to_string();
        println!("Url: {}", url);
        let http_bench = HttpBenchAdapterBuilder::default()
            .url(vec![format!("{}/1", url)])
            .headers(vec![
                ("x-header".to_string(), "value1".to_string()),
                ("x-another-header".to_string(), "value2".to_string()),
            ])
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{:?}", stats);
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("200 OK".to_string(), stats.status);
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

        let client = http_bench.build_client().expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{:?}", stats);
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("200 OK".to_string(), stats.status);
    }

    #[tokio::test]
    async fn test_success_post_request() {
        let body = "world";

        let _m = mock("POST", "/1")
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
            .method("POST".to_string())
            .headers(vec![
                ("x-header".to_string(), "value1".to_string()),
                ("x-another-header".to_string(), "value2".to_string()),
            ])
            .body("abcd".as_bytes().to_vec())
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
            .url(vec![format!("{}/1", url)])
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
            .url(vec![format!("{}/1", url)])
            .http2_only(true)
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
