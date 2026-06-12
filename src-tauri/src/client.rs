#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;

use async_trait::async_trait;
use reqwest::Client;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub data: Vec<u8>,
    #[allow(dead_code)]
    pub content_length: Option<u64>,
}

#[derive(Debug, Clone, thiserror::Error)]
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
    async fn get(&self, url: &str) -> Result<HttpResponse, HttpClientError> {
        self.get_streamed(url, &|_, _| {}).await
    }

    async fn get_streamed(
        &self,
        url: &str,
        progress: &(dyn Fn(u64, u64) + Sync),
    ) -> Result<HttpResponse, HttpClientError>;
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
    async fn get_streamed(
        &self,
        url: &str,
        progress: &(dyn Fn(u64, u64) + Sync),
    ) -> Result<HttpResponse, HttpClientError> {
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

        let total = resp.content_length().unwrap_or(0);
        let mut data = Vec::with_capacity(total as usize);
        let mut received: u64 = 0;

        let mut stream = resp;
        while let Some(chunk) = stream
            .chunk()
            .await
            .map_err(|e| HttpClientError::Other(e.to_string()))?
        {
            received += chunk.len() as u64;
            data.extend_from_slice(&chunk);
            if total > 0 {
                progress(received, total);
            }
        }

        Ok(HttpResponse {
            content_length: Some(total),
            data,
        })
    }
}

#[cfg(test)]
pub struct HttpClientMock {
    responses: Mutex<HashMap<String, Result<HttpResponse, HttpClientError>>>,
}

#[cfg(test)]
impl HttpClientMock {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(HashMap::new()),
        }
    }

    pub fn expect(&self, url: &str, result: Result<HttpResponse, HttpClientError>) {
        self.responses
            .lock()
            .unwrap()
            .insert(url.to_string(), result);
    }
}

#[cfg(test)]
#[async_trait]
impl HttpClient for HttpClientMock {
    async fn get_streamed(
        &self,
        url: &str,
        _progress: &(dyn Fn(u64, u64) + Sync),
    ) -> Result<HttpResponse, HttpClientError> {
        let responses = self.responses.lock().unwrap();
        responses
            .get(url)
            .cloned()
            .unwrap_or_else(|| Err(HttpClientError::Other(format!("no mock for {url}"))))
    }
}
