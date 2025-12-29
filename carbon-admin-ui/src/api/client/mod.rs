use crate::config::Config;
use dioxus::prelude::{info, warn};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use shared_http::api::{ErrorResponse, LoginResponse};

#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    config: Config,
}

impl ApiClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            config: Config::from_env(),
        }
    }
}

impl ApiClient {
    pub async fn check_health(&self) -> Result<serde_json::Value, reqwest::Error> {
        let url = format!("{}/health", self.config.http_server);
        info!("Checking health at URL: {}", url);
        let response = self.client.get(&url).send().await?;
        response.json().await
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<LoginResponse, ErrorResponse> {
        let url = format!("{}/auth/login", self.config.http_server);
        let payload = serde_json::json!({
            "username": username,
            "password": password,
        });
        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                warn!("Login request failed: {}", e);
                ErrorResponse::new("Login request failed")
            })?;
        if response.status() == StatusCode::OK {
            Ok(response
                .json()
                .await
                .map(|r: Value| LoginResponse::from(r))
                .unwrap())
        } else {
            warn!("Login failed for user: {}", username);
            Err(ErrorResponse::new("Login failed"))
        }
    }
}
