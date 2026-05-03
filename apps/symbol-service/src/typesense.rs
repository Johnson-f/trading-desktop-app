use anyhow::{Context, Result};
use reqwest::StatusCode;
use serde_json::json;

use crate::filter::TickerDoc;

#[derive(Clone)]
pub struct TypesenseClient {
    http: reqwest::Client,
    base: String,
    api_key: String,
    collection: String,
}

impl TypesenseClient {
    pub fn new(base_url: &str, api_key: &str, collection: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("build reqwest client")?;
        Ok(Self {
            http,
            base: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            collection: collection.to_string(),
        })
    }

    pub fn collection(&self) -> &str {
        &self.collection
    }

    /// Create the collection if it does not exist. Treats 201 (created) and
    /// 409 (already exists) as success; anything else is an error.
    pub async fn ensure_collection(&self) -> Result<()> {
        let body = json!({
            "name": self.collection,
            "fields": [
                { "name": "id", "type": "string" },
                { "name": "symbol", "type": "string" },
                { "name": "long_name", "type": "string", "optional": true },
                { "name": "exchange", "type": "string", "facet": true },
                { "name": "quote_type", "type": "string", "facet": true }
            ]
        });
        let resp = self
            .http
            .post(format!("{}/collections", self.base))
            .header("X-TYPESENSE-API-KEY", &self.api_key)
            .json(&body)
            .send()
            .await
            .context("POST /collections")?;
        let status = resp.status();
        if status == StatusCode::CREATED || status == StatusCode::CONFLICT {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("ensure_collection: {} {}", status, body);
    }

    /// Upsert a batch of docs. Body is NDJSON (one doc per line). Returns the
    /// number of successfully imported docs.
    pub async fn upsert_batch(&self, docs: &[TickerDoc]) -> Result<usize> {
        if docs.is_empty() {
            return Ok(0);
        }
        let mut body = String::with_capacity(docs.len() * 128);
        for doc in docs {
            body.push_str(&serde_json::to_string(doc).context("serialize doc")?);
            body.push('\n');
        }
        let url = format!(
            "{}/collections/{}/documents/import?action=upsert",
            self.base, self.collection
        );
        let resp = self
            .http
            .post(&url)
            .header("X-TYPESENSE-API-KEY", &self.api_key)
            .header("Content-Type", "text/plain")
            .body(body)
            .send()
            .await
            .context("POST documents/import")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("upsert_batch: {} {}", status, text);
        }
        // Response is NDJSON — one `{"success": true}` line per doc.
        #[derive(serde::Deserialize)]
        struct ImportResult {
            success: bool,
        }
        let mut ok = 0usize;
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<ImportResult>(line) {
                Ok(r) if r.success => ok += 1,
                Ok(_) => tracing::warn!(line = %line, "typesense per-doc failure"),
                Err(e) => {
                    tracing::warn!(line = %line, error = %e, "typesense malformed response line")
                }
            }
        }
        Ok(ok)
    }

    /// Stream every document `id` currently in the collection. Used by the
    /// stale-cleanup pass after a successful run.
    pub async fn list_all_ids(&self) -> Result<Vec<String>> {
        let url = format!(
            "{}/collections/{}/documents/export?include_fields=id",
            self.base, self.collection
        );
        let resp = self
            .http
            .get(&url)
            .header("X-TYPESENSE-API-KEY", &self.api_key)
            .send()
            .await
            .context("GET documents/export")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("list_all_ids: {} {}", status, text);
        }
        let mut ids = Vec::new();
        for line in text.lines() {
            #[derive(serde::Deserialize)]
            struct Row {
                id: String,
            }
            if let Ok(row) = serde_json::from_str::<Row>(line) {
                ids.push(row.id);
            }
        }
        Ok(ids)
    }

    /// Number of documents currently in the collection. Used at boot to
    /// decide whether to run an initial seed sync (empty → seed now) or
    /// just sleep until the next scheduled update window.
    pub async fn count_documents(&self) -> Result<u64> {
        let url = format!("{}/collections/{}", self.base, self.collection);
        let resp = self
            .http
            .get(&url)
            .header("X-TYPESENSE-API-KEY", &self.api_key)
            .send()
            .await
            .context("GET /collections/<name>")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("count_documents: {} {}", status, text);
        }
        #[derive(serde::Deserialize)]
        struct Body {
            num_documents: u64,
        }
        let body: Body = serde_json::from_str(&text)
            .with_context(|| format!("parse collection metadata: {text}"))?;
        Ok(body.num_documents)
    }

    pub async fn delete_document(&self, id: &str) -> Result<()> {
        let url = format!(
            "{}/collections/{}/documents/{}",
            self.base, self.collection, id
        );
        let resp = self
            .http
            .delete(&url)
            .header("X-TYPESENSE-API-KEY", &self.api_key)
            .send()
            .await
            .context("DELETE document")?;
        if !resp.status().is_success() && resp.status() != StatusCode::NOT_FOUND {
            anyhow::bail!("delete_document {}: {}", id, resp.status());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn doc(sym: &str) -> TickerDoc {
        TickerDoc {
            id: sym.to_string(),
            symbol: sym.to_string(),
            long_name: None,
            exchange: "NMS".into(),
            quote_type: "EQUITY".into(),
        }
    }

    #[tokio::test]
    async fn ensure_collection_treats_409_as_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/collections"))
            .and(header("X-TYPESENSE-API-KEY", "k"))
            .respond_with(ResponseTemplate::new(409))
            .mount(&server)
            .await;
        let client = TypesenseClient::new(&server.uri(), "k", "tickers").unwrap();
        client.ensure_collection().await.unwrap();
    }

    #[tokio::test]
    async fn upsert_batch_sends_ndjson_and_counts_successes() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/collections/tickers/documents/import"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("{\"success\":true}\n{\"success\":true}\n"),
            )
            .mount(&server)
            .await;
        let client = TypesenseClient::new(&server.uri(), "k", "tickers").unwrap();
        let n = client
            .upsert_batch(&[doc("AAPL"), doc("MSFT")])
            .await
            .unwrap();
        assert_eq!(n, 2);
    }

    #[tokio::test]
    async fn count_documents_reads_num_documents_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/collections/tickers"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"name":"tickers","num_documents":42}"#),
            )
            .mount(&server)
            .await;
        let client = TypesenseClient::new(&server.uri(), "k", "tickers").unwrap();
        let n = client.count_documents().await.unwrap();
        assert_eq!(n, 42);
    }

    #[tokio::test]
    async fn list_all_ids_parses_ndjson_export() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/collections/tickers/documents/export"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("{\"id\":\"AAPL\"}\n{\"id\":\"MSFT\"}\n"),
            )
            .mount(&server)
            .await;
        let client = TypesenseClient::new(&server.uri(), "k", "tickers").unwrap();
        let ids = client.list_all_ids().await.unwrap();
        assert_eq!(ids, vec!["AAPL".to_string(), "MSFT".to_string()]);
    }
}
