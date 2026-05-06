//! Read-only Typesense client. Wraps the Typesense `documents/search`
//! endpoint for symbol lookups. Writes are owned by `apps/symbol-service`
//! (scheduled job on the VPS); this client never modifies the collection.

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Clone)]
pub struct TypesenseClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    collection: String,
}

/// Mirrors the `TickerDoc` shape written by `symbol-service`.
#[derive(Debug, Clone, Deserialize)]
pub struct TickerDoc {
    pub id: String,
    pub symbol: String,
    #[serde(default)]
    pub long_name: Option<String>,
    pub exchange: String,
    pub quote_type: String,
}

impl TypesenseClient {
    pub fn new(base_url: &str, api_key: &str, collection: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("build typesense reqwest client")?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            collection: collection.to_string(),
        })
    }

    /// Run a fuzzy search across `symbol` and `long_name`. Returns up to
    /// `limit` matches sorted by Typesense's relevance score. Empty `q`
    /// returns `Ok(vec![])` without hitting the network.
    pub async fn search(&self, q: &str, limit: u32) -> Result<Vec<TickerDoc>> {
        if q.trim().is_empty() {
            return Ok(Vec::new());
        }
        let limit = limit.clamp(1, 50);
        let url = format!(
            "{}/collections/{}/documents/search",
            self.base_url, self.collection
        );
        let resp = self
            .http
            .get(&url)
            .header("X-TYPESENSE-API-KEY", &self.api_key)
            .query(&[
                ("q", q),
                ("query_by", "symbol,long_name"),
                ("query_by_weights", "3,1"),
                ("per_page", &limit.to_string()),
                ("num_typos", "1"),
                ("prefix", "true"),
            ])
            .send()
            .await
            .context("typesense search request")?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("typesense search {}: {}", status, body);
        }
        #[derive(Deserialize)]
        struct Hit {
            document: TickerDoc,
        }
        #[derive(Deserialize)]
        struct SearchResponse {
            hits: Vec<Hit>,
        }
        let parsed: SearchResponse =
            serde_json::from_str(&body).with_context(|| format!("parse typesense response: {body}"))?;
        Ok(parsed.hits.into_iter().map(|h| h.document).collect())
    }
}
