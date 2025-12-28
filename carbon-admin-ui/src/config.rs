#[derive(Clone)]
pub struct Config {
    pub http_server: String,
    pub api_base_url: String,
}

const DEFAULT_HTTP_SERVER: &str = "http://localhost:8090";

impl Config {
    pub fn from_env() -> Self {
        let carbon_http_server =
            std::env::var("CARBON_HTTP_SERVER").unwrap_or_else(|_| DEFAULT_HTTP_SERVER.to_string());

        let api_base_url = format!("{}{}", carbon_http_server, "/api");
        Self {
            http_server: carbon_http_server,
            api_base_url,
        }
    }
}
