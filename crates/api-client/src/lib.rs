use zaned_core::HealthResponse;

pub struct ApiClient {
    base_url: String,
    http: reqwest::Client,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn health(&self) -> Result<HealthResponse, reqwest::Error> {
        self.http
            .get(format!("{}/health", self.base_url))
            .send()
            .await?
            .json()
            .await
    }
}
