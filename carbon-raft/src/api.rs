use std::sync::Arc;

use axum::{extract::State, response::Json, routing::get, Router};
use serde_json::{json, Value};

use crate::node::RaftCacheNode;

pub fn raft_router(node: Arc<RaftCacheNode>) -> Router {
    Router::new()
        .route("/raft/metrics", get(metrics_handler))
        .with_state(node)
}

async fn metrics_handler(State(node): State<Arc<RaftCacheNode>>) -> Json<Value> {
    let m = node.raft.metrics().borrow().clone();
    Json(json!({
        "id": m.id,
        "current_leader": m.current_leader,
        "current_term": m.current_term,
        "last_log_index": m.last_log_index,
        "last_applied": m.last_applied,
        "membership": m.membership_config.voter_ids().collect::<Vec<_>>(),
        "state": format!("{:?}", m.state),
    }))
}
