/// Transparent HTTP forwarding to the Raft leader.
///
/// When a follower node receives a mutating HTTP request it cannot serve
/// locally — it proxies the full request to the leader's HTTP address and
/// returns the leader's response verbatim to the client.
use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use carbon_raft::node::RaftCacheNode;
use std::sync::Arc;
use tracing::info;

static CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(reqwest::Client::new);

/// Forward the given request to the leader. Returns the leader's response
/// as an axum `Response<Body>`, or a 503 if no leader is known.
pub async fn forward_to_leader(
    node: &Arc<RaftCacheNode>,
    req: Request<Body>,
) -> Response<Body> {
    let leader = match node.get_leader_node() {
        Some(n) => n,
        None => {
            return Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(Body::from("no leader elected"))
                .unwrap();
        }
    };

    // Reconstruct the URL on the leader's HTTP address.
    // leader.http_addr is e.g. "0.0.0.0:8080" — replace 0.0.0.0 with localhost
    // for loopback clusters; in production these will be real hostnames/IPs.
    let host = leader.http_addr.replace("0.0.0.0", "127.0.0.1");
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("");
    let url = format!("http://{host}{path_and_query}");

    info!("Forwarding {} {} to leader at {}", req.method(), path_and_query, host);

    let method = req.method().clone();
    let headers = req.headers().clone();

    // Collect body bytes
    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from("failed to read request body"))
                .unwrap();
        }
    };

    let mut builder = CLIENT.request(method, &url);
    for (name, value) in &headers {
        builder = builder.header(name, value);
    }
    builder = builder.body(body_bytes.to_vec());

    match builder.send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let mut response_builder = Response::builder().status(status);
            for (name, value) in resp.headers() {
                response_builder = response_builder.header(name, value);
            }
            let body = resp.bytes().await.unwrap_or_default();
            response_builder.body(Body::from(body)).unwrap()
        }
        Err(e) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from(format!("leader unreachable: {e}")))
            .unwrap(),
    }
}
