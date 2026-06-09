use async_trait::async_trait;
use reqwest::Client;

pub struct HttpResponse {
    pub data: Vec<u8>,
    #[allow(dead_code)]
    pub content_length: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    #[error("HTTP {0}: {1}")]
    HttpStatus(u16, String),
    #[error("connection error: {0}")]
    Connection(String),
    #[error("{0}")]
    Other(String),
}

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get(&self, url: &str) -> Result<HttpResponse, HttpClientError>;
}

pub struct HttpClientProd {
    client: Client,
}

impl HttpClientProd {
    pub fn new() -> Result<Self, HttpClientError> {
        let client = Client::builder()
            .user_agent("nodepilot/0.1.0")
            .build()
            .map_err(|e| HttpClientError::Other(e.to_string()))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl HttpClient for HttpClientProd {
    async fn get(&self, url: &str) -> Result<HttpResponse, HttpClientError> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| HttpClientError::Connection(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(HttpClientError::HttpStatus(
                resp.status().as_u16(),
                resp.status().canonical_reason().unwrap_or("unknown").to_string(),
            ));
        }

        let content_length = resp.content_length();
        let data = resp
            .bytes()
            .await
            .map_err(|e| HttpClientError::Other(e.to_string()))?
            .to_vec();

        Ok(HttpResponse { data, content_length })
    }
}
