use axum::{extract::State, response::Json};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::timeout;

use crate::state::AppState;

/// GET /raft/metrics
pub async fn raft_metrics(State(state): State<AppState>) -> Json<Value> {
    let Some(node) = state.raft_node.as_ref() else {
        return Json(json!({ "mode": "standalone" }));
    };
    let m = node.raft.metrics().borrow().clone();
    Json(json!({
        "mode": "cluster",
        "id": m.id,
        "current_leader": m.current_leader,
        "current_term": m.current_term,
        "last_log_index": m.last_log_index,
        "last_applied": m.last_applied,
        "membership": m.membership_config.voter_ids().collect::<Vec<_>>(),
        "state": format!("{:?}", m.state),
    }))
}

/// GET /cluster/nodes — returns all known nodes with liveness status.
pub async fn cluster_nodes(State(state): State<AppState>) -> Json<Value> {
    let Some(node) = state.raft_node.as_ref() else {
        return Json(json!({ "mode": "standalone", "nodes": [] }));
    };
    let leader_id = node.current_leader_id();
    let my_id = node.node_id();
    let all_nodes: Vec<_> = node.get_all_nodes().into_iter().collect();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap_or_default();

    let probes: Vec<_> = all_nodes
        .iter()
        .map(|(id, n)| {
            let client = client.clone();
            let probe_addr = n.http_addr.replace("0.0.0.0", "127.0.0.1");
            let url = format!("http://{}/health", probe_addr);
            let id = *id;
            async move {
                let reachable = timeout(Duration::from_millis(500), client.get(&url).send())
                    .await
                    .map(|r| r.is_ok())
                    .unwrap_or(false);
                (id, reachable)
            }
        })
        .collect();

    let results: Vec<(u64, bool)> = futures::future::join_all(probes).await;

    let nodes: Vec<Value> = all_nodes
        .into_iter()
        .map(|(id, n)| {
            let reachable = results
                .iter()
                .find(|(rid, _)| *rid == id)
                .map(|(_, r)| *r)
                .unwrap_or(false);
            json!({
                "id": id,
                "raft_addr": n.raft_addr,
                "http_addr": n.http_addr,
                "tcp_addr": n.tcp_addr,
                "is_leader": leader_id == Some(id),
                "is_self": id == my_id,
                "reachable": reachable,
            })
        })
        .collect();

    Json(json!({ "mode": "cluster", "nodes": nodes }))
}
