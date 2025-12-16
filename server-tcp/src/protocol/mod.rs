use bytes::{Buf, BufMut, Bytes, BytesMut};

// Command type identifiers
pub const CMD_PING: u8 = 0x00;
pub const CMD_PUT: u8 = 0x01;
pub const CMD_GET: u8 = 0x02;
pub const CMD_DELETE: u8 = 0x03;

// Response type identifiers
pub const RESP_PONG: u8 = 0x00;
pub const RESP_OK: u8 = 0x01;
pub const RESP_VALUE: u8 = 0x02;
pub const RESP_NOT_FOUND: u8 = 0x03;
pub const RESP_ERROR: u8 = 0x04;

#[derive(Debug, Clone)]
pub enum Request {
    Ping,
    Put { cache_name: String, key: Bytes, value: Bytes },
    Get { cache_name: String, key: Bytes },
    Delete { cache_name: String, key: Bytes },
}

#[derive(Debug, Clone)]
pub enum Response {
    Pong,
    Ok,
    Value { value: Bytes },
    NotFound,
    Error { msg: String },
}

impl Request {
    /// Encode a Request into Bytes for transmission
    ///
    /// Format:
    /// - PING: [0x00]
    /// - PUT: [0x01][key_len: u32][value_len: u32][key bytes][value bytes]
    /// - GET: [0x02][key_len: u32][key bytes]
    /// - DELETE: [0x03][key_len: u32][key bytes]
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();

        match self {
            Request::Ping => {
                buf.put_u8(CMD_PING);
            }
            Request::Put { cache_name, key, value } => {
                buf.put_u8(CMD_PUT);
                // Encode cache_name
                let cache_name_bytes = cache_name.as_bytes();
                buf.put_u32(cache_name_bytes.len() as u32);
                buf.put_slice(cache_name_bytes);
                // Encode key and value
                buf.put_u32(key.len() as u32);
                buf.put_u32(value.len() as u32);
                buf.put_slice(key);
                buf.put_slice(value);
            }
            Request::Get { cache_name, key } => {
                buf.put_u8(CMD_GET);
                // Encode cache_name
                let cache_name_bytes = cache_name.as_bytes();
                buf.put_u32(cache_name_bytes.len() as u32);
                buf.put_slice(cache_name_bytes);
                // Encode key
                buf.put_u32(key.len() as u32);
                buf.put_slice(key);
            }
            Request::Delete { cache_name, key } => {
                buf.put_u8(CMD_DELETE);
                // Encode cache_name
                let cache_name_bytes = cache_name.as_bytes();
                buf.put_u32(cache_name_bytes.len() as u32);
                buf.put_slice(cache_name_bytes);
                // Encode key
                buf.put_u32(key.len() as u32);
                buf.put_slice(key);
            }
        }

        buf.freeze()
    }

    /// Decode a Request from Bytes received from the network
    ///
    /// This is called AFTER LengthDelimitedCodec has extracted the frame,
    /// so we receive a complete message as Bytes
    pub fn decode(mut buf: Bytes) -> Result<Self, String> {
        if buf.is_empty() {
            return Err("Empty buffer".to_string());
        }

        // Read the command byte
        let cmd = buf.get_u8();

        match cmd {
            CMD_PING => {
                // PING has no payload
                Ok(Request::Ping)
            }
            CMD_PUT => {
                // Read cache_name
                if buf.remaining() < 4 {
                    return Err("Invalid PUT: missing cache_name length".to_string());
                }
                let cache_name_len = buf.get_u32() as usize;
                if buf.remaining() < cache_name_len {
                    return Err("Invalid PUT: cache_name too short".to_string());
                }
                let cache_name_bytes = buf.copy_to_bytes(cache_name_len);
                let cache_name = String::from_utf8(cache_name_bytes.to_vec())
                    .map_err(|e| format!("Invalid cache_name UTF-8: {}", e))?;

                // Read key_len and value_len
                if buf.remaining() < 8 {
                    return Err("Invalid PUT: missing key/value length fields".to_string());
                }
                let key_len = buf.get_u32() as usize;
                let value_len = buf.get_u32() as usize;

                // Check if we have enough bytes for key + value
                if buf.remaining() < key_len + value_len {
                    return Err(format!(
                        "Invalid PUT: expected {} bytes, got {}",
                        key_len + value_len,
                        buf.remaining()
                    ));
                }

                // Extract key and value (zero-copy!)
                let key = buf.copy_to_bytes(key_len);
                let value = buf.copy_to_bytes(value_len);

                Ok(Request::Put { cache_name, key, value })
            }
            CMD_GET => {
                // Read cache_name
                if buf.remaining() < 4 {
                    return Err("Invalid GET: missing cache_name length".to_string());
                }
                let cache_name_len = buf.get_u32() as usize;
                if buf.remaining() < cache_name_len {
                    return Err("Invalid GET: cache_name too short".to_string());
                }
                let cache_name_bytes = buf.copy_to_bytes(cache_name_len);
                let cache_name = String::from_utf8(cache_name_bytes.to_vec())
                    .map_err(|e| format!("Invalid cache_name UTF-8: {}", e))?;

                // Read key_len
                if buf.remaining() < 4 {
                    return Err("Invalid GET: missing key length".to_string());
                }
                let key_len = buf.get_u32() as usize;

                if buf.remaining() < key_len {
                    return Err(format!(
                        "Invalid GET: expected {} bytes, got {}",
                        key_len,
                        buf.remaining()
                    ));
                }

                let key = buf.copy_to_bytes(key_len);
                Ok(Request::Get { cache_name, key })
            }
            CMD_DELETE => {
                // Read cache_name
                if buf.remaining() < 4 {
                    return Err("Invalid DELETE: missing cache_name length".to_string());
                }
                let cache_name_len = buf.get_u32() as usize;
                if buf.remaining() < cache_name_len {
                    return Err("Invalid DELETE: cache_name too short".to_string());
                }
                let cache_name_bytes = buf.copy_to_bytes(cache_name_len);
                let cache_name = String::from_utf8(cache_name_bytes.to_vec())
                    .map_err(|e| format!("Invalid cache_name UTF-8: {}", e))?;

                // Read key_len
                if buf.remaining() < 4 {
                    return Err("Invalid DELETE: missing key length".to_string());
                }
                let key_len = buf.get_u32() as usize;

                if buf.remaining() < key_len {
                    return Err(format!(
                        "Invalid DELETE: expected {} bytes, got {}",
                        key_len,
                        buf.remaining()
                    ));
                }

                let key = buf.copy_to_bytes(key_len);
                Ok(Request::Delete { cache_name, key })
            }
            _ => Err(format!("Unknown command: 0x{:02X}", cmd)),
        }
    }
}

