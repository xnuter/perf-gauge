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
use bytes::Bytes;
use core::fmt;
use derive_builder::Builder;
use http_body_util::{BodyExt, Full};
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Method, Request};
#[cfg(feature = "tls-boring")]
use hyper_boring::HttpsConnector;
#[cfg(feature = "tls-native")]
use hyper_tls::HttpsConnector;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use log::error;
use rand::{thread_rng, Rng};
use serde::Deserialize;
use std::time::Duration;
use std::time::Instant;
#[cfg(feature = "tls-native")]
use tokio_native_tls::TlsConnector;

#[derive(Builder, Deserialize, Clone, Debug)]
pub struct HttpClientConfig {
    #[builder(default)]
    ignore_cert: bool,
    #[builder(default)]
    conn_reuse: bool,
    #[builder(default)]
    http2_only: bool,
    #[builder(default)]
    pub stop_on_errors: Vec<u16>,
}

#[derive(Builder, Deserialize, Clone)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct HttpRequest {
    url: Vec<String>,
    #[builder(default = "Method::GET")]
    #[serde(deserialize_with = "deserialize_method", default = "default_method")]
    method: Method,
    #[builder(default)]
    headers: Vec<(String, Vec<String>)>,
    #[builder(default)]
    body: Bytes,
}

fn default_method() -> Method {
    Method::GET
}

fn deserialize_method<'de, D>(deserializer: D) -> Result<Method, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Method::from_bytes(s.as_bytes()).map_err(serde::de::Error::custom)
}

#[derive(Builder, Deserialize, Clone)]
pub struct HttpBenchAdapter {
    config: HttpClientConfig,
    request: HttpRequest,
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
        if self.config.ignore_cert {
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
    type Client = Client<ProtocolConnector, Full<Bytes>>;

    fn build_client(&self) -> Result<Self::Client, String> {
        Ok(Client::builder(TokioExecutor::new())
            .http2_only(self.config.http2_only)
            .pool_max_idle_per_host(if !self.config.conn_reuse {
                0
            } else {
                usize::MAX
            })
            .build(self.build_connector()))
    }

    async fn send_request(&self, client: &Self::Client) -> RequestStats {
        let start = Instant::now();
        let request = self.request.build_request();
        let response = client.request(request).await;

        match response {
            Ok(r) => {
                let status = r.status().to_string();
                let success = r.status().is_success();

                let fatal_error =
                    !success && self.config.stop_on_errors.contains(&r.status().as_u16());

                let mut body = r.into_body();
                let mut total_size = 0;
                let mut body_error = false;
                while let Some(frame_result) = body.frame().await {
                    match frame_result {
                        Ok(frame) => {
                            if let Some(data) = frame.data_ref() {
                                total_size += data.len();
                            }
                        }
                        Err(_) => {
                            body_error = true;
                            break;
                        }
                    }
                }
                RequestStatsBuilder::default()
                    .bytes_processed(total_size)
                    .status(status)
                    .is_success(success && !body_error)
                    .duration(Instant::now().duration_since(start))
                    .fatal_error(fatal_error)
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
                    .fatal_error(false)
                    .build()
                    .expect("RequestStatsBuilder failed")
            }
        }
    }
}

impl HttpRequest {
    fn build_request(&self) -> Request<Full<Bytes>> {
        let uri = &self.url[thread_rng().gen_range(0..self.url.len())];
        let mut request_builder = Request::builder()
            .method(self.method.clone())
            .uri(uri.as_str());

        if !self.headers.is_empty() {
            for (key, value) in self.headers.iter() {
                request_builder = request_builder.header(
                    HeaderName::from_bytes(key.as_bytes())
                        .expect("Header name must be valid at this point"),
                    HeaderValue::from_str(&value[thread_rng().gen_range(0..value.len())])
                        .expect("Header value must be valid at this point"),
                );
            }
        }

        if !self.body.is_empty() {
            request_builder
                .body(Full::new(self.body.clone()))
                .expect("Error building Request")
        } else {
            request_builder
                .body(Full::new(Bytes::new()))
                .map_err(|e| {
                    error!(
                        "Cannot create url {}, headers: {:?}. Error: {}",
                        uri, self.headers, e
                    );
                })
                .expect("Error building Request")
        }
    }
}

impl HttpRequestBuilder {
    /// Validate request is going to be built from the given settings
    fn validate(&self) -> Result<(), String> {
        // Method is already parsed as Method type, so validation is done at construction time
        Ok(())
    }
}

impl fmt::Display for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Requests={}, first request={}, method={}, headers={:?}, body size={}",
            self.url.len(),
            self.url[0],
            self.method.as_str(),
            self.headers,
            self.body.len()
        )
    }
}

impl fmt::Display for HttpBenchAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Config={:?}, Request={}", self.config, self.request)
    }
}

#[cfg(test)]
mod tests {
    use crate::bench_run::BenchmarkProtocolAdapter;
    use crate::http_bench_session::{
        HttpBenchAdapter, HttpBenchAdapterBuilder, HttpClientConfigBuilder, HttpRequestBuilder,
    };
    use bytes::Bytes;
    use hyper::Method;
    use mockito::Matcher::Exact;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_success_request() {
        let body = "world";
        let mut server = mockito::Server::new_async().await;

        let _m = server
            .mock("GET", "/1")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .match_header("x-header", "value1")
            .match_header("x-another-header", "value2")
            .with_body(body)
            .create_async()
            .await;

        let url = server.url();
        println!("Url: {url}");
        let http_bench = HttpBenchAdapterBuilder::default()
            .request(
                HttpRequestBuilder::default()
                    .url(vec![format!("{url}/1")])
                    .headers(vec![
                        ("x-header".to_string(), vec!["value1".to_string()]),
                        ("x-another-header".to_string(), vec!["value2".to_string()]),
                    ])
                    .build()
                    .unwrap(),
            )
            .config(HttpClientConfigBuilder::default().build().unwrap())
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{stats:?}");
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("200 OK".to_string(), stats.status);
    }

