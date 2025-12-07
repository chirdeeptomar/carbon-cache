#!/usr/bin/env rust-script
//! High-performance load test with unique keys per request
//!
//! ```cargo
//! [dependencies]
//! tokio = { version = "1", features = ["full"] }
//! reqwest = { version = "0.11", features = ["json"] }
//! serde = { version = "1", features = ["derive"] }
//! serde_json = "1"
//! base64 = "0.21"
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

#[derive(Debug)]
struct LoadTestStats {
    total_requests: u64,
    successful: u64,
    failed: u64,
    duration_ms: u64,
}

impl LoadTestStats {
    fn print_summary(&self) {
        println!("\n=== Load Test Results ===");
        println!("Duration:          {} ms ({:.2} seconds)", self.duration_ms, self.duration_ms as f64 / 1000.0);
        println!("Total requests:    {}", self.total_requests);
        println!("Successful:        {} ({:.2}%)", self.successful, (self.successful as f64 / self.total_requests as f64) * 100.0);
        println!("Failed:            {} ({:.2}%)", self.failed, (self.failed as f64 / self.total_requests as f64) * 100.0);

        let rps = (self.total_requests as f64 / (self.duration_ms as f64 / 1000.0)) as u64;
        println!("Throughput:        {} req/sec", rps);
        println!("Avg latency:       {:.2} ms", self.duration_ms as f64 / self.total_requests as f64);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Load Test: Unique Keys Per Request ===");
    println!();

    // Configuration
    let url_base = "http://localhost:8080/cache/test";
    let username = "admin";
    let password = "admin123";
    let total_requests = 10_000u64;
    let concurrency = 100;

    println!("Configuration:");
    println!("  URL:             {}/[unique-key]", url_base);
    println!("  Total requests:  {}", total_requests);
    println!("  Concurrency:     {}", concurrency);
    println!("  Auth:            Basic Auth ({})", username);
    println!();

    // Create auth header
    let auth_value = format!("{}:{}", username, password);
    let auth_header = format!("Basic {}", base64::encode(auth_value.as_bytes()));

    // Shared counters
    let success_count = Arc::new(AtomicU64::new(0));
    let fail_count = Arc::new(AtomicU64::new(0));
    let request_counter = Arc::new(AtomicU64::new(0));

    // Create HTTP client (reuse connections)
    let client = Arc::new(
        reqwest::Client::builder()
            .pool_max_idle_per_host(concurrency)
            .build()?
    );

    println!("Starting load test...");
    let start = Instant::now();

    // Spawn concurrent workers
    let mut tasks = JoinSet::new();
    let requests_per_worker = total_requests / concurrency as u64;

    for worker_id in 0..concurrency {
        let client = client.clone();
        let url_base = url_base.to_string();
        let auth_header = auth_header.clone();
        let success_count = success_count.clone();
        let fail_count = fail_count.clone();
        let request_counter = request_counter.clone();

        tasks.spawn(async move {
            for i in 0..requests_per_worker {
                // Generate unique key using worker ID and iteration
                let key = format!("key-{}-{}", worker_id, i);
                let url = format!("{}/{}", url_base, key);

                // Make PUT request
                let result = client
                    .put(&url)
                    .header("Authorization", &auth_header)
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({"value": format!("test-value-{}", i)}))
                    .send()
                    .await;

                request_counter.fetch_add(1, Ordering::Relaxed);

                match result {
                    Ok(resp) if resp.status().is_success() => {
                        success_count.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => {
                        fail_count.fetch_add(1, Ordering::Relaxed);
                    }
                }

                // Print progress every 1000 requests
                let count = request_counter.load(Ordering::Relaxed);
                if count % 1000 == 0 {
                    let elapsed = start.elapsed().as_millis();
                    let current_rps = if elapsed > 0 {
                        (count as f64 / (elapsed as f64 / 1000.0)) as u64
                    } else {
                        0
                    };
                    print!("\rProgress: {}/{} requests ({} req/sec)     ",
                           count, total_requests, current_rps);
                }
            }
        });
    }

    // Wait for all workers to complete
    while let Some(_) = tasks.join_next().await {}

    let duration = start.elapsed();
    println!("\n");

    // Collect and print stats
    let stats = LoadTestStats {
        total_requests,
        successful: success_count.load(Ordering::Relaxed),
        failed: fail_count.load(Ordering::Relaxed),
        duration_ms: duration.as_millis() as u64,
    };

    stats.print_summary();

    Ok(())
}