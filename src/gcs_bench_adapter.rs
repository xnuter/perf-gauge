use crate::bench_run::BenchmarkProtocolAdapter;
use crate::metrics::{RequestStats, RequestStatsBuilder};
use async_trait::async_trait;
use google_cloud::authorize::ApplicationCredentials;
use google_cloud::storage;
use rand::{thread_rng, Rng};
use std::time::Instant;

#[derive(Builder, Deserialize, Clone, Debug)]
pub struct GcsBenchAdapter {
    gcp_project: String,
    bucket: String,
    objects: Vec<String>,
}

#[async_trait]
impl BenchmarkProtocolAdapter for GcsBenchAdapter {
    type Client = google_cloud::storage::Client;

    async fn build_client(&self) -> Result<Self::Client, String> {
        setup_client(self.gcp_project.clone()).await.map_err(|e| {
            format!(
                "Failed to build client for project={}, err={:?}",
                self.gcp_project, e
            )
        })
    }

    async fn send_request(&self, client: &Self::Client) -> RequestStats {
        let start = Instant::now();
        //? List all buckets of the project.
        let mut client = client.clone();
        let bucket = client.bucket(&self.bucket).await;
        if bucket.is_err() {
            return RequestStatsBuilder::default()
                .bytes_processed(0)
                .status(format!(
                    "Unexpected error getting bucket {}. Error: {:?}",
                    self.bucket,
                    bucket.err().expect("Error must be here")
                ))
                .is_success(false)
                .duration(Instant::now().duration_since(start))
                .build()
                .expect("Error building RequestStats");
        }
        let mut bucket = bucket.ok().expect("Bucket must exist at this point");

        let mut bytes_processed = 0;
        let object_name = self.objects[thread_rng().gen_range(0..self.objects.len())].clone();
        let result = bucket.object(object_name.as_str()).await;
        match result {
            Ok(mut r) => {
                let data = r.get().await.unwrap_or(vec![]);
                bytes_processed += data.len();

                RequestStatsBuilder::default()
                    .bytes_processed(bytes_processed)
                    .status("OK".to_string())
                    .is_success(true)
                    .duration(Instant::now().duration_since(start))
                    .build()
                    .expect("RequestStatsBuilder failed")
            }
            Err(e) => RequestStatsBuilder::default()
                .bytes_processed(0)
                .status(format!(
                    "Unexpected error getting bucket {}. Error: {:?}",
                    self.bucket, e
                ))
                .is_success(false)
                .duration(Instant::now().duration_since(start))
                .build()
                .expect("Error building RequestStats"),
        }
    }
}

fn load_creds() -> ApplicationCredentials {
    let creds = std::env::var("GCP_TEST_CREDENTIALS").expect("env GCP_TEST_CREDENTIALS not set");
    serde_json::from_str::<ApplicationCredentials>(&creds)
        .expect("incorrect application credentials format")
}

async fn setup_client(gcp_project: String) -> Result<storage::Client, storage::Error> {
    let creds = load_creds();
    storage::Client::from_credentials(gcp_project, creds).await
}
