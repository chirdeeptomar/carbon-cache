use bytes::Bytes;
use carbon::{
    domain::response::{DeleteResponse, GetResponse, PutResponse},
    planes::data::operation::CacheOperations,
};

use shared::Result;

struct CarbonTcpClient {
    stream: tokio::net::TcpStream,
}

impl CarbonTcpClient {
    pub fn new(stream: tokio::net::TcpStream) -> Self {
        Self { stream }
    }
}
