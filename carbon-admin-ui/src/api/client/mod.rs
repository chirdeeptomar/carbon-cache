use dioxus::prelude::info;
use reqwest::Client;

use crate::config::Config;

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
}