    #[tokio::test]
    async fn test_success_put_request() {
        let body = "world";
        let mut server = mockito::Server::new_async().await;

        let _m = server
            .mock("PUT", "/1")
            .match_header("x-header", "value1")
            .match_header("x-another-header", "value2")
            .match_body(Exact("abcd".to_string()))
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .create_async()
            .await;

        let url = server.url();
        println!("Url: {url}");
        let http_bench = HttpBenchAdapterBuilder::default()
            .request(
                HttpRequestBuilder::default()
                    .url(vec![format!("{url}/1")])
                    .method(Method::PUT)
                    .headers(vec![
                        ("x-header".to_string(), vec!["value1".to_string()]),
                        ("x-another-header".to_string(), vec!["value2".to_string()]),
                    ])
                    .body(Bytes::from_static(b"abcd"))
                    .build()
                    .unwrap(),
            )
            .config(HttpClientConfigBuilder::default().build().unwrap())
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{stats:?}");
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("200 OK".to_string(), stats.status);
    }

    #[tokio::test]
    async fn test_success_post_request() {
        let body = "world";
        let mut server = mockito::Server::new_async().await;

        let _m = server
            .mock("POST", "/1")
            .match_header("x-header", "value1")
            .match_header("x-another-header", "value2")
            .match_body(Exact("abcd".to_string()))
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .create_async()
            .await;

        let url = server.url();
        println!("Url: {url}");
        let http_bench = HttpBenchAdapterBuilder::default()
            .request(
                HttpRequestBuilder::default()
                    .url(vec![format!("{url}/1")])
                    .method(Method::POST)
                    .headers(vec![
                        ("x-header".to_string(), vec!["value1".to_string()]),
                        ("x-another-header".to_string(), vec!["value2".to_string()]),
                    ])
                    .body(Bytes::from_static(b"abcd"))
                    .build()
                    .unwrap(),
            )
            .config(HttpClientConfigBuilder::default().build().unwrap())
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{stats:?}");
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("200 OK".to_string(), stats.status);
    }

    #[tokio::test]
    async fn test_success_multiheader_request() {
        let mut server = mockito::Server::new_async().await;

        let m1 = server
            .mock("GET", "/1")
            .match_header("x-header", "value11")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body("abcd")
            .expect_at_least(1)
            .create_async()
            .await;

        let m2 = server
            .mock("GET", "/1")
            .match_header("x-header", "value12")
            .with_status(201)
            .with_header("content-type", "text/plain")
            .with_body("efg")
            .expect_at_least(1)
            .create_async()
            .await;

        let url = server.url();
        println!("Url: {url}");
        let http_bench = HttpBenchAdapterBuilder::default()
            .request(
                HttpRequestBuilder::default()
                    .url(vec![format!("{url}/1")])
                    .method(Method::GET)
                    .headers(vec![(
                        "x-header".to_string(),
                        vec!["value11".to_string(), "value12".to_string()],
                    )])
                    .build()
                    .unwrap(),
            )
            .config(HttpClientConfigBuilder::default().build().unwrap())
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");

        for _ in 0..128 {
            http_bench.send_request(&client).await;
        }

        m1.assert_async().await;
        m2.assert_async().await;
    }

    #[tokio::test]
    async fn test_failed_request() {
        let body = "world";
        let mut server = mockito::Server::new_async().await;

        let _m = server
            .mock("GET", "/1")
            .with_status(500)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .create_async()
            .await;

        let url = server.url();
        println!("Url: {url}");
        let http_bench = HttpBenchAdapterBuilder::default()
            .request(
                HttpRequestBuilder::default()
                    .url(vec![format!("{url}/1")])
                    .build()
                    .unwrap(),
            )
            .config(HttpClientConfigBuilder::default().build().unwrap())
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");
        let stats = http_bench.send_request(&client).await;

        println!("{stats:?}");
        assert_eq!(body.len(), stats.bytes_processed);
        assert_eq!("500 Internal Server Error".to_string(), stats.status);
    }

    #[tokio::test]
    async fn test_only_http2() {
        let body = "world";
        let mut server = mockito::Server::new_async().await;

        let _m = server
            .mock("GET", "/1")
            .with_status(500)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .create_async()
            .await;

        let url = server.url();
        println!("Url: {url}");
        let http_bench: HttpBenchAdapter = HttpBenchAdapterBuilder::default()
            .request(
                HttpRequestBuilder::default()
                    .url(vec![format!("{url}/1")])
                    .build()
                    .unwrap(),
            )
            .config(
                HttpClientConfigBuilder::default()
                    .http2_only(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let client = http_bench.build_client().expect("Client is built");
        let result = timeout(Duration::from_secs(1), http_bench.send_request(&client)).await;

        let failed = match result {
            Err(_) => true,
            Ok(stats) => !stats.is_success,
        };
        assert!(
            failed,
            "Expected to fail as h2 is not supported by the endpoint"
        );
    }
}
