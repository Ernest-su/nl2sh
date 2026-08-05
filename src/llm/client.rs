use super::{chat_completions, responses, retry, LlmRequest, LlmResponse};
use crate::config::{ApiType, Config};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::{Client, StatusCode};

#[async_trait]
/// Provider-neutral asynchronous language-model client.
pub trait LlmClient: Send + Sync {
    /// Completes one request and converts the provider response to internal types.
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse>;
}
/// OpenAI-compatible HTTP client configured for one concrete API dialect.
pub struct HttpLlmClient {
    client: Client,
    endpoint: String,
    key: String,
    api_type: ApiType,
    retries: u32,
    base_delay: u64,
}
/// Builds a rustls-backed HTTP client from validated configuration.
pub fn build_client(cfg: &Config) -> Result<HttpLlmClient> {
    Ok(HttpLlmClient {
        client: Client::builder()
            .timeout(std::time::Duration::from_secs(cfg.llm_request_timeout_secs))
            .build()?,
        endpoint: cfg.endpoint.trim_end_matches('/').into(),
        key: cfg.api_key.clone(),
        api_type: cfg.api_type,
        retries: cfg.llm_retry_count,
        base_delay: cfg.llm_retry_base_delay_ms,
    })
}

#[async_trait]
impl LlmClient for HttpLlmClient {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let suffix = match self.api_type {
            ApiType::ChatCompletions => "chat/completions",
            ApiType::Responses => "responses",
        };
        let url = format!("{}/{}", self.endpoint, suffix);
        let body = match self.api_type {
            ApiType::ChatCompletions => chat_completions::request(&request),
            ApiType::Responses => responses::request(&request),
        };
        for attempt in 0..=self.retries {
            let mut builder = self.client.post(&url).json(&body);
            if !self.key.is_empty() {
                builder = builder.bearer_auth(&self.key)
            }
            let sent = tokio::select! {
                response = builder.send() => response,
                signal = tokio::signal::ctrl_c() => {
                    signal?;
                    return Err(anyhow!("LLM request cancelled by user"));
                }
            };
            match sent {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let response = retry::checked(resp).await?;
                        let value = tokio::select! {
                            value = response.json() => value.context("invalid LLM JSON")?,
                            signal = tokio::signal::ctrl_c() => {
                                signal?;
                                return Err(anyhow!("LLM response cancelled by user"));
                            }
                        };
                        return match self.api_type {
                            ApiType::ChatCompletions => chat_completions::response(value),
                            ApiType::Responses => responses::response(value),
                        };
                    }
                    if !retry::retryable_status(status) || attempt == self.retries {
                        return Err(http_error(status, resp, &self.key).await);
                    }
                }
                Err(e) => {
                    if attempt == self.retries {
                        return Err(e).context("LLM request failed");
                    }
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(retry::delay(self.base_delay, attempt)) => {},
                signal = tokio::signal::ctrl_c() => {
                    signal?;
                    return Err(anyhow!("LLM retry cancelled by user"));
                }
            }
        }
        Err(anyhow!("retry loop ended unexpectedly"))
    }
}
async fn http_error(
    status: StatusCode,
    response: reqwest::Response,
    api_key: &str,
) -> anyhow::Error {
    let mut text = response.text().await.unwrap_or_default();
    if !api_key.is_empty() {
        text = text.replace(api_key, "[REDACTED]");
    }
    anyhow!(
        "LLM HTTP {status}: {}",
        text.chars().take(512).collect::<String>()
    )
}