impl Response {
    /// Encode a Response into Bytes for transmission
    ///
    /// Format:
    /// - PONG: [0x00]
    /// - OK: [0x01]
    /// - VALUE: [0x02][value_len: u32][value bytes]
    /// - NOT_FOUND: [0x03]
    /// - ERROR: [0x04][msg_len: u32][msg bytes]
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::new();

        match self {
            Response::Pong => {
                buf.put_u8(RESP_PONG);
            }
            Response::Ok => {
                buf.put_u8(RESP_OK);
            }
            Response::Value { value } => {
                buf.put_u8(RESP_VALUE);
                buf.put_u32(value.len() as u32);
                buf.put_slice(value);
            }
            Response::NotFound => {
                buf.put_u8(RESP_NOT_FOUND);
            }
            Response::Error { msg } => {
                buf.put_u8(RESP_ERROR);
                let msg_bytes = msg.as_bytes();
                buf.put_u32(msg_bytes.len() as u32);
                buf.put_slice(msg_bytes);
            }
        }

        buf.freeze()
    }

    /// Decode a Response from Bytes received from the network
    pub fn decode(mut buf: Bytes) -> Result<Self, String> {
        if buf.is_empty() {
            return Err("Empty buffer".to_string());
        }

        let resp_type = buf.get_u8();

        match resp_type {
            RESP_PONG => Ok(Response::Pong),
            RESP_OK => Ok(Response::Ok),
            RESP_VALUE => {
                if buf.remaining() < 4 {
                    return Err("Invalid VALUE: missing length".to_string());
                }

                let value_len = buf.get_u32() as usize;

                if buf.remaining() < value_len {
                    return Err(format!(
                        "Invalid VALUE: expected {} bytes, got {}",
                        value_len,
                        buf.remaining()
                    ));
                }

                let value = buf.copy_to_bytes(value_len);
                Ok(Response::Value { value })
            }
            RESP_NOT_FOUND => Ok(Response::NotFound),
            RESP_ERROR => {
                if buf.remaining() < 4 {
                    return Err("Invalid ERROR: missing length".to_string());
                }

                let msg_len = buf.get_u32() as usize;

                if buf.remaining() < msg_len {
                    return Err(format!(
                        "Invalid ERROR: expected {} bytes, got {}",
                        msg_len,
                        buf.remaining()
                    ));
                }

                let msg_bytes = buf.copy_to_bytes(msg_len);
                let msg = String::from_utf8_lossy(&msg_bytes).to_string();
                Ok(Response::Error { msg })
            }
            _ => Err(format!("Unknown response type: 0x{:02X}", resp_type)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_encode_decode() {
        let req = Request::Ping;
        let encoded = req.encode();
        let decoded = Request::decode(encoded).unwrap();
        assert!(matches!(decoded, Request::Ping));
    }

    #[test]
    fn test_put_encode_decode() {
        let req = Request::Put {
            cache_name: "test_cache".to_string(),
            key: Bytes::from("hello"),
            value: Bytes::from("world"),
        };
        let encoded = req.encode();
        let decoded = Request::decode(encoded).unwrap();

        match decoded {
            Request::Put { cache_name, key, value } => {
                assert_eq!(cache_name, "test_cache");
                assert_eq!(key, Bytes::from("hello"));
                assert_eq!(value, Bytes::from("world"));
            }
            _ => panic!("Expected Put"),
        }
    }

    #[test]
    fn test_response_value_encode_decode() {
        let resp = Response::Value {
            value: Bytes::from("test_data"),
        };
        let encoded = resp.encode();
        let decoded = Response::decode(encoded).unwrap();

        match decoded {
            Response::Value { value } => {
                assert_eq!(value, Bytes::from("test_data"));
            }
            _ => panic!("Expected Value"),
        }
    }
}
