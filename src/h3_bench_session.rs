/// Copyright 2020 Developers of the perf-gauge project.
///
/// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
/// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
/// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
/// option. This file may not be copied, modified, or distributed
/// except according to those terms.
use crate::bench_run::BenchmarkProtocolAdapter;
use crate::http_bench_session::{HttpClientConfig, HttpRequest};
use crate::metrics::{RequestStats, RequestStatsBuilder};
use async_trait::async_trait;
use bytes::{Buf, Bytes};
use core::fmt;
use derive_builder::Builder;
use h3::client::SendRequest;
use h3_quinn::quinn;
use hyper::Request;
use log::error;
use rustls_pki_types::ServerName;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

#[derive(Builder, Deserialize, Clone)]
pub struct H3BenchAdapter {
    config: HttpClientConfig,
    request: HttpRequest,
}

/// A custom certificate verifier that accepts any server certificate.
/// Used when `--ignore_cert` is specified.
#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &rustls_pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

impl H3BenchAdapter {
    fn build_quinn_client_config(&self) -> quinn::ClientConfig {
        let mut crypto_config = if self.config.ignore_cert {
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(SkipServerVerification::new())
                .with_no_client_auth()
        } else {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth()
        };

        crypto_config.alpn_protocols = vec![b"h3".to_vec()];

        let quic_config = quinn::crypto::rustls::QuicClientConfig::try_from(crypto_config)
            .expect("Failed to create QuicClientConfig");
        quinn::ClientConfig::new(Arc::new(quic_config))
    }

    /// Resolve a URL to (host, port, SocketAddr).
    async fn resolve_target(url: &str) -> Result<(String, u16, SocketAddr), String> {
        let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL: {e}"))?;

        if parsed.scheme() != "https" {
            return Err("HTTP/3 requires https:// URLs".to_string());
        }

        let host = parsed.host_str().ok_or("URL must have a host")?.to_string();
        let port = parsed.port().unwrap_or(443);

        let addr = tokio::net::lookup_host(format!("{host}:{port}"))
            .await
            .map_err(|e| format!("DNS resolution failed: {e}"))?
            .next()
            .ok_or_else(|| format!("No addresses found for {host}:{port}"))?;

        Ok((host, port, addr))
    }
}

/// The h3 client handle used for benchmarking.
/// Wraps the SendRequest handle plus connection metadata needed per-request.
pub struct H3Client {
    send_request: SendRequest<h3_quinn::OpenStreams, Bytes>,
    /// Keep endpoint alive for the lifetime of the client.
    _endpoint: quinn::Endpoint,
}

#[async_trait]
impl BenchmarkProtocolAdapter for H3BenchAdapter {
    type Client = H3Client;

    async fn build_client(&self) -> Result<Self::Client, String> {
        let first_url = self.request.first_url();
        let (host, _port, addr) = Self::resolve_target(first_url).await?;

        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| format!("Failed to create QUIC endpoint: {e}"))?;
        endpoint.set_default_client_config(self.build_quinn_client_config());

        let server_name: ServerName<'_> = host
            .as_str()
            .try_into()
            .map_err(|e| format!("Invalid server name: {e}"))?;

        let connection = endpoint
            .connect(addr, &server_name.to_str())
            .map_err(|e| format!("QUIC connect error: {e}"))?
            .await
            .map_err(|e| format!("QUIC connection failed: {e}"))?;

        let (mut driver, send_request) = h3::client::new(h3_quinn::Connection::new(connection))
            .await
            .map_err(|e| format!("H3 handshake failed: {e}"))?;

        tokio::spawn(async move {
            let e = futures_util::future::poll_fn(|cx| driver.poll_close(cx)).await;
            error!("H3 connection closed: {:?}", e);
        });

        Ok(H3Client {
            send_request,
            _endpoint: endpoint,
        })
    }

    async fn send_request(&self, client: &Self::Client) -> RequestStats {
        let start = Instant::now();

        let (url, method, headers, body) = self.request.request_parts();

        let mut request_builder = Request::builder().method(method).uri(url);

        for (name, value) in headers {
            request_builder = request_builder.header(name, value);
        }

        let request = match request_builder.body(()) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to build H3 request: {}", e);
                return RequestStatsBuilder::default()
                    .bytes_processed(0)
                    .status(e.to_string())
                    .is_success(false)
                    .duration(Instant::now().duration_since(start))
                    .fatal_error(false)
                    .build()
                    .expect("RequestStatsBuilder failed");
            }
        };

        // Clone the send_request handle — opens a new stream on the same QUIC connection
        let mut send_request = client.send_request.clone();

        match send_request.send_request(request).await {
            Ok(mut stream) => {
                // Send body if present
                if !body.is_empty() {
                    if let Err(e) = stream.send_data(body).await {
                        error!("H3 send_data error: {}", e);
                        return RequestStatsBuilder::default()
                            .bytes_processed(0)
                            .status(e.to_string())
                            .is_success(false)
                            .duration(Instant::now().duration_since(start))
                            .fatal_error(false)
                            .build()
                            .expect("RequestStatsBuilder failed");
                    }
                }

                // Signal end of request
                if let Err(e) = stream.finish().await {
                    error!("H3 finish error: {}", e);
                    return RequestStatsBuilder::default()
                        .bytes_processed(0)
                        .status(e.to_string())
                        .is_success(false)
                        .duration(Instant::now().duration_since(start))
                        .fatal_error(false)
                        .build()
                        .expect("RequestStatsBuilder failed");
                }

                // Receive response
                match stream.recv_response().await {
                    Ok(response) => {
                        let status = response.status().to_string();
                        let success = response.status().is_success();
                        let fatal_error = !success
                            && self
                                .config
                                .stop_on_errors
                                .contains(&response.status().as_u16());

                        // Read response body
                        let mut total_size = 0;
                        let mut body_error = false;
                        while let Ok(Some(data)) = stream.recv_data().await {
                            total_size += data.remaining();
                        }

                        // Check for stream errors after body
                        if stream.recv_data().await.is_err() {
                            body_error = true;
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
                        error!("H3 recv_response error: {}", e);
                        RequestStatsBuilder::default()
                            .bytes_processed(0)
                            .status(e.to_string())
                            .is_success(false)
                            .duration(Instant::now().duration_since(start))
                            .fatal_error(false)
                            .build()
                            .expect("RequestStatsBuilder failed")
                    }
                }
            }
            Err(e) => {
                error!("H3 send_request error: {}", e);
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

impl fmt::Display for H3BenchAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "H3 Config={:?}, Request={}", self.config, self.request)
    }
}
