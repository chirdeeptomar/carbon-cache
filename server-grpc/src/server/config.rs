pub fn get_server_port() -> u32 {
    50051
}

pub fn get_server_host() -> String {
    "[::1]".to_string()
}

pub fn get_scheme_grpc() -> String {
    "grpc".to_string()
}

#[allow(dead_code)]
pub fn get_scheme_grpc_tls() -> String {
    "grpc+tls".to_string()
}
