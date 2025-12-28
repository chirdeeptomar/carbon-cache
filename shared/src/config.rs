use tracing::warn;

pub enum Protocol {
    Http(u16),                  // port
    Https(u16, String, String), // port, cert_path, key_path,
    Tcp(u16),                   // port
    Tcps(u16, String, String),  // port, cert_path, key_path,
}

pub struct Config {
    pub host: String,
    pub http: Protocol,
    pub tcp: Protocol,
    pub data_dir: String,
    pub admin_username: String,
    pub admin_password: String,
    pub allowed_origins: Vec<String>,
}

impl Config {
    const DEFAULT_ADMIN_USERNAME: &str = "admin";
    const DEFAULT_ADMIN_PASSWORD: &str = "admin123";
    const DEFAULT_DATA_DIR: &str = "./data";

    pub fn from_env() -> Self {
        let host = std::env::var("CARBON_HOST").unwrap_or_else(|_| "localhost".to_string());
        let tcp_port = std::env::var("CARBON_TCP_PORT")
            .unwrap_or_else(|_| "5500".to_string())
            .parse::<u16>()
            .unwrap_or(5500);
        let http_port = std::env::var("CARBON_HTTP_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .unwrap_or(8080);
        let https_port = std::env::var("CARBON_HTTPS_PORT")
            .unwrap_or_else(|_| "8443".to_string())
            .parse::<u16>()
            .unwrap_or(8443);
        let tls_cert_path = std::env::var("CARBON_TLS_CERT_PATH").ok();
        let tls_key_path = std::env::var("CARBON_TLS_KEY_PATH").ok();
        Self {
            host,
            data_dir: std::env::var("CARBON_DATA_DIR")
                .unwrap_or_else(|_| Self::DEFAULT_DATA_DIR.to_string()),
            admin_username: std::env::var("CARBON_ADMIN_USERNAME")
                .unwrap_or_else(|_| Self::DEFAULT_ADMIN_USERNAME.to_string()),
            admin_password: std::env::var("CARBON_ADMIN_PASSWORD").unwrap_or_else(|_| {
                warn!("CARBON_ADMIN_PASSWORD not set, using default password 'admin123'");
                warn!("⚠️  WARNING: Please change the default admin password immediately!");
                Self::DEFAULT_ADMIN_PASSWORD.to_string()
            }),
            http: match (&tls_cert_path, &tls_key_path) {
                (Some(cert), Some(key)) => Protocol::Https(https_port, cert.clone(), key.clone()),
                _ => Protocol::Http(http_port),
            },
            tcp: match (&tls_cert_path, &tls_key_path) {
                (Some(cert), Some(key)) => Protocol::Tcps(tcp_port, cert.clone(), key.clone()),
                _ => Protocol::Tcp(tcp_port),
            },
            allowed_origins: std::env::var("CARBON_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "*".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
        }
    }
}

impl Protocol {
    pub fn port(&self) -> u16 {
        match self {
            Protocol::Http(port) | Protocol::Tcp(port) => *port,

            Protocol::Https(port, _, _) | Protocol::Tcps(port, _, _) => *port,
        }
    }

    pub fn is_tls(&self) -> bool {
        matches!(self, Protocol::Https(..) | Protocol::Tcps(..))
    }

    pub fn tls_paths(&self) -> Option<(&str, &str)> {
        match self {
            Protocol::Https(_, cert, key) | Protocol::Tcps(_, cert, key) => Some((cert, key)),
            _ => None,
        }
    }

    pub fn is_http(&self) -> bool {
        matches!(self, Protocol::Http(..) | Protocol::Https(..))
    }

    pub fn http_protcol(&self) -> &str {
        match self {
            Protocol::Http(..) => "http",
            Protocol::Https(..) => "https",
            _ => "tcp",
        }
    }

    pub fn tcp_protcol(&self) -> &str {
        match self {
            Protocol::Tcp(..) => "tcp",
            Protocol::Tcps(..) => "tcp+tls",
            _ => "unknown",
        }
    }
}
